/* ═══════════════════════════════════════════════════════════════
   Bambe Web — returns.js

   `GET /v1/return/list` and `POST /v1/return` were both complete
   and had no page at all: a customer with a damaged delivery had
   nowhere to say so. This is that page, plus the composer the
   orders page opens from a delivered order's own card.

   What the backend will and won't accept shapes the whole screen
   (return_request/create.rs):

   - only a Delivered order can be returned, so the order picker
     asks for `status=delivered` and nothing else is offered;
   - a line can be returned at most `quantity - refunded_quantity`
     times, so each stepper's ceiling is that number and a line
     with nothing left is shown struck through rather than hidden,
     because "where did my other item go" is a worse question;
   - `reason` is a free string, so the select's labels are what
     gets stored — the admin console reads them verbatim.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { money, imageUrl, emptyState, hideSkeleton } from './catalog.js';
import { openModal, field, textArea, selectInput } from './modal.js';
import { mountAccountChrome } from './account.js';

const $ = (sel) => document.querySelector(sel);

/* The lifecycle in ReturnStatus (model/order.rs), mapped onto the
   pills the stylesheet already has. */
const PILL = {
  Requested: 'status-processing',
  Approved: 'status-shipped',
  InTransit: 'status-shipped',
  Received: 'status-shipped',
  Refunded: 'status-delivered',
  Rejected: 'status-cancelled',
};

const STATUS_LABEL = {
  Requested: 'Requested',
  Approved: 'Approved',
  InTransit: 'On its way back',
  Received: 'Received',
  Refunded: 'Refunded',
  Rejected: 'Declined',
};

const STATUS_NOTE = {
  Requested: 'We are reviewing your request.',
  Approved: 'Approved — post it back with the label we emailed you.',
  InTransit: 'On its way back to us.',
  Received: 'We have it. Your refund is being processed.',
  Refunded: 'Refunded.',
  Rejected: 'This request was declined.',
};

const REASONS = [
  'Arrived damaged',
  'Faulty or stopped working',
  'Wrong item sent',
  'Not as described',
  "Doesn't fit",
  'Changed my mind',
  'Other',
];

let list = null;

function date(ms) {
  return new Date(ms).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
}

/* ── A return card ────────────────────────────────────────────  */

function returnCard(request) {
  const card = document.createElement('div');
  card.className = 'order-card';

  card.innerHTML =
    '<div class="order-card-header">'
      + '<div><div class="order-id-label">Return for order</div><div class="order-id"></div></div>'
      + '<div class="order-date"></div>'
      + '<div class="status-pill"></div>'
    + '</div>'
    + '<div class="order-card-body">'
      + '<div class="order-meta">'
        + '<div class="order-meta-item return-reason"></div>'
        + '<div class="order-meta-item return-note"></div>'
      + '</div>'
    + '</div>'
    + '<div class="order-card-footer">'
      + '<div><div class="order-total-lbl"></div><div class="order-total"></div></div>'
      + '<div class="order-actions"></div>'
    + '</div>';

  card.querySelector('.order-id').textContent =
    request.order_number ? '#' + request.order_number : '—';
  card.querySelector('.order-date').textContent = date(request.requested_at)
    + ' · ' + unitCount(request);

  const pill = card.querySelector('.status-pill');
  pill.classList.add(PILL[request.status] || 'status-processing');
  pill.textContent = STATUS_LABEL[request.status] || request.status;

  card.querySelector('.return-reason').textContent = request.reason || '';
  card.querySelector('.return-note').textContent = STATUS_NOTE[request.status] || '';

  /* `refund_amount` is only set once a decision has been made, so an
     open request shows nothing rather than "$0.00 refunded". */
  const total = card.querySelector('.order-total');
  const label = card.querySelector('.order-total-lbl');
  if (request.refund_amount !== null && request.refund_amount !== undefined) {
    label.textContent = request.status === 'Refunded' ? 'Refunded' : 'Refund due';
    total.textContent = money(request.refund_amount);
    total.style.color = 'var(--success)';
  } else {
    label.textContent = 'Refund';
    total.textContent = 'Once approved';
    total.style.fontSize = '13px';
    total.style.color = 'var(--subtle)';
  }

  if (request.order_id) {
    const view = document.createElement('button');
    view.className = 'btn btn-ghost btn-sm';
    view.textContent = 'View Order';
    view.addEventListener('click', () => {
      location.href = '/checkout/confirmation?id=' + encodeURIComponent(request.order_id);
    });
    card.querySelector('.order-actions').appendChild(view);
  }

  if (request.details) {
    const details = document.createElement('div');
    details.style.cssText = 'padding:0 20px 16px;font-size:13px;color:var(--subtle);line-height:1.6;';
    details.textContent = request.details;
    card.querySelector('.order-card-body').appendChild(details);
  }

  return card;
}

function unitCount(request) {
  const units = (request.items || []).reduce((sum, line) => sum + (line.quantity || 0), 0);
  return units === 1 ? '1 item' : units + ' items';
}

/* ── Composer ─────────────────────────────────────────────────  */

function lineRow(item, onChange) {
  const remaining = (item.quantity || 0) - (item.refunded_quantity || 0);

  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:12px;align-items:center;padding:10px 0;'
    + 'border-bottom:1px solid var(--border);';

  const thumb = document.createElement('div');
  thumb.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
    + 'background:var(--input-bg);flex-shrink:0;';
  const url = imageUrl(item.image);
  if (url) thumb.style.background = "url('" + url + "') center / cover no-repeat";

  const text = document.createElement('div');
  text.style.cssText = 'flex:1;min-width:0;';
  const title = document.createElement('div');
  title.style.cssText = 'font-size:13px;font-weight:600;color:var(--dark);line-height:1.4;';
  title.textContent = item.title;
  const meta = document.createElement('div');
  meta.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:2px;';
  meta.textContent = [item.variant_label, money(item.unit_price)].filter(Boolean).join(' · ');
  text.appendChild(title);
  text.appendChild(meta);

  row.appendChild(thumb);
  row.appendChild(text);

  if (remaining <= 0) {
    // Already returned in full. Left visible, because a line that
    // silently vanishes from the list reads as a bug.
    const done = document.createElement('div');
    done.style.cssText = 'font-size:12px;color:var(--subtle);white-space:nowrap;';
    done.textContent = 'Already returned';
    row.appendChild(done);
    row.style.opacity = '.55';
    return { el: row, read: () => null };
  }

  const control = document.createElement('div');
  control.style.cssText = 'display:flex;align-items:center;gap:8px;';

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.style.cssText = 'width:16px;height:16px;cursor:pointer;accent-color:var(--blue);';
  check.setAttribute('aria-label', 'Return ' + item.title);

  const qty = document.createElement('select');
  qty.className = 'form-input';
  qty.style.cssText = 'width:64px;height:34px;padding:0 8px;font-size:13px;';
  for (let n = 1; n <= remaining; n++) {
    const option = document.createElement('option');
    option.value = String(n);
    option.textContent = String(n);
    qty.appendChild(option);
  }
  qty.disabled = true;

  check.addEventListener('change', () => {
    qty.disabled = !check.checked;
    onChange();
  });

  control.appendChild(check);
  control.appendChild(qty);
  row.appendChild(control);

  return {
    el: row,
    read: () => (check.checked
      ? { variant_id: item.variant_id, quantity: Number(qty.value) }
      : null),
  };
}

/**
 * The return form for one order.
 * @param {string} orderId
 * @param {Function} [onDone]
 */
export async function openReturnComposer(orderId, onDone) {
  const response = await Fetcher.get({ endpoint: '/v1/order/' + orderId });
  if (!response.ok) return;
  const order = response.data;

  if (order.fulfilment_status !== 'Delivered') {
    toast.info('Only a delivered order can be returned');
    return;
  }

  const form = document.createElement('div');

  const intro = document.createElement('div');
  intro.style.cssText = 'font-size:13px;color:var(--subtle);margin-bottom:14px;line-height:1.6;';
  intro.textContent = 'Order #' + order.number + ' · delivered '
    + (order.delivered_at ? date(order.delivered_at) : '');
  form.appendChild(intro);

  const lines = (order.items || []).map((item) => lineRow(item, () => { error.style.display = 'none'; }));
  const lineBox = document.createElement('div');
  lines.forEach((line) => lineBox.appendChild(line.el));
  form.appendChild(field('What are you sending back?', lineBox));

  const reason = selectInput(REASONS.map((r) => ({ value: r, label: r })), REASONS[0]);
  form.appendChild(field('Reason', reason));

  const details = textArea({ placeholder: 'Anything else we should know? (optional)', rows: 4 });
  form.appendChild(field('Details', details,
    'Photos help with a damage claim — email them to us once the request is open.'));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  openModal({
    title: 'Request a return',
    body: form,
    width: '560px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Send request',
        className: 'btn-primary',
        onClick: async (close, button) => {
          const items = lines.map((line) => line.read()).filter(Boolean);
          if (!items.length) {
            error.textContent = 'Pick at least one item to send back.';
            error.style.display = '';
            return;
          }

          button.disabled = true;
          const created = await Fetcher.post({
            endpoint: '/v1/return',
            body: {
              order_id: orderId,
              items,
              reason: reason.value,
              details: details.value.trim() || null,
            },
          });
          button.disabled = false;
          if (!created.ok) return;

          close();
          toast.success('Return requested — we will email you once it is reviewed');
          if (onDone) onDone();
        },
      },
    ],
  });
}

/*
  Opened from the Returns page, which has no order in hand: pick one
  of the delivered orders first. Nothing else can be returned, so
  nothing else is listed.
*/
async function pickOrderToReturn() {
  const response = await Fetcher.get({
    endpoint: '/v1/order/list',
    query: { status: 'delivered', limit: 50 },
  });
  if (!response.ok) return;

  const orders = response.data.items || [];
  if (!orders.length) {
    toast.info('Nothing delivered yet — only a delivered order can be returned');
    return;
  }

  const box = document.createElement('div');
  box.style.cssText = 'display:flex;flex-direction:column;gap:8px;';

  orders.forEach((order) => {
    const row = document.createElement('div');
    row.setAttribute('role', 'button');
    row.tabIndex = 0;
    row.style.cssText = 'display:flex;justify-content:space-between;align-items:center;gap:12px;'
      + 'padding:12px;border:1px solid var(--border);border-radius:var(--radius-sm);cursor:pointer;';

    const left = document.createElement('div');
    const number = document.createElement('div');
    number.style.cssText = 'font-size:13px;font-weight:700;color:var(--dark);';
    number.textContent = '#' + order.number;
    const when = document.createElement('div');
    when.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:2px;';
    when.textContent = date(order.placed_at) + ' · ' + order.item_count
      + (order.item_count === 1 ? ' item' : ' items');
    left.appendChild(number);
    left.appendChild(when);

    const total = document.createElement('div');
    total.style.cssText = 'font-size:13px;font-weight:700;color:var(--dark);';
    total.textContent = money(order.totals.grand_total);

    row.appendChild(left);
    row.appendChild(total);
    row.addEventListener('click', () => { close(); openReturnComposer(order.uuid, load); });
    row.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); row.click(); }
    });
    box.appendChild(row);
  });

  const { close } = openModal({
    title: 'Which order?',
    body: box,
    width: '480px',
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({ endpoint: '/v1/return/list' });

  hideSkeleton(list);
  list.innerHTML = '';

  if (!response.ok) {
    emptyState(list, "Couldn't load your returns", 'Please try again in a moment.');
    return;
  }

  const requests = response.data || [];
  if (!requests.length) {
    emptyState(list, 'No returns yet',
      'If something arrives damaged or is not what you expected, start a return here.');
    return;
  }

  const frag = document.createDocumentFragment();
  requests.forEach((request) => frag.appendChild(returnCard(request)));
  list.appendChild(frag);
}

/* ── Boot ─────────────────────────────────────────────────────
   The orders page imports `openReturnComposer` from here, and an ES
   module body runs on import — so the page boot has to establish it
   is actually on the returns page before it mounts anything.      */
(async function init() {
  list = $('.returns-list');
  if (!list) return;

  const profile = await mountAccountChrome();
  if (!profile) return;

  const start = $('.returns-start');
  if (start) start.addEventListener('click', pickOrderToReturn);

  // Arriving from an order card: open straight onto that order.
  const requested = new URLSearchParams(location.search).get('order');

  await load();
  if (requested) openReturnComposer(requested, load);
})();
