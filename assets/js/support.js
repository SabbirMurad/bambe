/* ═══════════════════════════════════════════════════════════════
   Bambee Web — support.js

   The four support endpoints were complete and unreachable: the
   admin console has a whole inbox to answer tickets in, and there
   was no way for a customer to open one. Both halves of that
   conversation now exist.

   Two panes on one page rather than two pages, because a thread is
   only ever read from the inbox and going "back" should not mean a
   round trip. `?id=` is kept in sync so a thread can be linked to
   and survives a reload.

   Deliberate shapes:

   - Name and email are prefilled from the profile and left
     editable, since `create.rs` stores them on the ticket as the
     reply-to — a customer writing about a gift delivered elsewhere
     may want a different address on it.
   - A Closed ticket shows its history with the composer removed:
     `reply.rs` refuses it, and a send button that always errors is
     worse than none.
   - The thread shows only what `detail.rs` returns, which already
     excludes staff-internal notes.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { emptyState, hideSkeleton } from './catalog.js';
import { openModal, field, textInput, textArea, selectInput } from './modal.js';
import { mountAccountChrome, currentProfile } from './account.js';

const $ = (sel) => document.querySelector(sel);

const TOPICS = ['Shipping', 'Returns', 'Billing', 'Stock', 'Damage', 'Account', 'Promotions', 'Other'];

const TOPIC_LABEL = {
  Shipping: 'Delivery',
  Returns: 'A return or refund',
  Billing: 'Payment or billing',
  Stock: 'Availability',
  Damage: 'Damaged item',
  Account: 'My account',
  Promotions: 'A code or promotion',
  Other: 'Something else',
};

const PILL = {
  Open: 'status-processing',
  Pending: 'status-shipped',
  Resolved: 'status-delivered',
  Closed: 'status-cancelled',
};

const STATUS_LABEL = {
  Open: 'With us',
  Pending: 'Awaiting your reply',
  Resolved: 'Resolved',
  Closed: 'Closed',
};

let inbox = null;
let thread = null;

function when(ms) {
  const then = new Date(ms);
  const today = new Date();
  const sameDay = then.toDateString() === today.toDateString();
  return sameDay
    ? then.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
    : then.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
}

/* ── Inbox ────────────────────────────────────────────────────  */

function ticketCard(ticket) {
  const card = document.createElement('div');
  card.className = 'order-card';
  card.style.cursor = 'pointer';
  card.setAttribute('role', 'button');
  card.tabIndex = 0;

  card.innerHTML =
    '<div class="order-card-header">'
      + '<div><div class="order-id-label">Ticket</div><div class="order-id"></div></div>'
      + '<div class="order-date"></div>'
      + '<div class="status-pill"></div>'
    + '</div>'
    + '<div class="order-card-body">'
      + '<div class="ticket-subject" style="font-size:14px;font-weight:700;color:var(--dark);margin-bottom:6px;"></div>'
      + '<div class="ticket-preview" style="font-size:13px;color:var(--subtle);line-height:1.6;"></div>'
    + '</div>';

  card.querySelector('.order-id').textContent = '#' + ticket.number;
  card.querySelector('.order-date').textContent =
    when(ticket.last_message_at) + ' · ' + ticket.message_count
    + (ticket.message_count === 1 ? ' message' : ' messages');

  const pill = card.querySelector('.status-pill');
  pill.classList.add(PILL[ticket.status] || 'status-processing');
  pill.textContent = STATUS_LABEL[ticket.status] || ticket.status;

  card.querySelector('.ticket-subject').textContent = ticket.subject;
  card.querySelector('.ticket-preview').textContent = ticket.last_message_preview || '';

  const open = () => openThread(ticket.uuid);
  card.addEventListener('click', open);
  card.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });

  return card;
}

async function loadInbox() {
  showInbox();

  const response = await Fetcher.get({ endpoint: '/v1/support/ticket/list' });

  hideSkeleton(inbox);
  inbox.innerHTML = '';

  if (!response.ok) {
    emptyState(inbox, "Couldn't load your messages", 'Please try again in a moment.');
    return;
  }

  const tickets = response.data || [];
  if (!tickets.length) {
    emptyState(inbox, 'No messages yet',
      'Anything you write to us — and every reply — stays here.');
    return;
  }

  const frag = document.createDocumentFragment();
  tickets.forEach((ticket) => frag.appendChild(ticketCard(ticket)));
  inbox.appendChild(frag);
}

/* ── Thread ───────────────────────────────────────────────────  */

function showInbox() {
  inbox.style.display = '';
  thread.style.display = 'none';
  const start = $('.ticket-start');
  if (start) start.style.display = '';
  const url = new URL(location.href);
  url.searchParams.delete('id');
  history.replaceState({}, '', url);
}

function messageBubble(message) {
  const staff = !!message.from_staff;

  const row = document.createElement('div');
  row.style.cssText = 'display:flex;flex-direction:column;gap:4px;max-width:78%;'
    + (staff ? 'align-self:flex-start;' : 'align-self:flex-end;');

  const bubble = document.createElement('div');
  bubble.style.cssText = 'padding:12px 14px;border-radius:var(--radius-md);font-size:13.5px;'
    + 'line-height:1.65;white-space:pre-wrap;word-break:break-word;'
    + (staff
      ? 'background:var(--input-bg);color:var(--dark);border:1px solid var(--border);'
      : 'background:var(--blue);color:var(--white);');
  bubble.textContent = message.body;

  const meta = document.createElement('div');
  meta.style.cssText = 'font-size:11.5px;color:var(--subtle);'
    + (staff ? '' : 'text-align:right;');
  meta.textContent = (staff ? (message.author_name || 'Bambee Support') : 'You')
    + ' · ' + when(message.created_at);

  row.appendChild(bubble);
  row.appendChild(meta);
  return row;
}

function composer(ticketId, onSent) {
  const box = document.createElement('div');
  box.style.cssText = 'margin-top:20px;padding-top:20px;border-top:1px solid var(--border);';

  const area = textArea({ placeholder: 'Write a reply…', rows: 3 });
  box.appendChild(area);

  const bar = document.createElement('div');
  bar.style.cssText = 'display:flex;justify-content:flex-end;margin-top:10px;';

  const send = document.createElement('button');
  send.type = 'button';
  send.className = 'btn btn-primary btn-sm';
  send.textContent = 'Send';
  send.addEventListener('click', async () => {
    const body = area.value.trim();
    if (!body) return;

    send.disabled = true;
    const response = await Fetcher.post({
      endpoint: '/v1/support/ticket/' + ticketId + '/message',
      body: { body },
    });
    send.disabled = false;
    if (!response.ok) return;

    area.value = '';
    onSent();
  });

  bar.appendChild(send);
  box.appendChild(bar);
  return box;
}

async function openThread(ticketId) {
  const response = await Fetcher.get({ endpoint: '/v1/support/ticket/' + ticketId });
  if (!response.ok) return;

  const { ticket, messages } = response.data;

  inbox.style.display = 'none';
  const start = $('.ticket-start');
  if (start) start.style.display = 'none';
  thread.style.display = '';
  thread.innerHTML = '';

  const url = new URL(location.href);
  url.searchParams.set('id', ticketId);
  history.replaceState({}, '', url);

  const back = document.createElement('button');
  back.type = 'button';
  back.className = 'btn btn-ghost btn-sm';
  back.style.marginBottom = '14px';
  back.textContent = '‹ All messages';
  back.addEventListener('click', loadInbox);
  thread.appendChild(back);

  const card = document.createElement('div');
  card.className = 'order-card';
  card.style.padding = '20px';

  const head = document.createElement('div');
  head.style.cssText = 'display:flex;justify-content:space-between;align-items:flex-start;gap:16px;'
    + 'padding-bottom:16px;margin-bottom:20px;border-bottom:1px solid var(--border);';

  const heading = document.createElement('div');
  const subject = document.createElement('div');
  subject.style.cssText = 'font-size:15px;font-weight:700;color:var(--black);line-height:1.4;';
  subject.textContent = ticket.subject;
  const meta = document.createElement('div');
  meta.style.cssText = 'font-size:12.5px;color:var(--subtle);margin-top:4px;';
  meta.textContent = '#' + ticket.number + ' · ' + (TOPIC_LABEL[ticket.topic] || ticket.topic)
    + ' · opened ' + when(ticket.created_at);
  heading.appendChild(subject);
  heading.appendChild(meta);

  const pill = document.createElement('div');
  pill.className = 'status-pill ' + (PILL[ticket.status] || 'status-processing');
  pill.style.flexShrink = '0';
  pill.textContent = STATUS_LABEL[ticket.status] || ticket.status;

  head.appendChild(heading);
  head.appendChild(pill);
  card.appendChild(head);

  const stream = document.createElement('div');
  stream.style.cssText = 'display:flex;flex-direction:column;gap:16px;';
  (messages || []).forEach((message) => stream.appendChild(messageBubble(message)));
  card.appendChild(stream);

  if (ticket.status === 'Closed') {
    const note = document.createElement('div');
    note.style.cssText = 'margin-top:20px;padding:12px 14px;background:var(--input-bg);'
      + 'border-radius:var(--radius-sm);font-size:13px;color:var(--subtle);text-align:center;';
    note.textContent = 'This conversation is closed. Start a new message if you still need us.';
    card.appendChild(note);
  } else {
    card.appendChild(composer(ticket.uuid, () => openThread(ticket.uuid)));
  }

  thread.appendChild(card);
}

/* ── New ticket ───────────────────────────────────────────────  */

async function openTicketComposer() {
  const profile = currentProfile() || {};
  const form = document.createElement('div');

  const name = textInput({
    value: [profile.first_name, profile.last_name].filter(Boolean).join(' ').trim() || profile.username || '',
    placeholder: 'Your name',
  });
  form.appendChild(field('Name', name));

  const email = textInput({ value: profile.email || '', placeholder: 'you@example.com' });
  form.appendChild(field('Reply to', email, 'We answer here as well as in this thread.'));

  const topic = selectInput(
    TOPICS.map((value) => ({ value, label: TOPIC_LABEL[value] || value })),
    'Shipping');
  form.appendChild(field('What is it about?', topic));

  /* An order reference is optional but saves a round trip, so the
     picker is filled from the caller's own orders rather than asking
     them to copy a number out of an email. */
  const orderSelect = selectInput([{ value: '', label: 'Not about a specific order' }], '');
  form.appendChild(field('Order', orderSelect));

  Fetcher.get({ endpoint: '/v1/order/list', query: { limit: 20 }, showError: false })
    .then((response) => {
      if (!response.ok) return;
      (response.data.items || []).forEach((order) => {
        const option = document.createElement('option');
        option.value = order.uuid;
        option.textContent = '#' + order.number + ' · '
          + new Date(order.placed_at).toLocaleDateString();
        orderSelect.appendChild(option);
      });
    });

  const subject = textInput({ placeholder: 'A one-line summary', maxLength: 140 });
  form.appendChild(field('Subject', subject));

  const body = textArea({ placeholder: 'Tell us what happened…', rows: 5 });
  form.appendChild(field('Message', body));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  const fail = (message) => {
    error.textContent = message;
    error.style.display = '';
  };

  openModal({
    title: 'New message',
    body: form,
    width: '560px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Send',
        className: 'btn-primary',
        onClick: async (close, button) => {
          error.style.display = 'none';

          if (!name.value.trim()) return fail('We need a name to address you by.');
          if (!email.value.trim()) return fail('We need somewhere to reply.');
          if (!subject.value.trim()) return fail('Give it a subject so it is easy to find later.');
          if (!body.value.trim()) return fail('Tell us what you need help with.');

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/support/ticket',
            body: {
              name: name.value.trim(),
              email: email.value.trim(),
              subject: subject.value.trim(),
              topic: topic.value,
              body: body.value.trim(),
              order_id: orderSelect.value || null,
            },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Sent — we usually reply within a day');
          openThread(response.data.uuid);
        },
      },
    ],
  });
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  inbox = $('.ticket-list');
  thread = $('.ticket-thread');

  const start = $('.ticket-start');
  if (start) start.addEventListener('click', openTicketComposer);

  const requested = new URLSearchParams(location.search).get('id');
  if (requested) {
    // Draw the inbox behind it, so "‹ All messages" has somewhere to go.
    await loadInbox();
    openThread(requested);
  } else {
    loadInbox();
  }
})();
