/* ═══════════════════════════════════════════════════════════════
   Bambe Web — confirmation.js

   The receipt. Everything here comes from `GET /v1/order/{id}` —
   the order as it was actually written, not as the checkout draft
   remembered it. That distinction matters: a coupon that expired
   between the review page and the insert is dropped by
   `place_order`, and the total on this page has to be the one that
   was charged.

   The tracker shows where the order really is. A fresh order is
   Pending, so only the first node is lit; opening this page later
   from the orders list will show it further along rather than
   frozen at "Confirmed · Packing" the way the wireframe drew it.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { session } from './session.js';
import { money, imageUrl } from './catalog.js';

const $ = (sel) => document.querySelector(sel);

/* ── Header ───────────────────────────────────────────────────  */

function renderHeader(order, profile) {
  // `first_name` is optional on the profile; the username is what
  // every account has, so it stands in when there is no real name.
  const firstName = profile && (profile.first_name || profile.username);

  const line = $('#confirm-greeting');
  const greeting = line && line.querySelector('strong');
  if (greeting && firstName) {
    greeting.textContent = firstName;
  } else if (line) {
    // No name on file — "Thank you!" reads better than "Thank you, !".
    line.textContent = 'Your order has been confirmed and will be packed shortly.';
  }

  const email = $('#confirm-email');
  const row = email && email.parentNode;
  if (email && profile && profile.email) {
    email.textContent = profile.email;
  } else if (row) {
    // Nothing to name, so nothing to claim was emailed.
    row.style.display = 'none';
  }
}

/* ── Order box ────────────────────────────────────────────────  */

function renderOrderBox(order) {
  const box = $('.confirm-order-box');
  if (!box) return;

  const heads = box.querySelectorAll(':scope > div:first-child > div');
  const [numberCol, dateCol, totalCol] = heads;

  if (numberCol) numberCol.children[1].textContent = '#' + order.number;
  if (dateCol) {
    dateCol.children[1].textContent = new Date(order.placed_at)
      .toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  }
  if (totalCol) {
    totalCol.children[1].textContent = money(order.totals.grand_total);
    // Cash on delivery is not paid yet, so the label has to stop
    // claiming it was.
    totalCol.children[0].textContent = order.payment_status === 'Paid' ? 'TOTAL PAID' : 'TOTAL DUE';
  }
}

function thumb(item) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'display:flex;flex-direction:column;gap:4px;align-items:center;';

  const picture = document.createElement('div');
  picture.style.cssText = 'width:64px;height:64px;border-radius:var(--radius-sm);background:var(--input-bg);position:relative;';
  const url = imageUrl(item.image);
  if (url) picture.style.background = "url('" + url + "') center / cover no-repeat";

  if (item.quantity > 1) {
    const badge = document.createElement('div');
    badge.style.cssText = 'position:absolute;top:-6px;right:-6px;background:var(--blue);color:white;font-size:10px;font-weight:700;width:18px;height:18px;border-radius:50%;display:flex;align-items:center;justify-content:center;';
    badge.textContent = item.quantity;
    picture.appendChild(badge);
  }

  const caption = document.createElement('div');
  caption.style.cssText = 'font-size:11px;color:var(--mid);text-align:center;max-width:72px;line-height:1.3;';
  caption.textContent = item.title;

  wrap.append(picture, caption);
  return wrap;
}

function renderThumbs(order) {
  const host = $('.confirm-thumbs');
  if (!host) return;
  host.innerHTML = '';
  order.items.forEach((item) => host.appendChild(thumb(item)));
}

function addressLines(snapshot) {
  const region = [snapshot.state, snapshot.zip].filter(Boolean).join(' ');
  return [
    snapshot.street,
    [snapshot.city, region].filter(Boolean).join(', ') + (snapshot.country ? ', ' + snapshot.country : ''),
  ].filter(Boolean);
}

function renderDeliveryAndPayment(order) {
  const grid = $('.confirm-order-box > div:last-child');
  if (!grid) return;
  const [delivery, payment] = grid.children;

  if (delivery) {
    const address = order.shipping_address;
    delivery.children[1].textContent = address.recipient_name;

    const lines = delivery.children[2];
    lines.innerHTML = '';
    addressLines(address).forEach((line, index) => {
      if (index) lines.appendChild(document.createElement('br'));
      lines.appendChild(document.createTextNode(line));
    });

    const pill = delivery.children[3].children[0];
    const window_ = delivery.children[3].children[1];
    pill.textContent = order.delivery.name + ' · ' + (order.delivery.price ? money(order.delivery.price) : 'Free');
    window_.textContent = estimateWindow(order.delivery);
  }

  if (payment) {
    payment.children[1].textContent = order.payment_summary || 'Payment';
    payment.children[2].textContent = order.billing_address
      ? 'Billing: ' + [order.billing_address.street, order.billing_address.city].filter(Boolean).join(', ')
      : '';
  }
}

function estimateWindow(delivery) {
  if (!delivery.estimated_from || !delivery.estimated_to) return '';
  const from = new Date(delivery.estimated_from);
  const to = new Date(delivery.estimated_to);
  const day = { month: 'short', day: 'numeric' };
  if (from.getTime() === to.getTime()) return to.toLocaleDateString(undefined, day);
  return from.toLocaleDateString(undefined, day) + ' – ' + to.toLocaleDateString(undefined, day);
}

/* ── Tracker ──────────────────────────────────────────────────
   Four nodes against the order's real fulfilment status. Cancelled
   is not one of the four, so it replaces the tracker rather than
   pretending the parcel is on its way.                            */

const STAGES = ['Pending', 'Packing', 'Shipped', 'Delivered'];

function renderTracker(order) {
  const tracker = $('#order-tracker');
  if (!tracker) return;

  const status = order.fulfilment_status;
  if (status === 'Cancelled' || status === 'Returned') {
    tracker.innerHTML = '';
    const note = document.createElement('div');
    note.style.cssText = 'text-align:center;font-size:13px;color:var(--error);font-weight:600;';
    note.textContent = 'This order was ' + status.toLowerCase() + '.';
    tracker.appendChild(note);
    return;
  }

  const reached = Math.max(0, STAGES.indexOf(status));
  const nodes = tracker.querySelectorAll(':scope > div:last-child > div');

  let step = -1;
  nodes.forEach((node) => {
    /* The connectors sit between the steps in the same row; only the
       ones holding a circle are stages. A connector is filled when the
       order has moved past the stage on its left — the wireframe's
       gradient implied progress the order hasn't made yet. */
    if (node.children.length < 2) {
      node.style.background = reached > step ? 'var(--blue)' : 'var(--border)';
      return;
    }
    step += 1;

    const circle = node.children[0];
    const label = node.children[1];
    const done = step < reached;
    const current = step === reached;

    circle.style.background = done ? 'var(--blue)' : (current ? 'var(--blue-lt)' : 'var(--input-bg)');
    circle.style.border = done ? 'none' : ('2px solid ' + (current ? 'var(--blue)' : 'var(--border)'));
    label.style.color = done || current ? 'var(--blue)' : 'var(--subtle)';
    label.style.fontWeight = done || current ? '700' : '600';

    /* The glyphs are drawn for the state the wireframe froze them in —
       the first one is white, for a filled circle it no longer has.
       Repainted to match, or it vanishes into its own background. */
    const tint = done ? 'white' : (current ? 'var(--blue)' : '#C4C0B9');
    circle.querySelectorAll('svg *').forEach((shape) => {
      if (shape.getAttribute('stroke')) shape.setAttribute('stroke', tint);
      const fill = shape.getAttribute('fill');
      if (fill && fill !== 'none') shape.setAttribute('fill', tint);
    });
  });
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  if (!session.requireSignedIn()) return;

  const id = new URLSearchParams(location.search).get('id');
  const page = $('.confirmation-page');

  if (!id) {
    // Landed here without an order — nothing to show, and inventing a
    // receipt is worse than saying so.
    page.innerHTML = '<div class="confirm-title">No order to show</div>'
      + '<p style="font-size:14px;color:var(--mid);text-align:center;margin:8px 0 24px;">'
      + 'Open an order from your account to see its details.</p>'
      + '<a href="/account/orders"><button class="btn btn-primary" style="height:46px;padding:0 28px;">View my orders</button></a>';
    return;
  }

  const [orderResponse, profileResponse] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/order/' + id }),
    Fetcher.get({ endpoint: '/v1/account/profile', showError: false }),
  ]);

  if (!orderResponse.ok) {
    page.innerHTML = '<div class="confirm-title">That order couldn’t be found</div>'
      + '<a href="/account/orders" style="margin-top:16px;"><button class="btn btn-primary" style="height:46px;padding:0 28px;">View my orders</button></a>';
    return;
  }

  const order = orderResponse.data;
  const profile = profileResponse.ok ? profileResponse.data : null;

  renderHeader(order, profile);
  renderOrderBox(order);
  renderThumbs(order);
  renderDeliveryAndPayment(order);
  renderTracker(order);

  const track = document.querySelector('a[href="/account/tracking"]');
  if (track) track.href = '/account/tracking?id=' + encodeURIComponent(order.uuid);
})();
