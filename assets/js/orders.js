/* ═══════════════════════════════════════════════════════════════
   Bambee Web — orders.js

   The order history: five tabs, a search box, and a card per order.

   The card's actions are the interesting part, because they are not
   the same on every order. `POST /v1/order/{id}/cancel` is refused
   by the backend past Packing, and a button that only ever produces
   an error is worse than no button — so Cancel appears exactly while
   it will work. Track appears once there is something to track;
   Buy Again on anything delivered or cancelled.

   The tab counts come from the endpoint itself (`counts`), so they
   are the real totals rather than the length of the current page.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { money, imageUrl, paintImage, emptyState, hideSkeleton } from './catalog.js';
import { mountAccountChrome } from './account.js';
import { openReviewComposer, openOrderReviewPicker } from './review-form.js';
import { openReturnComposer } from './returns.js';
import { confirmModal } from './modal.js';

const $ = (sel) => document.querySelector(sel);

const TABS = ['all', 'processing', 'shipped', 'delivered', 'cancelled'];
const TAB_LABEL = {
  all: 'All', processing: 'Processing', shipped: 'Shipped',
  delivered: 'Delivered', cancelled: 'Cancelled',
};

/* The pill class the stylesheet already has for each state. */
const PILL = {
  Pending: 'status-processing',
  Packing: 'status-processing',
  OnHold: 'status-processing',
  Shipped: 'status-shipped',
  Delivered: 'status-delivered',
  Cancelled: 'status-cancelled',
  Returned: 'status-cancelled',
};

const STATUS_LABEL = {
  Pending: 'Processing',
  Packing: 'Packing',
  OnHold: 'On Hold',
  Shipped: 'Shipped',
  Delivered: 'Delivered',
  Cancelled: 'Cancelled',
  Returned: 'Returned',
};

let state = { tab: 'all', q: '', page: 1 };
let cards = null;   // the container the cards live in

/* ── Card ─────────────────────────────────────────────────────  */

function thumbs(order) {
  const row = document.createElement('div');
  row.className = 'order-items-row';

  // Four thumbnails, then a count — a card with eleven of them stops
  // being a card.
  order.items.slice(0, 3).forEach((item) => {
    const thumb = document.createElement('div');
    thumb.className = 'order-item-thumb';
    thumb.title = item.title;
    const inner = document.createElement('div');
    inner.className = 'order-item-thumb-bg';
    if (imageUrl(item.image_id)) paintImage(inner, item.image_id);
    else inner.style.background = 'var(--input-bg)';
    thumb.appendChild(inner);
    row.appendChild(thumb);
  });

  if (order.items.length > 3) {
    const more = document.createElement('div');
    more.className = 'order-items-more';
    more.textContent = '+' + (order.items.length - 3);
    row.appendChild(more);
  }

  return row;
}

function deliveryLine(order) {
  const parts = [order.delivery.name];
  const done = order.fulfilment_status === 'Delivered' && order.delivered_at;
  if (done) {
    parts.push('Delivered ' + new Date(order.delivered_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }));
  } else if (order.delivery.estimated_from && order.delivery.estimated_to) {
    const day = { month: 'short', day: 'numeric' };
    parts.push('Est. ' + new Date(order.delivery.estimated_from).toLocaleDateString(undefined, day)
      + ' – ' + new Date(order.delivery.estimated_to).toLocaleDateString(undefined, day));
  }
  return parts.join(' · ');
}

function orderCard(order) {
  const card = document.createElement('div');
  card.className = 'order-card';
  card.dataset.id = order.uuid;

  const placed = new Date(order.placed_at)
    .toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  const units = order.item_count;

  card.innerHTML =
    '<div class="order-card-header">'
      + '<div><div class="order-id-label">Order</div><div class="order-id"></div></div>'
      + '<div class="order-date"></div>'
      + '<div class="status-pill"></div>'
    + '</div>'
    + '<div class="order-card-body">'
      + '<div class="order-meta">'
        + '<div class="order-meta-item order-meta-delivery"></div>'
        + '<div class="order-meta-item order-meta-payment"></div>'
      + '</div>'
    + '</div>'
    + '<div class="order-card-footer">'
      + '<div><div class="order-total-lbl"></div><div class="order-total"></div></div>'
      + '<div class="order-actions"></div>'
    + '</div>';

  card.querySelector('.order-id').textContent = '#' + order.number;
  card.querySelector('.order-date').textContent = placed + ' · ' + units + (units === 1 ? ' item' : ' items');

  const pill = card.querySelector('.status-pill');
  pill.classList.add(PILL[order.fulfilment_status] || 'status-processing');
  pill.textContent = STATUS_LABEL[order.fulfilment_status] || order.fulfilment_status;

  card.querySelector('.order-card-body').prepend(thumbs(order));
  card.querySelector('.order-meta-delivery').textContent = deliveryLine(order);
  card.querySelector('.order-meta-payment').textContent = order.payment_summary || '';

  card.querySelector('.order-total-lbl').textContent =
    order.payment_status === 'Paid' ? 'Total Paid' : 'Total Due';
  card.querySelector('.order-total').textContent = money(order.totals.grand_total);

  card.querySelector('.order-actions').append(...actions(order, card));
  return card;
}

/* ── Actions ──────────────────────────────────────────────────  */

function button(label, className, onClick) {
  const el = document.createElement('button');
  el.className = 'btn ' + className + ' btn-sm';
  el.textContent = label;
  el.addEventListener('click', onClick);
  return el;
}

function actions(order, card) {
  const out = [];
  const status = order.fulfilment_status;

  if (status === 'Shipped') {
    out.push(button('Track Order', 'btn-primary', () => {
      location.href = '/account/tracking?id=' + encodeURIComponent(order.uuid);
    }));
  }

  /* `order/cancel` refuses anything past Packing, so the button is
     offered exactly while the server will honour it. */
  if (status === 'Pending' || status === 'Packing') {
    out.push(button('Cancel Order', 'btn-outline', () => cancel(order, card)));
  }

  /* Both of these are Delivered-only for the same reason: the
     backend says so. `return/create.rs` refuses anything else
     outright, and a review earns its Verified Purchase badge from a
     delivered order — which is also the moment a customer has an
     opinion worth writing down. */
  if (status === 'Delivered') {
    out.push(button('Write Review', 'btn-outline', () => review(order)));
    out.push(button('Return Items', 'btn-ghost', () => {
      openReturnComposer(order.uuid, () => { location.href = '/account/returns'; });
    }));
  }

  if (status === 'Delivered' || status === 'Cancelled' || status === 'Returned') {
    out.push(button('Buy Again', 'btn-primary', (e) => reorder(order, e.target)));
  }

  out.push(button('View Details', 'btn-ghost', () => {
    location.href = '/checkout/confirmation?id=' + encodeURIComponent(order.uuid);
  }));

  return out;
}

/*
  A multi-product order asks first: one item, or the whole order in
  one review. A single-product order goes straight to the composer.
*/
function review(order) {
  openOrderReviewPicker(order, (item, items) => {
    openReviewComposer(item
      ? { orderId: order.uuid, productId: item.product_id, productTitle: item.title, image: item.image_id }
      : { orderId: order.uuid, items });
  });
}

async function cancel(order, card) {
  // Cancelling is not something to do on a stray click.
  const confirmed = await confirmModal({
    title: 'Cancel this order?',
    message: 'Order #' + order.number + ' will be cancelled and the stock released. '
      + 'This cannot be undone.',
    confirmLabel: 'Cancel order',
  });
  if (!confirmed) return;

  const response = await Fetcher.post({
    endpoint: '/v1/order/' + order.uuid + '/cancel',
    body: { reason: 'Cancelled from my orders' },
  });
  if (!response.ok) return;

  toast.success('Order cancelled');
  load();
}

async function reorder(order, target) {
  target.disabled = true;
  const response = await Fetcher.post({ endpoint: '/v1/order/' + order.uuid + '/reorder', body: {} });
  target.disabled = false;
  if (!response.ok) return;

  toast.success('Added back to your cart');
  setTimeout(() => { location.href = '/cart'; }, 600);
}

/* ── Tabs, search, paging ─────────────────────────────────────  */

function paintTabs(counts) {
  const host = $('.orders-filter-bar');
  host.innerHTML = '';
  TABS.forEach((tab) => {
    const el = document.createElement('div');
    el.className = 'order-filter-tab' + (tab === state.tab ? ' active' : '');
    el.textContent = TAB_LABEL[tab] + ' (' + (counts[tab] || 0) + ')';
    el.addEventListener('click', () => {
      state.tab = tab;
      state.page = 1;
      load();
    });
    host.appendChild(el);
  });
}

function paintPager(total, limit) {
  const pager = $('.pagination');
  if (!pager) return;
  const pages = Math.ceil(total / limit);

  pager.innerHTML = '';
  pager.style.display = pages > 1 ? '' : 'none';
  if (pages <= 1) return;

  const step = (page, label, disabled) => {
    const el = document.createElement('div');
    el.className = 'page-btn' + (page === state.page ? ' active' : '') + (disabled ? ' disabled' : '');
    el.textContent = label;
    if (!disabled && page !== state.page) {
      el.addEventListener('click', () => { state.page = page; load(); });
    }
    return el;
  };

  pager.appendChild(step(state.page - 1, '‹', state.page === 1));
  for (let page = 1; page <= pages; page++) pager.appendChild(step(page, String(page)));
  pager.appendChild(step(state.page + 1, '›', state.page === pages));
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/order/list',
    query: { status: state.tab, q: state.q, page: state.page, limit: 10 },
  });

  hideSkeleton(cards);
  cards.innerHTML = '';

  if (!response.ok) {
    emptyState(cards, "Couldn't load your orders", 'Please try again in a moment.');
    return;
  }

  const { items, total, limit, counts } = response.data;
  paintTabs(counts);
  paintPager(total, limit);

  if (!items.length) {
    emptyState(cards,
      state.q ? 'No orders match “' + state.q + '”' : 'No orders here yet',
      state.tab === 'all' && !state.q
        ? 'Anything you buy will show up here.'
        : 'Try another tab, or clear the search.');
    return;
  }

  const frag = document.createDocumentFragment();
  items.forEach((order) => frag.appendChild(orderCard(order)));
  cards.appendChild(frag);
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  cards = $('.orders-cards');

  const search = document.querySelector('.form-input[placeholder^="Search orders"]');
  if (search) {
    let timer = null;
    search.addEventListener('input', () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        state.q = search.value.trim();
        state.page = 1;
        load();
      }, 300);
    });
  }

  load();
})();
