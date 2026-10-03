/* ═══════════════════════════════════════════════════════════════
   Bambe Web — account.js

   The furniture every account page carries: the profile card at the
   top of the sidebar, the counts beside the nav items, and Log Out.
   All four pages drew the same John Doe and the same three badges,
   so it is filled in once here and imported by each page module.

   The two nav items with nothing behind them — Addresses and Payment
   Methods — point at the profile page, where those sections actually
   live, rather than sitting there unclickable.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { session } from './session.js';
import { imageUrl, paintImage, clearImage } from './catalog.js';

let profile = null;

export function currentProfile() {
  return profile;
}

/*
  The sidebar card's picture. Exported because the profile page
  changes it in place after an upload, and the card should follow
  without a reload. Painted as a background so the SVG silhouette
  stays underneath as the fallback, the same as the profile hero.
*/
export function paintSidebarAvatar(imageId) {
  const wrap = document.querySelector('.account-avatar');
  if (!wrap) return;

  const svg = wrap.querySelector('svg');
  if (!imageUrl(imageId)) {
    clearImage(wrap);
    if (svg) svg.style.display = '';
    return;
  }

  paintImage(wrap, imageId);
  if (svg) svg.style.display = 'none';
}

function displayName(p) {
  const full = [p.first_name, p.last_name].filter(Boolean).join(' ').trim();
  return full || p.username || 'Your account';
}

/*
  Counts, not decoration: an orders badge reading 12 when the account
  has one order is worse than no badge. Anything that comes back
  empty is hidden rather than shown as zero.
*/
async function paintBadges() {
  const items = document.querySelectorAll('.account-nav-item');
  const badges = {};
  items.forEach((item) => {
    const badge = item.querySelector('.account-nav-badge');
    if (!badge) return;
    const label = item.textContent.trim().toLowerCase();
    if (label.startsWith('my orders')) badges.orders = badge;
    else if (label.startsWith('wishlist')) badges.wishlist = badge;
    else if (label.startsWith('notifications')) badges.unread = badge;
  });

  const set = (badge, count) => {
    if (!badge) return;
    badge.textContent = count;
    badge.style.display = count ? '' : 'none';
  };

  const [orders, wishlist, notifications] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/order/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/wishlist', showError: false }),
    Fetcher.get({ endpoint: '/v1/notification/list', showError: false }),
  ]);

  set(badges.orders, orders.ok ? orders.data.total : 0);
  set(badges.wishlist, wishlist.ok ? (wishlist.data || []).length : 0);
  // The endpoint answers with a bare array of the last 50 and takes no
  // filter, so the unread count is counted here.
  set(badges.unread, notifications.ok
    ? (notifications.data || []).filter((n) => !n.is_read).length
    : 0);
}

function wireNav() {
  const logout = document.querySelector('.account-nav-logout');
  if (logout) {
    logout.style.cursor = 'pointer';
    logout.onclick = async () => {
      await Fetcher.post({ endpoint: '/v1/auth/sign-out', body: {}, showError: false });
      session.clear();
      location.href = '/login';
    };
  }

  document.querySelectorAll('.account-nav-item').forEach((item) => {
    const label = item.textContent.trim().toLowerCase();
    if (item.closest('a')) return;
    // The two settings rows are the profile page's own sections.
    const section = label.startsWith('addresses') ? 'addresses'
      : label.startsWith('payment') ? 'payment'
      : label.startsWith('preferences') ? 'notifications'
      : null;
    if (section) {
      item.style.cursor = 'pointer';
      item.onclick = () => { location.href = '/account/profile#' + section; };
    }
  });
}

/*
  Call at the top of every account page module. Returns the profile so
  the page can reuse it, or null when the visitor is being sent to sign
  in — in which case the caller should stop.
*/
export async function mountAccountChrome() {
  if (!session.requireSignedIn()) return null;

  wireNav();

  const response = await Fetcher.get({ endpoint: '/v1/account/profile' });
  if (!response.ok) return null;
  profile = response.data;

  const name = document.querySelector('.account-user-name');
  const email = document.querySelector('.account-user-email');
  if (name) name.textContent = displayName(profile);
  if (email) email.textContent = profile.email || '';
  paintSidebarAvatar(profile.profile_picture);

  // The navbar above is navbar.js's, on every page that carries it;
  // this module owns the sidebar card below it.
  paintBadges();
  return profile;
}
