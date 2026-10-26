/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-orders.js

   The fulfilment queue. Six tabs, a search, payment and date
   filters, and a drawer per order carrying everything the five
   order endpoints can do: read it, move its status, attach
   tracking, and refund it.

   The three tiles above the table come from the list endpoint's
   own `stats` block, so they are the real counts across the whole
   collection rather than of the page on screen.

   What the wireframe drew and the API cannot do:

   - **Create order** — there is no admin order-creation endpoint;
     orders come from checkout. Removed.
   - **Print labels** — no label provider is connected. Removed.
   - **Export** — no endpoint; it saves the rows on screen as CSV,
     and is labelled Export CSV so it doesn't promise a report.
   - **A "Refunded" tab** — `fulfilment_status` has no Refunded
     variant; refunds live on `payment_status`. That tab now filters
     on the payment side, which is what it was always describing.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, dateTime, fullDate, dateValue, spaced,
  cell, fillTable, paintPager, paintShowing, debounce, rowButton,
  orderTag, paymentTag, tag, mountAdminChrome,
  openModal, confirmModal, field, textInput, textArea, selectInput,
} from './admin-core.js';
import { paintImage } from './catalog.js';

/* Tabs are two different filters wearing one row: five are
   fulfilment states, the last is a payment state. */
const TABS = [
  { value: 'all', label: 'All', query: {} },
  { value: 'pending', label: 'Pending', query: { status: 'pending' } },
  { value: 'packing', label: 'Packing', query: { status: 'packing' } },
  { value: 'shipped', label: 'Shipped', query: { status: 'shipped' } },
  { value: 'delivered', label: 'Delivered', query: { status: 'delivered' } },
  { value: 'refunded', label: 'Refunded', query: { payment_status: 'Refunded' } },
];

const FULFILMENT = ['Pending', 'Packing', 'Shipped', 'Delivered', 'OnHold', 'Cancelled', 'Returned'];
const PAYMENTS = ['', 'Paid', 'Pending', 'Refunded', 'PartiallyRefunded', 'Failed'];
const RANGES = [
  { value: '', label: 'Any time' },
  { value: '7', label: 'Last 7 days' },
  { value: '30', label: 'Last 30 days' },
  { value: '90', label: 'This quarter' },
];

const DAY = 24 * 60 * 60 * 1000;

let state = { tab: 'all', q: '', payment: '', days: '', page: 1, limit: 20 };
let tbody = null;
let rows = [];
let counts = {};

/* ── Rows ─────────────────────────────────────────────────────  */

function orderRow(order) {
  const tr = document.createElement('tr');
  tr.dataset.id = order.uuid;

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.className = 'admin-check';
  check.setAttribute('data-row-check', '');
  check.setAttribute('aria-label', 'Select order ' + order.number);
  tr.appendChild(cell(check));

  const idCell = document.createElement('div');
  const number = document.createElement('span');
  number.className = 'cell-id';
  number.textContent = '#' + order.number;
  idCell.appendChild(number);
  const delivery = document.createElement('div');
  delivery.className = 'cell-sub';
  delivery.textContent = order.delivery ? order.delivery.name : '';
  idCell.appendChild(delivery);
  tr.appendChild(cell(idCell));

  /* The list returns whole order documents, and the only name on one
     is the shipping recipient — which is also what the endpoint's own
     `q` searches, so the column and the search agree. */
  const who = document.createElement('div');
  const name = document.createElement('div');
  name.className = 'cell-strong';
  name.textContent = order.shipping_address ? order.shipping_address.recipient_name : '—';
  who.appendChild(name);
  const where = document.createElement('div');
  where.className = 'cell-sub';
  where.textContent = order.shipping_address
    ? [order.shipping_address.city, order.shipping_address.country].filter(Boolean).join(', ')
    : '';
  who.appendChild(where);
  tr.appendChild(cell(who));

  tr.appendChild(cell(dateTime(order.placed_at), 'nowrap'));
  tr.appendChild(cell(paymentTag(order.payment_status)));
  tr.appendChild(cell(orderTag(order.fulfilment_status)));
  tr.appendChild(cell(money(order.totals.grand_total), 'num'));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Open', () => openOrder(order.uuid)));
  tr.appendChild(cell(actions, 'col-actions'));

  tr.style.cursor = 'pointer';
  tr.addEventListener('click', (e) => {
    if (e.target.closest('input, .row-btn')) return;
    openOrder(order.uuid);
  });

  return tr;
}

/* ── The order drawer ─────────────────────────────────────────  */

function line(label, value) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;justify-content:space-between;gap:16px;font-size:13px;padding:3px 0;';
  const left = document.createElement('span');
  left.style.color = 'var(--subtle)';
  left.textContent = label;
  const right = document.createElement('span');
  right.style.cssText = 'color:var(--dark);text-align:right;';
  if (value instanceof Node) right.appendChild(value);
  else right.textContent = value === undefined || value === null ? '—' : value;
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

async function openOrder(orderId) {
  const response = await Fetcher.get({ endpoint: '/v1/admin/order/' + orderId });
  if (!response.ok) return;

  const { order, customer, items, shipment_events: events } = response.data;
  const body = document.createElement('div');

  /* Header: the two states, side by side, because every action
     below is gated on one or the other. */
  const states = document.createElement('div');
  states.style.cssText = 'display:flex;gap:8px;margin-bottom:16px;';
  states.appendChild(orderTag(order.fulfilment_status));
  states.appendChild(paymentTag(order.payment_status));
  body.appendChild(states);

  body.appendChild(line('Placed', dateTime(order.placed_at)));
  body.appendChild(line('Customer', customer ? customer.username : '—'));
  body.appendChild(line('Email', customer ? customer.email : '—'));
  body.appendChild(line('Delivery', order.delivery ? order.delivery.name : '—'));
  if (order.packaging) body.appendChild(line('Packaging', order.packaging.name));
  if (order.coupon) body.appendChild(line('Coupon', order.coupon.code));

  const address = section('Ship to');
  const addressText = document.createElement('div');
  addressText.style.cssText = 'font-size:13px;color:var(--dark);line-height:1.7;white-space:pre-line;';
  const shipping = order.shipping_address || {};
  addressText.textContent = [
    shipping.recipient_name,
    shipping.street,
    [shipping.city, shipping.state, shipping.zip].filter(Boolean).join(', '),
    shipping.country,
    shipping.phone,
  ].filter(Boolean).join('\n');
  address.appendChild(addressText);
  body.appendChild(address);

  const lines = section(items.length + (items.length === 1 ? ' item' : ' items'));
  items.forEach((item) => {
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;gap:10px;align-items:center;padding:6px 0;';

    const thumb = document.createElement('div');
    thumb.style.cssText = 'width:36px;height:36px;border-radius:var(--radius-sm);'
      + 'background:var(--input-bg);flex-shrink:0;';
    if (item.image && item.image.uuid) {
      paintImage(thumb, item.image);
    }

    const text = document.createElement('div');
    text.style.flex = '1';
    const title = document.createElement('div');
    title.style.cssText = 'font-size:13px;color:var(--dark);';
    title.textContent = item.title;
    const meta = document.createElement('div');
    meta.className = 'cell-sub';
    meta.textContent = [item.variant_label, item.sku, '×' + item.quantity].filter(Boolean).join(' · ');
    text.appendChild(title);
    text.appendChild(meta);

    const total = document.createElement('div');
    total.style.cssText = 'font-size:13px;font-weight:700;color:var(--dark);';
    total.textContent = money(item.line_total);

    row.appendChild(thumb);
    row.appendChild(text);
    row.appendChild(total);
    lines.appendChild(row);
  });
  body.appendChild(lines);

  const totals = section('Totals');
  totals.appendChild(line('Subtotal', money(order.totals.subtotal)));
  if (order.totals.discount) totals.appendChild(line('Discount', '−' + money(order.totals.discount)));
  totals.appendChild(line('Shipping', money(order.totals.shipping)));
  if (order.totals.packaging) totals.appendChild(line('Packaging', money(order.totals.packaging)));
  totals.appendChild(line('Tax', money(order.totals.tax)));
  const grand = line('Total', money(order.totals.grand_total));
  grand.style.fontWeight = '700';
  totals.appendChild(grand);
  body.appendChild(totals);

  if (order.tracking) {
    const track = section('Tracking');
    track.appendChild(line('Carrier', order.tracking.carrier));
    track.appendChild(line('Number', order.tracking.tracking_number));
    if (order.tracking.estimated_delivery) {
      track.appendChild(line('Estimated', fullDate(order.tracking.estimated_delivery)));
    }
    body.appendChild(track);
  }

  if (events && events.length) {
    const timeline = section('Timeline');
    events.forEach((event) => {
      timeline.appendChild(line(
        new Date(event.at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }),
        [event.status, event.location].filter(Boolean).join(' · ')));
    });
    body.appendChild(timeline);
  }

  /* Refund is offered only while there is something left to refund —
     `refund.rs` answers 400 otherwise, and an operator should not
     have to click to find that out. */
  const paid = (order.payment_status === 'Paid' || order.payment_status === 'PartiallyRefunded')
    && response.data.refundable > 0;

  const actions = [
    { label: 'Close', className: 'btn-ghost', onClick: (close) => close() },
    { label: 'Tracking', className: 'btn-outline', onClick: (close) => { close(); editTracking(order); } },
    { label: 'Status', className: 'btn-primary', onClick: (close) => { close(); editStatus(order); } },
  ];
  if (paid) {
    actions.splice(1, 0, {
      label: 'Refund',
      className: 'btn-danger',
      onClick: (close) => { close(); refund(order, response.data.refundable, response.data.refunded_total); },
    });
  }

  openModal({ title: 'Order #' + order.number, body, width: '560px', actions });
}

/* ── Status ───────────────────────────────────────────────────  */

function editStatus(order) {
  const form = document.createElement('div');

  const select = selectInput(
    FULFILMENT.map((value) => ({ value, label: spaced(value) })),
    order.fulfilment_status);
  form.appendChild(field('Fulfilment status', select));

  const note = document.createElement('div');
  note.style.cssText = 'font-size:12.5px;color:var(--subtle);line-height:1.6;';
  note.textContent = 'Moving an order to Cancelled puts its stock back and reverses the '
    + 'sold count. Shipped and Delivered stamp their timestamps the first time they are set.';
  form.appendChild(note);

  openModal({
    title: 'Move order #' + order.number,
    body: form,
    width: '460px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Update',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (select.value === order.fulfilment_status) { close(); return; }

          if (select.value === 'Cancelled') {
            const sure = await confirmModal({
              title: 'Cancel this order?',
              message: 'Stock goes back to the shelf and the sold count is reversed. '
                + 'The customer is not automatically refunded — do that separately.',
              confirmLabel: 'Cancel order',
            });
            if (!sure) return;
          }

          button.disabled = true;
          const response = await Fetcher.patch({
            endpoint: '/v1/admin/order/' + order.uuid + '/status',
            body: { fulfilment_status: select.value },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Order moved to ' + spaced(select.value));
          load();
          loadCounts();
        },
      },
    ],
  });
}

/* ── Tracking ─────────────────────────────────────────────────  */

function editTracking(order) {
  const existing = order.tracking || {};
  const form = document.createElement('div');

  const carrier = textInput({ placeholder: 'DHL, Royal Mail, …', value: existing.carrier || '' });
  form.appendChild(field('Carrier', carrier));

  const number = textInput({ placeholder: 'Tracking number', value: existing.tracking_number || '' });
  form.appendChild(field('Tracking number', number));

  const url = textInput({ placeholder: 'https://… (optional)', value: existing.tracking_url || '' });
  form.appendChild(field('Tracking URL', url));

  const eta = document.createElement('input');
  eta.type = 'date';
  eta.className = 'form-input';
  eta.value = dateValue(existing.estimated_delivery);
  form.appendChild(field('Estimated delivery', eta));

  const eventStatus = textInput({ placeholder: 'Picked up, Out for delivery, … (optional)' });
  form.appendChild(field('Add a timeline entry', eventStatus));

  const eventWhere = textInput({ placeholder: 'Location (optional)' });
  form.appendChild(field('Where', eventWhere,
    'The timeline entry is what the customer sees on their tracking page.'));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  openModal({
    title: 'Tracking for #' + order.number,
    body: form,
    width: '520px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Save',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (!carrier.value.trim() || !number.value.trim()) {
            error.textContent = 'A carrier and a tracking number are both required.';
            error.style.display = '';
            return;
          }

          const payload = {
            carrier: carrier.value.trim(),
            tracking_number: number.value.trim(),
            tracking_url: url.value.trim() || null,
            estimated_delivery: eta.value ? new Date(eta.value + 'T12:00:00').getTime() : null,
          };
          if (eventStatus.value.trim()) {
            payload.event = {
              status: eventStatus.value.trim(),
              description: null,
              location: eventWhere.value.trim() || null,
            };
          }

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/admin/order/' + order.uuid + '/tracking',
            body: payload,
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Tracking updated');
          load();
        },
      },
    ],
  });
}

/* ── Refund ───────────────────────────────────────────────────  */

function refund(order, remaining, refunded) {

  const form = document.createElement('div');

  const summary = document.createElement('div');
  summary.style.cssText = 'font-size:13px;color:var(--subtle);line-height:1.7;margin-bottom:14px;';
  summary.textContent = 'Order total ' + money(order.totals.grand_total)
    + (refunded ? ' · already refunded ' + money(refunded) : '')
    + ' · up to ' + money(remaining) + ' can be refunded.';
  form.appendChild(summary);

  const amount = textInput({ placeholder: (remaining / 100).toFixed(2) });
  form.appendChild(field('Amount', amount, 'Leave blank to refund everything outstanding.'));

  const reason = textArea({ placeholder: 'Why? (kept on the transaction)', rows: 3 });
  form.appendChild(field('Reason', reason));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  openModal({
    title: 'Refund #' + order.number,
    body: form,
    width: '460px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Issue refund',
        className: 'btn-danger',
        onClick: async (close, button) => {
          let cents = null;
          if (amount.value.trim()) {
            const parsed = Number(amount.value.trim());
            if (!Number.isFinite(parsed) || parsed <= 0) {
              error.textContent = 'Enter an amount in dollars, or leave it blank.';
              error.style.display = '';
              return;
            }
            cents = Math.round(parsed * 100);
            if (cents > remaining) {
              error.textContent = 'That is more than the ' + money(remaining) + ' outstanding.';
              error.style.display = '';
              return;
            }
          }

          const sure = await confirmModal({
            title: 'Refund ' + money(cents === null ? remaining : cents) + '?',
            message: 'This records the refund against the order. Money is not moved — no '
              + 'payment processor is connected to this store yet.',
            confirmLabel: 'Refund',
          });
          if (!sure) return;

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/admin/order/' + order.uuid + '/refund',
            body: { amount: cents, reason: reason.value.trim() || null },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Refund recorded');
          load();
          loadCounts();
        },
      },
    ],
  });
}

/* ── Bulk ─────────────────────────────────────────────────────  */

function selectedIds() {
  return $$('[data-row-check]:checked').map((box) => box.closest('tr').dataset.id).filter(Boolean);
}

async function bulkStatus(status, label) {
  const ids = selectedIds();
  if (!ids.length) return;

  const sure = await confirmModal({
    title: label + ' ' + ids.length + (ids.length === 1 ? ' order?' : ' orders?'),
    message: 'One request per order — there is no bulk endpoint behind it.'
      + (status === 'Cancelled' ? ' Cancelling also restocks every line.' : ''),
    confirmLabel: label,
    danger: status === 'Cancelled',
  });
  if (!sure) return;

  let failed = 0;
  for (const id of ids) {
    const response = await Fetcher.patch({
      endpoint: '/v1/admin/order/' + id + '/status',
      body: { fulfilment_status: status },
      showError: false,
    });
    if (!response.ok) failed++;
  }

  if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be updated");
  else toast.success(ids.length + (ids.length === 1 ? ' order updated' : ' orders updated'));
  load();
  loadCounts();
}

/* ── Export ───────────────────────────────────────────────────  */

function exportCsv() {
  if (!rows.length) return;
  const header = ['Order', 'Placed', 'Customer', 'Payment', 'Fulfilment', 'Total'];
  const lines = [header, ...rows.map((order) => [
    order.number,
    new Date(order.placed_at).toISOString(),
    order.shipping_address ? order.shipping_address.recipient_name : '',
    order.payment_status,
    order.fulfilment_status,
    (order.totals.grand_total / 100).toFixed(2),
  ])];

  const csv = lines
    .map((row) => row.map((value) => '"' + String(value).replace(/"/g, '""') + '"').join(','))
    .join('\n');

  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'bambee-orders-page-' + state.page + '.csv';
  link.click();
  URL.revokeObjectURL(url);
  toast.success('This page saved as CSV');
}

/* ── Toolbar ──────────────────────────────────────────────────  */

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;
  host.innerHTML = '';

  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.tab ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;

    if (counts[value] !== undefined) {
      const count = document.createElement('span');
      count.className = 'admin-tab-count';
      count.textContent = counts[value].toLocaleString();
      el.appendChild(count);
    }

    const pick = () => { state.tab = value; state.page = 1; paintTabs(); load(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

async function loadCounts() {
  const responses = await Promise.all(TABS.map(({ query }) =>
    Fetcher.get({
      endpoint: '/v1/admin/order/list',
      query: { ...query, limit: 1 },
      showError: false,
    })));

  TABS.forEach(({ value }, index) => {
    counts[value] = responses[index].ok ? responses[index].data.total : 0;
  });
  paintTabs();
}

function paintStats(stats) {
  const tiles = $$('.stat-tile');
  const values = [stats.to_fulfil, stats.in_transit, stats.returns];
  const feet = [
    'Pending or packing, right now',
    'Shipped and not yet delivered',
    'Requested, approved or on the way back',
  ];

  tiles.forEach((tile, index) => {
    const value = tile.querySelector('.stat-tile-value');
    if (value) value.textContent = (values[index] || 0).toLocaleString();
    const foot = tile.querySelector('.stat-tile-foot');
    if (foot) { foot.innerHTML = ''; foot.textContent = feet[index]; }
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const tab = TABS.find((t) => t.value === state.tab) || TABS[0];

  const query = {
    ...tab.query,
    q: state.q,
    page: state.page,
    limit: state.limit,
  };
  if (state.payment) query.payment_status = state.payment;
  if (state.days) query.from = Date.now() - Number(state.days) * DAY;

  const response = await Fetcher.get({ endpoint: '/v1/admin/order/list', query });

  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load orders" });
    { const showing = $('.admin-table-foot > span'); if (showing) showing.textContent = ''; }
    return;
  }

  const { items, total, limit, page, stats } = response.data;
  rows = items;

  paintStats(stats || {});
  fillTable(tbody, items.map(orderRow), {
    empty: state.q ? 'No orders match “' + state.q + '”' : 'Nothing in this tab',
  });
  paintPager($('.pagination'), { total, limit, page }, (next) => { state.page = next; load(); });
  paintShowing($('.admin-table-foot > span'), { total, limit, page }, 'orders');

  const subtitle = $('.admin-subtitle');
  if (subtitle && stats) {
    subtitle.textContent = stats.to_fulfil + ' awaiting action · '
      + total.toLocaleString() + (total === 1 ? ' order' : ' orders') + ' in this view';
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

  const params = new URLSearchParams(location.search);
  if (params.get('q')) state.q = params.get('q');

  const [paymentSelect, rangeSelect] = $$('.admin-toolbar select.admin-filter');

  if (paymentSelect) {
    paymentSelect.innerHTML = '';
    PAYMENTS.forEach((value) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = value ? spaced(value) : 'All payments';
      paymentSelect.appendChild(option);
    });
    paymentSelect.addEventListener('change', () => {
      state.payment = paymentSelect.value;
      state.page = 1;
      load();
    });
  }

  if (rangeSelect) {
    rangeSelect.innerHTML = '';
    RANGES.forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      rangeSelect.appendChild(option);
    });
    rangeSelect.addEventListener('change', () => {
      state.days = rangeSelect.value;
      state.page = 1;
      load();
    });
  }

  const search = $('.admin-search input');
  if (search) {
    search.value = state.q;
    search.placeholder = 'Search by order number or recipient…';
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      state.page = 1;
      load();
    }, 300));
  }

  const [exportButton, createButton] = $$('.admin-head-actions button');
  if (exportButton) {
    exportButton.textContent = 'Export CSV';
    exportButton.addEventListener('click', exportCsv);
  }
  // Orders are created by checkout. There is no admin endpoint that
  // makes one, and a form that posts nowhere is worse than no button.
  if (createButton) createButton.remove();

  const [printLabels, markFulfilled, cancelAll] = $$('[data-bulk] button');
  if (printLabels) printLabels.remove();   // no label provider connected
  if (markFulfilled) {
    markFulfilled.textContent = 'Mark shipped';
    markFulfilled.addEventListener('click', () => bulkStatus('Shipped', 'Mark shipped'));
  }
  if (cancelAll) {
    cancelAll.addEventListener('click', () => bulkStatus('Cancelled', 'Cancel'));
  }

  loadCounts();
  load();
})();
