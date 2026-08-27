/* ═══════════════════════════════════════════════════════════════
   Bambe Web — admin-customers.js

   The customer list, its four stat tiles, and a drawer per person
   holding what `GET /v1/admin/customer/{id}` returns: their
   commerce rollup, their recent orders, and the two fields staff
   can actually write — the internal note and the tags.

   Things the wireframe drew that the endpoints cannot do:

   - **Search by name or email.** `customer/list` takes `segment`,
     `location`, `page` and `limit` and nothing else. Rather than
     leave a box that silently does nothing, the search filters the
     page you can see and says so; a real one needs a `q` on the
     endpoint.
   - **Sort.** The pipeline sorts by `order_count` and takes no sort
     parameter, so the select is gone rather than lying.
   - **"Email selected" and "Remove".** No mail-out endpoint, and no
     endpoint deletes a customer — account deletion is a request
     the customer makes, and it lands in Settings. Both removed.
   - **Add tag** is real: it PATCHes each selected customer.

   `lifetime_spend` is Cents like everything else.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, fullDate, dateTime, dateValue, cell, fillTable, paintPager,
  debounce, rowButton, tag, orderTag, mountAdminChrome,
  openModal, field, textInput, textArea,
} from './admin-core.js';

const TABS = [
  { value: '', label: 'All' },
  { value: 'vip', label: 'VIP' },
  { value: 'repeat', label: 'Repeat' },
  { value: 'new', label: 'New' },
  { value: 'dormant', label: 'Dormant' },
];

const SEGMENT_TAG = { Vip: 'tag-gold', Repeat: 'tag-green', New: 'tag-blue', Dormant: 'tag-red' };

let state = { segment: '', q: '', location: '', page: 1, limit: 20 };
let rows = [];
let tbody = null;
let counts = {};

function initials(customer) {
  const source = customer.username || customer.email || '?';
  return source.slice(0, 2).toUpperCase();
}

/* ── Rows ─────────────────────────────────────────────────────  */

function customerRow(customer) {
  const tr = document.createElement('tr');
  tr.dataset.id = customer.user_id;
  tr.style.cursor = 'pointer';

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.className = 'admin-check';
  check.setAttribute('data-row-check', '');
  check.setAttribute('aria-label', 'Select ' + (customer.username || customer.email));
  tr.appendChild(cell(check));

  const media = document.createElement('div');
  media.className = 'cell-media';
  const avatar = document.createElement('div');
  avatar.className = 'cell-avatar';
  avatar.textContent = initials(customer);
  const text = document.createElement('div');
  const name = document.createElement('div');
  name.className = 'cell-strong';
  name.textContent = customer.username || '—';
  const email = document.createElement('div');
  email.className = 'cell-sub';
  email.textContent = customer.email || '';
  text.appendChild(name);
  text.appendChild(email);
  media.appendChild(avatar);
  media.appendChild(text);
  tr.appendChild(cell(media));

  tr.appendChild(cell(customer.location || '—'));
  tr.appendChild(cell(String(customer.order_count || 0), 'num'));
  tr.appendChild(cell(money(customer.lifetime_spend || 0), 'num'));
  tr.appendChild(cell(customer.last_order_at ? fullDate(customer.last_order_at) : 'Never', 'nowrap'));

  const segment = customer.segment || 'New';
  tr.appendChild(cell(tag(segment === 'Vip' ? 'VIP' : segment, SEGMENT_TAG[segment] || 'tag-blue')));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Open', () => openCustomer(customer.user_id)));
  tr.appendChild(cell(actions, 'col-actions'));

  tr.addEventListener('click', (e) => {
    if (e.target.closest('input, .row-btn')) return;
    openCustomer(customer.user_id);
  });

  return tr;
}

/* ── Drawer ───────────────────────────────────────────────────  */

function line(label, value) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;justify-content:space-between;gap:16px;font-size:13px;padding:3px 0;';
  const left = document.createElement('span');
  left.style.color = 'var(--subtle)';
  left.textContent = label;
  const right = document.createElement('span');
  right.style.cssText = 'color:var(--dark);text-align:right;';
  if (value instanceof Node) right.appendChild(value);
  else right.textContent = value === null || value === undefined || value === '' ? '—' : value;
  row.appendChild(left);
  row.appendChild(right);
  return row;
}

function section(title) {
  const box = document.createElement('div');
  box.style.cssText = 'margin-top:18px;padding-top:16px;border-top:1px solid var(--border);';
  const heading = document.createElement('div');
  heading.style.cssText = 'font-size:12px;font-weight:700;text-transform:uppercase;'
    + 'letter-spacing:.04em;color:var(--subtle);margin-bottom:10px;';
  heading.textContent = title;
  box.appendChild(heading);
  return box;
}

async function openCustomer(userId) {
  const response = await Fetcher.get({ endpoint: '/v1/admin/customer/' + userId });
  if (!response.ok) return;

  const customer = response.data;
  const body = document.createElement('div');

  const segment = customer.segment || 'New';
  const head = document.createElement('div');
  head.style.cssText = 'display:flex;gap:8px;margin-bottom:16px;';
  head.appendChild(tag(segment === 'Vip' ? 'VIP' : segment, SEGMENT_TAG[segment] || 'tag-blue'));
  if (customer.accepts_marketing) head.appendChild(tag('Accepts marketing', 'tag-green'));
  body.appendChild(head);

  const fullName = [customer.first_name, customer.last_name].filter(Boolean).join(' ');
  body.appendChild(line('Name', fullName || customer.username));
  body.appendChild(line('Email', customer.email));
  body.appendChild(line('Phone', customer.phone_number));
  body.appendChild(line('Location', customer.location));
  body.appendChild(line('Joined', fullDate(customer.created_at)));

  const commerce = section('Commerce');
  commerce.appendChild(line('Orders', String(customer.order_count || 0)));
  commerce.appendChild(line('Lifetime spend', money(customer.lifetime_spend || 0)));
  commerce.appendChild(line('Loyalty points', String(customer.loyalty_points || 0)));
  body.appendChild(commerce);

  const orders = customer.recent_orders || [];
  const recent = section(orders.length ? 'Recent orders' : 'No orders yet');
  orders.forEach((order) => {
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;justify-content:space-between;align-items:center;'
      + 'gap:12px;padding:6px 0;cursor:pointer;';
    row.addEventListener('click', () => {
      location.href = '/admin/orders?q=' + encodeURIComponent(order.number);
    });

    const left = document.createElement('div');
    const number = document.createElement('div');
    number.style.cssText = 'font-size:13px;color:var(--dark);';
    number.textContent = '#' + order.number;
    const when = document.createElement('div');
    when.className = 'cell-sub';
    when.textContent = dateTime(order.placed_at);
    left.appendChild(number);
    left.appendChild(when);

    const right = document.createElement('div');
    right.style.cssText = 'display:flex;align-items:center;gap:10px;';
    right.appendChild(orderTag(order.fulfilment_status));
    const total = document.createElement('span');
    total.style.cssText = 'font-size:13px;font-weight:700;color:var(--dark);';
    total.textContent = money(order.grand_total);
    right.appendChild(total);

    row.appendChild(left);
    row.appendChild(right);
    recent.appendChild(row);
  });
  body.appendChild(recent);

  /* The two writable fields. `customer/update` takes `note` and
     `tags` and nothing else, so this is the whole of what staff can
     change about a customer from here. */
  const staffBox = section('Staff notes');

  const tagsInput = textInput({
    value: (customer.tags || []).join(', '),
    placeholder: 'wholesale, press, careful-packer',
  });
  staffBox.appendChild(field('Tags', tagsInput, 'Comma separated. Internal only.'));

  const note = textArea({ value: customer.note || '', rows: 4, placeholder: 'Anything the team should know…' });
  staffBox.appendChild(field('Note', note));
  body.appendChild(staffBox);

  openModal({
    title: customer.username || customer.email,
    body,
    width: '560px',
    actions: [
      { label: 'Close', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Save notes',
        className: 'btn-primary',
        onClick: async (close, button) => {
          button.disabled = true;
          const saved = await Fetcher.patch({
            endpoint: '/v1/admin/customer/' + userId,
            body: {
              note: note.value.trim() || null,
              tags: tagsInput.value.split(',').map((t) => t.trim()).filter(Boolean),
            },
          });
          button.disabled = false;
          if (!saved.ok) return;

          close();
          toast.success('Saved');
          load();
          loadCounts();
        },
      },
    ],
  });
}

/* ── Bulk tagging ─────────────────────────────────────────────  */

function selectedIds() {
  return $$('[data-row-check]:checked').map((box) => box.closest('tr').dataset.id).filter(Boolean);
}

function bulkTag() {
  const ids = selectedIds();
  if (!ids.length) return;

  const value = textInput({ placeholder: 'wholesale' });
  const form = document.createElement('div');
  form.appendChild(field('Tag to add', value,
    'Added to whatever tags each customer already has.'));

  openModal({
    title: 'Tag ' + ids.length + (ids.length === 1 ? ' customer' : ' customers'),
    body: form,
    width: '440px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Add tag',
        className: 'btn-primary',
        onClick: async (close, button) => {
          const label = value.value.trim();
          if (!label) return;

          button.disabled = true;
          let failed = 0;

          /* `customer/update` replaces the tag array rather than
             appending, so each row's existing tags have to be read
             back before writing — one GET and one PATCH per person.
             Acceptable for a page of 20; it is why this only ever
             acts on the selection. */
          for (const id of ids) {
            const current = await Fetcher.get({ endpoint: '/v1/admin/customer/' + id, showError: false });
            if (!current.ok) { failed++; continue; }

            const existing = current.data.tags || [];
            if (existing.includes(label)) continue;

            const saved = await Fetcher.patch({
              endpoint: '/v1/admin/customer/' + id,
              body: { tags: [...existing, label] },
              showError: false,
            });
            if (!saved.ok) failed++;
          }

          button.disabled = false;
          close();

          if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be tagged");
          else toast.success('Tagged');
          load();
        },
      },
    ],
  });
}

/* ── Tiles, tabs, export ──────────────────────────────────────  */

function paintStats(stats) {
  const tiles = $$('.stat-tile');
  const values = [
    (stats.total_customers || 0).toLocaleString(),
    Math.round(stats.repeat_rate || 0) + '%',
    money(stats.avg_lifetime_value || 0),
    (stats.vip_count || 0).toLocaleString(),
  ];
  const feet = [
    'Everyone with an account',
    'Have ordered more than once',
    'Mean across every customer',
    'Spend over $1,000',
  ];

  tiles.forEach((tile, index) => {
    const value = tile.querySelector('.stat-tile-value');
    if (value) value.textContent = values[index];
    const foot = tile.querySelector('.stat-tile-foot');
    if (foot) { foot.innerHTML = ''; foot.textContent = feet[index]; }
  });
}

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;
  host.innerHTML = '';

  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.segment ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;

    if (counts[value] !== undefined) {
      const count = document.createElement('span');
      count.className = 'admin-tab-count';
      count.textContent = counts[value].toLocaleString();
      el.appendChild(count);
    }

    const pick = () => { state.segment = value; state.page = 1; load(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

async function loadCounts() {
  const responses = await Promise.all(TABS.map(({ value }) =>
    Fetcher.get({
      endpoint: '/v1/admin/customer/list',
      query: { segment: value, limit: 1 },
      showError: false,
    })));

  TABS.forEach(({ value }, index) => {
    counts[value] = responses[index].ok ? responses[index].data.total : 0;
  });
  paintTabs();
}

function exportCsv() {
  if (!rows.length) return;
  const header = ['Username', 'Email', 'Location', 'Orders', 'Lifetime spend', 'Last order', 'Segment'];
  const lines = [header, ...rows.map((customer) => [
    customer.username || '',
    customer.email || '',
    customer.location || '',
    customer.order_count || 0,
    ((customer.lifetime_spend || 0) / 100).toFixed(2),
    dateValue(customer.last_order_at),
    customer.segment || 'New',
  ])];

  const csv = lines
    .map((row) => row.map((value) => '"' + String(value).replace(/"/g, '""') + '"').join(','))
    .join('\n');

  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'bambe-customers-page-' + state.page + '.csv';
  link.click();
  URL.revokeObjectURL(url);
  toast.success('This page saved as CSV');
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/admin/customer/list',
    query: {
      segment: state.segment,
      location: state.location,
      page: state.page,
      limit: state.limit,
    },
  });

  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load customers" });
    return;
  }

  const { items, total, limit, page, stats } = response.data;
  paintStats(stats || {});

  // The endpoint has no `q`; this narrows the page on screen only.
  const needle = state.q.toLowerCase();
  rows = (items || []).filter((customer) => !needle
    || (customer.username || '').toLowerCase().includes(needle)
    || (customer.email || '').toLowerCase().includes(needle));

  fillTable(tbody, rows.map(customerRow), {
    empty: state.q ? 'Nobody on this page matches “' + state.q + '”' : 'No customers in this segment',
  });
  paintPager($('.pagination'), { total, limit, page }, (next) => { state.page = next; load(); });

  const subtitle = $('.admin-subtitle');
  if (subtitle && stats) {
    subtitle.textContent = (stats.total_customers || 0).toLocaleString() + ' total · '
      + (stats.vip_count || 0) + ' VIP';
  }

  const all = $('[data-check-all]');
  if (all) { all.checked = false; all.indeterminate = false; }
  const bulkBar = $('[data-bulk]');
  if (bulkBar) bulkBar.classList.remove('visible');
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  tbody = $('.admin-table tbody');

  const [locationSelect, sortSelect] = $$('.admin-toolbar select.admin-filter');

  /* `location` on the endpoint is a case-insensitive regex over
     whatever string the customer profile holds, so the filter is a
     free text box rather than a fixed list of countries the store
     may not sell to. */
  if (locationSelect) {
    const box = document.createElement('input');
    box.type = 'text';
    box.className = 'admin-filter';
    box.placeholder = 'Filter by location…';
    box.setAttribute('aria-label', 'Filter by location');
    box.addEventListener('input', debounce(() => {
      state.location = box.value.trim();
      state.page = 1;
      load();
    }, 350));
    // The skin button select.js put in front of it has to go too.
    const skin = locationSelect.previousElementSibling;
    if (skin && skin.classList.contains('select-btn')) skin.remove();
    locationSelect.replaceWith(box);
  }

  // The pipeline sorts by order count and takes no sort parameter.
  if (sortSelect) {
    const skin = sortSelect.previousElementSibling;
    if (skin && skin.classList.contains('select-btn')) skin.remove();
    sortSelect.remove();
  }

  const search = $('.admin-search input');
  if (search) {
    search.placeholder = 'Filter this page by name or email…';
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      load();
    }, 200));
  }

  const exportButton = $('.admin-head-actions button');
  if (exportButton) {
    exportButton.textContent = 'Export CSV';
    exportButton.addEventListener('click', exportCsv);
  }

  const [addTag, emailSelected, removeSelected] = $$('[data-bulk] button');
  if (addTag) addTag.addEventListener('click', bulkTag);
  if (emailSelected) emailSelected.remove();     // no mail-out endpoint
  if (removeSelected) removeSelected.remove();   // deletion is a customer request

  loadCounts();
  load();
})();
