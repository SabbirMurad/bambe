/* ═══════════════════════════════════════════════════════════════
   Bambee Web — notifications.js

   `GET /v1/notification/list` returns the last 50, newest first, as
   a bare array — no paging, no filter, no unread count. So the
   grouping into Today / Yesterday / Earlier, the unread tally and
   the per-row actions are all worked out here from `created_at`,
   `is_read` and `type`.

   Opening one marks it read, because that is what opening it means.
   The dot and the tally update straight away and the PATCH follows;
   a failed PATCH is not worth blocking the navigation over, but the
   row's state is restored so the page doesn't lie about it.

   Where a notification goes is `link` when the server set one, and
   otherwise derived from its type and `related_id` — an order id
   opens the order, a product id opens the product.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { imageUrl, paintImage, emptyState, hideSkeleton } from './catalog.js';
import { mountAccountChrome } from './account.js';

const $ = (sel) => document.querySelector(sel);

let notifications = [];

/* ── Type → look and destination ──────────────────────────────
   The stylesheet has three icon treatments — order, delivery,
   promo — so every type maps onto one of them rather than
   inventing a fourth that has no styles.                        */

const DELIVERY_ICON = '<path d="M1 7h12v9H1zM13 9l3-3 4 4v6h-7V9Z" stroke="var(--success)" stroke-width="1.3" stroke-linejoin="round"/><circle cx="4" cy="17" r="1.5" stroke="var(--success)" stroke-width="1.2"/><circle cx="14" cy="17" r="1.5" stroke="var(--success)" stroke-width="1.2"/>';
const ORDER_ICON = '<rect x="2" y="3" width="16" height="14" rx="2.5" stroke="var(--blue)" stroke-width="1.3"/><path d="M6 8h8M6 12h5" stroke="var(--blue)" stroke-width="1.3" stroke-linecap="round"/>';
const PROMO_ICON = '<path d="M10 17S2 12 2 6.5a4 4 0 0 1 8-1 4 4 0 0 1 8 1C18 12 10 17 10 17Z" stroke="var(--gold)" stroke-width="1.4" stroke-linejoin="round"/>';

const TYPES = {
  order_placed:    { look: 'order',    icon: ORDER_ICON,    target: 'order' },
  order_confirmed: { look: 'order',    icon: ORDER_ICON,    target: 'order' },
  order_shipped:   { look: 'delivery', icon: DELIVERY_ICON, target: 'tracking' },
  order_delivered: { look: 'delivery', icon: DELIVERY_ICON, target: 'tracking' },
  order_cancelled: { look: 'order',    icon: ORDER_ICON,    target: 'order' },
  refund_issued:   { look: 'order',    icon: ORDER_ICON,    target: 'order' },
  back_in_stock:   { look: 'promo',    icon: PROMO_ICON,    target: 'product' },
  price_drop:      { look: 'promo',    icon: PROMO_ICON,    target: 'product' },
  promotion:       { look: 'promo',    icon: PROMO_ICON,    target: null },
  loyalty_points:  { look: 'promo',    icon: PROMO_ICON,    target: null },
  review_reminder: { look: 'order',    icon: ORDER_ICON,    target: 'product' },
  support_reply:   { look: 'order',    icon: ORDER_ICON,    target: null },
  security:        { look: 'order',    icon: ORDER_ICON,    target: null },
  system:          { look: 'order',    icon: ORDER_ICON,    target: null },
};

function kind(notification) {
  return TYPES[notification.type] || TYPES.system;
}

/*
  `link` is stored so an old notification survives a route change, so
  it wins. Otherwise the type says which collection `related_id`
  points into.
*/
function destination(notification) {
  if (notification.link) return notification.link;
  const id = notification.related_id;
  if (!id) return null;

  switch (kind(notification).target) {
    case 'order': return '/checkout/confirmation?id=' + encodeURIComponent(id);
    case 'tracking': return '/account/tracking?id=' + encodeURIComponent(id);
    // Products are addressed by slug on the storefront, and a
    // notification carries an id — the listing resolves it.
    case 'product': return '/listing?product=' + encodeURIComponent(id);
    default: return null;
  }
}

/* ── Time ─────────────────────────────────────────────────────  */

function startOfToday() {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

function group(at) {
  const today = startOfToday();
  if (at >= today) return 'TODAY';
  if (at >= today - 86400000) return 'YESTERDAY';
  return 'EARLIER';
}

function relative(at) {
  const seconds = Math.max(0, Math.round((Date.now() - at) / 1000));
  if (seconds < 60) return 'Just now';
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return minutes + (minutes === 1 ? ' minute ago' : ' minutes ago');
  const hours = Math.round(minutes / 60);
  if (hours < 24) return hours + (hours === 1 ? ' hour ago' : ' hours ago');
  const days = Math.round(hours / 24);
  if (days < 7) return days + (days === 1 ? ' day ago' : ' days ago');
  return null;   // past a week the absolute date is the useful half
}

function timeText(at) {
  const absolute = new Date(at).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  const ago = relative(at);
  return ago ? ago + ' · ' + absolute : absolute;
}

/* ── Row ──────────────────────────────────────────────────────  */

function row(notification) {
  const item = document.createElement('div');
  item.className = 'notif-item' + (notification.is_read ? '' : ' unread');
  item.style.padding = '16px 20px';
  item.dataset.id = notification.uuid;

  const look = kind(notification);
  const href = destination(notification);

  item.innerHTML =
    '<div class="notif-icon-wrap notif-icon-' + look.look + '">'
      + '<svg width="20" height="20" viewBox="0 0 20 20" fill="none">' + look.icon + '</svg>'
    + '</div>'
    + '<div class="notif-body">'
      + '<div class="notif-title"></div>'
      + '<div class="notif-text"></div>'
      + '<div class="notif-time"></div>'
    + '</div>'
    + (notification.is_read ? '' : '<div class="notif-unread-dot"></div>');

  item.querySelector('.notif-title').textContent = notification.title;
  item.querySelector('.notif-text').textContent = notification.body;
  item.querySelector('.notif-time').textContent = timeText(notification.created_at);

  // The thumbnail replaces the type glyph when the notification has
  // one — a price drop is about a specific thing, and showing it is
  // the whole point of the image being on the record.
  if (imageUrl(notification.image)) {
    const wrap = item.querySelector('.notif-icon-wrap');
    wrap.innerHTML = '';
    paintImage(wrap, notification.image);
  }

  if (href) {
    item.style.cursor = 'pointer';
    item.addEventListener('click', () => open(notification, item, href));
  } else {
    item.addEventListener('click', () => markRead(notification, item));
  }

  return item;
}

/* ── Actions ──────────────────────────────────────────────────  */

async function markRead(notification, item) {
  if (notification.is_read) return true;

  notification.is_read = true;
  item.classList.remove('unread');
  const dot = item.querySelector('.notif-unread-dot');
  if (dot) dot.remove();
  paintUnread();

  const response = await Fetcher.patch({
    endpoint: '/v1/notification/read',
    body: { notification_id: notification.uuid },
    showError: false,
  });

  if (!response.ok) {
    // Put it back rather than leave the page claiming something the
    // server doesn't agree with.
    notification.is_read = false;
    item.classList.add('unread');
    paintUnread();
    return false;
  }
  return true;
}

function open(notification, item, href) {
  markRead(notification, item);
  location.href = href;
}

async function markAll() {
  const unread = notifications.filter((n) => !n.is_read);
  if (!unread.length) {
    toast.info('Nothing unread');
    return;
  }

  const response = await Fetcher.patch({ endpoint: '/v1/notification/read-all', body: {} });
  if (!response.ok) return;

  notifications.forEach((n) => { n.is_read = true; });
  render();
  toast.success(unread.length === 1 ? 'Marked as read' : 'Marked ' + unread.length + ' as read');
}

/* ── Render ───────────────────────────────────────────────────  */

function paintUnread() {
  const count = notifications.filter((n) => !n.is_read).length;
  const label = document.querySelector('.notif-toolbar h1 span');
  if (label) label.textContent = count ? count + ' unread' : 'All caught up';

  const markAllBtn = $('.notif-mark-all');
  if (markAllBtn) markAllBtn.style.display = count ? '' : 'none';

  const badge = document.querySelector('.account-nav-item .account-nav-badge');
  const navBadge = [...document.querySelectorAll('.account-nav-item')]
    .find((item) => item.textContent.trim().toLowerCase().startsWith('notifications'));
  const target = navBadge ? navBadge.querySelector('.account-nav-badge') : badge;
  if (target) {
    target.textContent = count;
    target.style.display = count ? '' : 'none';
  }
}

function groupCard(label, rows) {
  const card = document.createElement('div');
  card.style.cssText = 'background:var(--white);border:1px solid var(--border);border-radius:var(--radius-lg);overflow:hidden;margin-bottom:16px;';

  const heading = document.createElement('div');
  heading.className = 'notif-group-label';
  heading.style.padding = '12px 20px 8px';
  heading.textContent = label;
  card.appendChild(heading);

  rows.forEach((r) => card.appendChild(r));
  return card;
}

function render() {
  const host = $('.notif-list');
  hideSkeleton(host);
  host.innerHTML = '';

  if (!notifications.length) {
    emptyState(host, 'No notifications yet',
      'Order updates, price drops and back-in-stock alerts land here.');
    paintUnread();
    return;
  }

  ['TODAY', 'YESTERDAY', 'EARLIER'].forEach((label) => {
    const rows = notifications
      .filter((n) => group(n.created_at) === label)
      .map((n) => row(n));
    if (rows.length) host.appendChild(groupCard(label, rows));
  });

  paintUnread();
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  const markAllBtn = $('.notif-mark-all');
  if (markAllBtn) markAllBtn.onclick = markAll;

  /* The Settings button belongs to the notification preferences, which
     are edited on the profile page — the switches are already there. */
  const settings = document.querySelector('.notif-toolbar .btn');
  if (settings) settings.onclick = () => { location.href = '/account/profile#notifications'; };

  const response = await Fetcher.get({ endpoint: '/v1/notification/list' });
  notifications = response.ok ? (response.data || []) : [];
  render();
})();
