/* ═══════════════════════════════════════════════════════════════
   Bambe Web — admin-support.js

   The support inbox: a queue on the left, one thread on the right,
   against the five `/v1/admin/support/ticket/*` endpoints plus
   `/v1/admin/admins` for the assignee picker.

   The customer half of this conversation is `support.js` on the
   storefront. Both were written together; a reply sent here shows
   up there and moves the ticket to Pending, which is exactly what
   `reply.rs` does.

   An **internal note** is a real thing the endpoint supports
   (`is_internal`), and `detail.rs` on the customer side filters
   them out — so "Save as note" is kept and is genuinely private.
   It is drawn differently from a reply so nobody posts a note
   thinking the customer will read it.

   What the wireframe drew and the API cannot do:

   - **Median first reply / Satisfaction tiles** — nothing measures
     either. Replaced with counts the list endpoint does return.
   - **SLA breach** — no SLA is configured anywhere.
   - **New ticket** — `create.rs` opens a ticket *as the customer*,
     using their email; staff opening one on someone's behalf would
     put the wrong name on the thread. Removed.
   - **Canned responses** — nothing stores them.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, ago, dateTime, spaced, debounce,
  ticketTag, tag, mountAdminChrome, openModal, field, selectInput,
} from './admin-core.js';

const TABS = [
  { value: 'open', label: 'Open' },
  { value: 'pending', label: 'Pending' },
  { value: 'resolved', label: 'Resolved' },
  { value: 'closed', label: 'Closed' },
];

const PRIORITY_CLASS = { Urgent: 'prio-high', High: 'prio-high', Normal: 'prio-med', Low: 'prio-low' };

let state = { status: 'open', assignee: 'all', page: 1, limit: 30 };
let tickets = [];
let admins = [];
let current = null;      // the open ticket id
let queue = null;

const me = () => (admins.find((a) => a.is_you) || {});

function initials(name) {
  if (!name) return '?';
  return name.trim().split(/\s+/).slice(0, 2).map((part) => part[0]).join('').toUpperCase();
}

/* ── Queue ────────────────────────────────────────────────────  */

function ticketItem(ticket) {
  const item = document.createElement('div');
  item.className = 'ticket-item' + (ticket.uuid === current ? ' active' : '');
  item.tabIndex = 0;
  item.dataset.id = ticket.uuid;

  const top = document.createElement('div');
  top.className = 'ticket-item-top';
  const id = document.createElement('span');
  id.className = 'ticket-id';
  id.textContent = '#' + ticket.number;
  const time = document.createElement('span');
  time.className = 'ticket-time';
  time.textContent = ago(ticket.last_message_at);
  top.appendChild(id);
  top.appendChild(time);
  item.appendChild(top);

  const subject = document.createElement('div');
  subject.className = 'ticket-subject';
  subject.textContent = ticket.subject;
  item.appendChild(subject);

  const preview = document.createElement('div');
  preview.className = 'ticket-preview';
  preview.textContent = ticket.last_message_preview || '';
  item.appendChild(preview);

  const meta = document.createElement('div');
  meta.className = 'ticket-meta';

  const priority = document.createElement('span');
  priority.className = 'prio ' + (PRIORITY_CLASS[ticket.priority] || 'prio-med');
  priority.textContent = ticket.priority;
  meta.appendChild(priority);

  meta.appendChild(tag(ticket.topic, 'tag-gray'));

  /* An unanswered ticket is the one thing an operator scans for,
     and `unread_by_staff` is exactly that flag. */
  if (ticket.unread_by_staff) meta.appendChild(tag('New reply', 'tag-blue'));
  meta.appendChild(document.createTextNode(''));
  item.appendChild(meta);

  const open = () => openTicket(ticket.uuid);
  item.addEventListener('click', open);
  item.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });

  return item;
}

function paintQueue() {
  if (!queue) return;
  if (window.Skeleton) window.Skeleton.clear(queue);
  queue.innerHTML = '';

  const filtered = tickets.filter((ticket) => {
    if (state.assignee === 'mine') return ticket.assigned_to === me().user_id;
    if (state.assignee === 'none') return !ticket.assigned_to;
    return true;
  });

  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.style.cssText = 'padding:32px 16px;text-align:center;color:var(--subtle);font-size:13px;';
    empty.textContent = state.assignee === 'mine'
      ? 'Nothing assigned to you here'
      : 'Nothing in this tab';
    queue.appendChild(empty);
    return;
  }

  filtered.forEach((ticket) => queue.appendChild(ticketItem(ticket)));
}

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;
  host.innerHTML = '';

  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.status ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;

    const pick = () => { state.status = value; current = null; load(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

function paintStats(stats) {
  const tiles = $$('.stat-tile');
  const values = [stats.open || 0, stats.pending || 0, stats.unread || 0];
  const labels = ['Open', 'Awaiting the customer', 'Unanswered'];
  const feet = [
    'Waiting on us',
    'We have replied, they have not',
    'Customer wrote last',
  ];

  tiles.forEach((tile, index) => {
    const label = tile.querySelector('.stat-tile-label');
    if (label) label.textContent = labels[index];
    const value = tile.querySelector('.stat-tile-value');
    if (value) value.textContent = String(values[index]);
    const foot = tile.querySelector('.stat-tile-foot');
    if (foot) { foot.innerHTML = ''; foot.textContent = feet[index]; }
  });

  const subtitle = $('.admin-head .admin-subtitle');
  if (subtitle) {
    subtitle.textContent = (stats.open || 0) + ' open · ' + (stats.unread || 0) + ' waiting on a reply';
  }
}

/* ── Thread ───────────────────────────────────────────────────  */

function message(entry) {
  const row = document.createElement('div');
  row.className = 'msg' + (entry.from_staff ? ' staff' : '');

  const avatar = document.createElement('div');
  avatar.className = 'msg-avatar';
  avatar.textContent = initials(entry.author_name);
  row.appendChild(avatar);

  const column = document.createElement('div');

  const name = document.createElement('div');
  name.className = 'msg-name';
  name.textContent = entry.from_staff
    ? (entry.author_id === me().user_id ? 'You' : (entry.author_name || 'Support'))
    : (entry.author_name || 'Customer');

  const when = document.createElement('span');
  when.className = 'msg-time';
  when.textContent = dateTime(entry.created_at);
  name.append(' ');
  name.appendChild(when);

  if (entry.is_internal) {
    const badge = tag('Internal note', 'tag-gold');
    badge.style.marginLeft = '8px';
    name.appendChild(badge);
  }
  column.appendChild(name);

  const bubble = document.createElement('div');
  bubble.className = 'msg-bubble';
  bubble.style.whiteSpace = 'pre-wrap';
  if (entry.is_internal) {
    // Visibly not part of the conversation the customer can see.
    bubble.style.background = 'var(--warn-lt, #FDF3E3)';
    bubble.style.border = '1px dashed var(--warn)';
  }
  bubble.textContent = entry.body;
  column.appendChild(bubble);

  row.appendChild(column);
  return row;
}

async function openTicket(ticketId) {
  const response = await Fetcher.get({ endpoint: '/v1/admin/support/ticket/' + ticketId });
  if (!response.ok) return;

  current = ticketId;
  paintQueue();

  const { ticket, messages } = response.data;

  const title = $('.support-cols .admin-panel-title');
  if (title) title.textContent = ticket.subject;

  const subtitle = $$('.support-cols .admin-subtitle')[0];
  if (subtitle) {
    subtitle.textContent = '#' + ticket.number + ' · ' + ticket.name + ' · ' + ticket.email;
  }

  const chips = subtitle ? subtitle.nextElementSibling : null;
  if (chips) {
    chips.innerHTML = '';
    const priority = document.createElement('span');
    priority.className = 'prio ' + (PRIORITY_CLASS[ticket.priority] || 'prio-med');
    priority.textContent = ticket.priority;
    chips.appendChild(priority);
    chips.appendChild(tag(ticket.topic, 'tag-gray'));
    chips.appendChild(ticketTag(ticket.status));

    const assignee = admins.find((a) => a.user_id === ticket.assigned_to);
    chips.appendChild(tag(
      assignee ? (assignee.is_you ? 'Assigned to you' : 'Assigned to ' + assignee.name) : 'Unassigned',
      assignee ? 'tag-blue' : 'tag-gray'));
  }

  const body = $('.thread-body');
  if (body) {
    body.innerHTML = '';
    (messages || []).forEach((entry) => body.appendChild(message(entry)));
    body.scrollTop = body.scrollHeight;
  }

  const label = $('.reply-box .form-label');
  if (label) label.textContent = 'Reply to ' + ticket.name;

  const box = $('#reply');
  if (box) box.value = '';

  wireThreadActions(ticket);
}

function wireThreadActions(ticket) {
  const [assign, resolve] = $$('.support-cols .admin-head-actions button');

  if (assign) {
    assign.onclick = () => assignTicket(ticket);
  }

  if (resolve) {
    /* Resolve and Close are two different states, and a resolved
       ticket can still be replied to while a closed one cannot —
       so the button says which one it will do next. */
    const next = ticket.status === 'Resolved' || ticket.status === 'Closed' ? 'Open' : 'Resolved';
    resolve.textContent = next === 'Open' ? 'Reopen' : 'Resolve';
    resolve.onclick = () => setStatus(ticket, next);
  }

  const [send, note] = $$('.reply-actions button');
  if (send) send.onclick = () => reply(ticket, false, send);
  if (note) {
    note.textContent = 'Save as internal note';
    note.onclick = () => reply(ticket, true, note);
  }
}

async function reply(ticket, isInternal, button) {
  const box = $('#reply');
  const text = box ? box.value.trim() : '';
  if (!text) return;

  button.disabled = true;
  const response = await Fetcher.post({
    endpoint: '/v1/admin/support/ticket/' + ticket.uuid + '/message',
    body: { body: text, is_internal: isInternal },
  });
  button.disabled = false;
  if (!response.ok) return;

  if (box) box.value = '';
  toast.success(isInternal ? 'Note saved — the customer cannot see it' : 'Reply sent');
  await load();
  openTicket(ticket.uuid);
}

function assignTicket(ticket) {
  const options = [{ value: '', label: '— Unassigned —' }]
    .concat(admins.map((admin) => ({
      value: admin.user_id,
      label: admin.is_you ? admin.name + ' (you)' : admin.name,
    })));

  const select = selectInput(options, ticket.assigned_to || '');
  const form = document.createElement('div');
  form.appendChild(field('Assign to', select));

  openModal({
    title: 'Assign #' + ticket.number,
    body: form,
    width: '420px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Assign',
        className: 'btn-primary',
        onClick: async (close, button) => {
          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/admin/support/ticket/' + ticket.uuid + '/assign',
            body: { admin_id: select.value || null },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success(select.value ? 'Assigned' : 'Unassigned');
          await load();
          openTicket(ticket.uuid);
        },
      },
    ],
  });
}

async function setStatus(ticket, status) {
  const response = await Fetcher.patch({
    endpoint: '/v1/admin/support/ticket/' + ticket.uuid + '/status',
    body: { status },
  });
  if (!response.ok) return;

  toast.success('Ticket ' + spaced(status).toLowerCase());
  await load();
  // The ticket may have left the current tab; only reopen it if it is
  // still in the queue.
  if (tickets.some((t) => t.uuid === ticket.uuid)) openTicket(ticket.uuid);
  else clearThread();
}

function clearThread() {
  current = null;
  const title = $('.support-cols .admin-panel-title');
  if (title) title.textContent = 'No ticket selected';
  const subtitle = $$('.support-cols .admin-subtitle')[0];
  if (subtitle) subtitle.textContent = 'Pick one from the queue';
  const chips = subtitle ? subtitle.nextElementSibling : null;
  if (chips) chips.innerHTML = '';
  const body = $('.thread-body');
  if (body) body.innerHTML = '';
  const replyBox = $('.reply-box');
  if (replyBox) replyBox.style.display = 'none';
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/admin/support/ticket/list',
    query: { status: state.status, page: state.page, limit: state.limit },
  });
  if (!response.ok) return;

  tickets = response.data.items || [];
  paintStats(response.data.stats || {});
  paintQueue();

  const replyBox = $('.reply-box');
  if (replyBox) replyBox.style.display = '';

  if (!tickets.length) {
    clearThread();
    return;
  }

  if (!current || !tickets.some((t) => t.uuid === current)) {
    await openTicket(tickets[0].uuid);
  }
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  queue = $('.ticket-list');

  const adminsResponse = await Fetcher.get({ endpoint: '/v1/admin/admins', showError: false });
  admins = adminsResponse.ok ? (adminsResponse.data.admins || []) : [];

  const assignee = $('.admin-head-actions select.admin-filter');
  if (assignee) {
    assignee.innerHTML = '';
    [
      { value: 'all', label: 'All assignees' },
      { value: 'mine', label: 'Assigned to me' },
      { value: 'none', label: 'Unassigned' },
    ].forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      assignee.appendChild(option);
    });
    assignee.addEventListener('change', () => {
      state.assignee = assignee.value;
      paintQueue();
    });
  }

  /* Staff cannot open a ticket on a customer's behalf — `create.rs`
     stamps the caller's own name and email on the thread as the
     reply-to, so a staff-opened ticket would reply to staff. */
  const newTicket = $('.admin-head-actions .btn-primary');
  if (newTicket) newTicket.remove();

  // Nothing stores canned responses.
  const canned = $('.reply-actions select.admin-filter');
  if (canned) {
    const skin = canned.previousElementSibling;
    if (skin && skin.classList.contains('select-btn')) skin.remove();
    canned.remove();
  }

  paintTabs();
  load();
})();
