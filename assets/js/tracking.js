/* ═══════════════════════════════════════════════════════════════
   Bambe Web — tracking.js

   Where one order has got to. `GET /v1/order/{id}` carries the four
   lifecycle timestamps, the carrier's `tracking` block and any
   `shipment_events` the carrier has posted.

   The wireframe drew five fixed steps with times typed into them.
   Here the steps that have happened get their real timestamp, the
   one in progress is marked active, and the rest are pending with
   the estimate rather than a made-up date. Carrier scans, when
   there are any, are inserted as their own rows — they are the only
   part of this that is actually live data from outside the shop.

   A cancelled order gets no timeline at all. Showing "Out for
   delivery — expected Friday" under a cancelled order is the kind
   of thing that generates a support ticket.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { money, imageUrl, hideSkeleton } from './catalog.js';
import { mountAccountChrome } from './account.js';

const $ = (sel) => document.querySelector(sel);

const STATUS_BADGE = {
  Pending: 'Processing',
  Packing: 'Packing',
  OnHold: 'On Hold',
  Shipped: 'In Transit',
  Delivered: 'Delivered',
  Cancelled: 'Cancelled',
  Returned: 'Returned',
};

const dateTime = (ms) => new Date(ms).toLocaleString(undefined, {
  year: 'numeric', month: 'long', day: 'numeric', hour: 'numeric', minute: '2-digit',
});
const dayOnly = (ms) => new Date(ms).toLocaleDateString(undefined, {
  year: 'numeric', month: 'short', day: 'numeric',
});

/* ── Header ───────────────────────────────────────────────────  */

function renderHeader(order) {
  const card = $('.tracking-header-card');
  $('.tracking-order-id').textContent = '#' + order.number;
  $('.tracking-order-meta').textContent = 'Placed ' + dayOnly(order.placed_at)
    + ' · ' + order.items.length + (order.items.length === 1 ? ' item' : ' items')
    + ' · ' + money(order.totals.grand_total);

  $('.tracking-status-badge').textContent =
    STATUS_BADGE[order.fulfilment_status] || order.fulfilment_status;

  // The two lines under the badge are the carrier's, and an order that
  // hasn't shipped has no carrier yet.
  const right = card.lastElementChild;
  const [trackingLine, carrierLine] = [right.children[1], right.children[2]];
  if (order.tracking) {
    trackingLine.textContent = 'Tracking: ' + order.tracking.tracking_number;
    carrierLine.textContent = 'Carrier: ' + order.tracking.carrier;
  } else {
    trackingLine.textContent = order.fulfilment_status === 'Cancelled'
      ? 'This order was cancelled'
      : 'Not dispatched yet';
    carrierLine.style.display = 'none';
  }
}

/* ── Timeline ─────────────────────────────────────────────────  */

const TICK = '<svg width="10" height="10" viewBox="0 0 10 10" fill="none"><path d="M2 5l2 2 4-4" stroke="white" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>';

function timelineRow(step, isLast) {
  const item = document.createElement('div');
  item.className = 'tl-item';

  const dotClass = step.state === 'done' ? 'done' : (step.state === 'active' ? 'active' : 'pending');
  item.innerHTML =
    '<div class="tl-left">'
      + '<div class="tl-dot ' + dotClass + '">'
        + (step.state === 'done' ? TICK : (step.state === 'active' ? '<div style="width:6px;height:6px;border-radius:50%;background:white;"></div>' : ''))
      + '</div>'
      + (isLast ? '' : '<div class="tl-line' + (step.state === 'done' ? ' done' : '') + '"></div>')
    + '</div>'
    + '<div class="tl-content">'
      + '<div class="tl-title ' + dotClass + '"></div>'
      + '<div class="tl-desc"></div>'
      + '<div class="tl-time"></div>'
    + '</div>';

  item.querySelector('.tl-title').textContent = step.title;
  item.querySelector('.tl-desc').textContent = step.detail;

  const time = item.querySelector('.tl-time');
  time.textContent = step.time;
  if (step.state === 'pending') time.style.color = 'var(--subtle)';

  return item;
}

/*
  The order's own milestones, plus whatever the carrier has posted.
  A step is `done` when its timestamp exists, `active` when it is the
  furthest one reached, and `pending` otherwise.
*/
function buildSteps(order) {
  const status = order.fulfilment_status;
  const shipped = !!order.shipped_at;
  const delivered = !!order.delivered_at;
  const estimate = order.delivery.estimated_to;

  const steps = [
    {
      title: 'Order Confirmed',
      detail: order.payment_status === 'Paid'
        ? 'Your order has been placed and payment confirmed.'
        : 'Your order has been placed. Payment is due on delivery.',
      time: dateTime(order.placed_at),
      state: 'done',
    },
    {
      title: 'Packed & Ready',
      detail: 'Items picked and packed at the fulfilment centre.',
      time: shipped ? dateTime(order.shipped_at) : 'In progress',
      state: shipped || delivered ? 'done' : (status === 'Packing' ? 'active' : 'pending'),
    },
    {
      title: 'In Transit',
      detail: order.tracking
        ? 'Handed to ' + order.tracking.carrier + '.'
        : 'On its way once the carrier collects it.',
      time: shipped ? dateTime(order.shipped_at) : (estimate ? 'Expected by ' + dayOnly(estimate) : ''),
      state: delivered ? 'done' : (shipped ? 'active' : 'pending'),
    },
  ];

  /* Carrier scans, in the order they happened. These are the only rows
     with a location, and the only ones this shop doesn't write itself. */
  (order.shipment_events || []).forEach((event) => {
    steps.push({
      title: event.status,
      detail: event.description || '',
      time: [dateTime(event.at), event.location].filter(Boolean).join(' · '),
      state: 'done',
    });
  });

  steps.push({
    title: 'Delivered',
    detail: delivered ? 'Package delivered to your address.' : 'Package will be delivered to your address.',
    time: delivered ? dateTime(order.delivered_at) : (estimate ? 'Expected ' + dayOnly(estimate) : ''),
    state: delivered ? 'done' : 'pending',
  });

  return steps;
}

function renderTimeline(order) {
  const host = $('.timeline');
  hideSkeleton(host);

  const title = host.querySelector('.timeline-title');
  host.innerHTML = '';
  host.appendChild(title);

  if (order.fulfilment_status === 'Cancelled' || order.fulfilment_status === 'Returned') {
    const note = document.createElement('div');
    note.style.cssText = 'padding:6px 0 4px;font-size:13px;color:var(--mid);line-height:1.7;';
    const when = order.cancelled_at ? ' on ' + dayOnly(order.cancelled_at) : '';
    note.textContent = 'This order was ' + order.fulfilment_status.toLowerCase() + when + '.'
      + (order.cancel_reason ? ' Reason: ' + order.cancel_reason : '');
    host.appendChild(note);

    if (order.payment_status === 'Refunded' || order.payment_status === 'PartiallyRefunded') {
      const refund = document.createElement('div');
      refund.style.cssText = 'font-size:13px;color:var(--success);font-weight:600;padding-top:6px;';
      refund.textContent = order.payment_status === 'Refunded'
        ? 'Payment refunded in full.'
        : 'Part of this order has been refunded.';
      host.appendChild(refund);
    }
    return;
  }

  const steps = buildSteps(order);
  steps.forEach((step, index) => host.appendChild(timelineRow(step, index === steps.length - 1)));
}

/* ── Items ────────────────────────────────────────────────────  */

function renderItems(order) {
  const card = $('.profile-form-card');
  card.querySelector('.profile-form-title').textContent =
    'Items in This Shipment (' + order.items.length + ')';

  const link = card.querySelector('a');
  if (link) link.href = '/checkout/confirmation?id=' + encodeURIComponent(order.uuid);

  const body = card.lastElementChild;
  body.innerHTML = '';

  order.items.forEach((item, index) => {
    if (index) {
      const divider = document.createElement('div');
      divider.className = 'divider';
      body.appendChild(divider);
    }

    const row = document.createElement('div');
    row.style.cssText = 'display:flex;gap:14px;align-items:center;';

    const thumb = document.createElement('div');
    thumb.style.cssText = 'width:56px;height:60px;border-radius:var(--radius-sm);background:var(--input-bg);flex-shrink:0;';
    const url = imageUrl(item.image);
    if (url) thumb.style.background = "url('" + url + "') center / cover no-repeat";

    const middle = document.createElement('div');
    middle.style.flex = '1';
    const name = document.createElement('div');
    name.style.cssText = 'font-weight:600;font-size:13px;color:var(--dark);';
    name.textContent = item.title + ' × ' + item.quantity;
    const variant = document.createElement('div');
    variant.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:2px;';
    variant.textContent = item.variant_label || '';
    middle.append(name, variant);

    const price = document.createElement('div');
    price.style.cssText = 'font-weight:700;font-size:14px;';
    price.textContent = money(item.line_total);

    row.append(thumb, middle, price);
    body.appendChild(row);
  });
}

/* ── Delivery panel ───────────────────────────────────────────  */

function renderDeliveryInfo(order) {
  const rows = document.querySelectorAll('.delivery-info-row');
  const value = (row) => row.querySelector('.delivery-info-val');
  const address = order.shipping_address;

  const shipTo = value(rows[0]);
  shipTo.innerHTML = '';
  shipTo.appendChild(document.createTextNode(address.recipient_name));
  const rest = document.createElement('span');
  rest.style.cssText = 'color:var(--subtle);font-size:12px;';
  rest.innerHTML = '';
  [address.street, [address.city, [address.state, address.zip].filter(Boolean).join(' ')].filter(Boolean).join(', ')]
    .filter(Boolean)
    .forEach((line) => {
      rest.appendChild(document.createElement('br'));
      rest.appendChild(document.createTextNode(line));
    });
  shipTo.appendChild(rest);

  const method = value(rows[1]);
  method.textContent = order.delivery.name + ' ';
  const price = document.createElement('span');
  price.style.cssText = order.delivery.price
    ? 'font-size:12px;color:var(--mid);'
    : 'color:var(--success);font-size:12px;';
  price.textContent = order.delivery.price ? money(order.delivery.price) : 'Free';
  method.appendChild(price);

  // Carrier and tracking number only exist once it has been dispatched.
  if (order.tracking) {
    value(rows[2]).textContent = order.tracking.carrier;
    value(rows[3]).textContent = order.tracking.tracking_number;
  } else {
    rows[2].style.display = 'none';
    rows[3].style.display = 'none';
  }

  const estimate = value(rows[4]);
  if (order.delivered_at) {
    rows[4].querySelector('.delivery-info-lbl').textContent = 'Delivered';
    estimate.textContent = dayOnly(order.delivered_at);
  } else if (order.delivery.estimated_from && order.delivery.estimated_to) {
    estimate.textContent = new Date(order.delivery.estimated_from)
      .toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
      + ' – ' + dayOnly(order.delivery.estimated_to);
  } else {
    rows[4].style.display = 'none';
  }
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  /* Reached from an order card with an id, or from the nav with none —
     in which case the most recently shipped order is the one being
     asked about, falling back to the most recent of any kind. */
  let id = new URLSearchParams(location.search).get('id');
  if (!id) {
    const list = await Fetcher.get({ endpoint: '/v1/order/list', query: { limit: 10 }, showError: false });
    const orders = list.ok ? list.data.items : [];
    const trackable = orders.find((o) => o.fulfilment_status === 'Shipped')
      || orders.find((o) => o.fulfilment_status === 'Delivered')
      || orders[0];
    if (!trackable) {
      hideSkeleton($('.timeline'));
      $('.tracking-layout').innerHTML =
        '<div style="padding:40px 0;font-size:14px;color:var(--mid);">Nothing to track yet — your orders will appear here once you place one.</div>';
      $('.tracking-header-card').style.display = 'none';
      return;
    }
    id = trackable.uuid;
  }

  const response = await Fetcher.get({ endpoint: '/v1/order/' + id });
  if (!response.ok) {
    hideSkeleton($('.timeline'));
    $('.tracking-layout').innerHTML =
      '<div style="padding:40px 0;font-size:14px;color:var(--mid);">That order couldn’t be found.</div>';
    return;
  }

  const order = response.data;
  renderHeader(order);
  renderTimeline(order);
  renderItems(order);
  renderDeliveryInfo(order);

  /* There is no customer-facing support page yet — the ticket API
     exists, nothing renders it — so two buttons that go nowhere are
     hidden rather than left to be clicked. */
  const help = $('.delivery-info-card').nextElementSibling;
  if (help) help.style.display = 'none';
})();
