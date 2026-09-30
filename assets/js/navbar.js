/* ═══════════════════════════════════════════════════════════════
   Bambe Web — navbar.js

   The storefront navbar's account button: whose name it shows, and
   where it goes.

   The account pages paint theirs in account.js and the admin console
   in admin-core.js, but nothing painted it on the shopping and
   checkout pages — so the "John D." the navbar was drawn with
   survived into the running site, signed in or out.

   Imported for its side effect: a page only has to load it.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { session } from './session.js';

/*
  The name is cached so the next page paints it on the first frame.
  Without it, every navigation shows a placeholder until the profile
  request lands — the same flicker, just briefer. Keyed by user id so
  one account never briefly wears another's name on a shared browser.
*/
const cacheKey = () => `bambe.nav_name.${session.userId() || 'anon'}`;

function readCachedName() {
  try {
    return localStorage.getItem(cacheKey());
  } catch (e) {
    return null;   // private mode, or storage disabled
  }
}

function writeCachedName(name) {
  try {
    localStorage.setItem(cacheKey(), name);
  } catch (e) {
    /* Not worth failing the navbar over. */
  }
}

/*
  The navbar has room for a first name and an initial, which is the
  shape it was drawn with. account.js spells the name out in full
  because its sidebar has the width for it.
*/
function shortName(profile) {
  const first = (profile.first_name || '').trim();
  const last = (profile.last_name || '').trim();
  // [...last][0] rather than last[0]: an initial should be a whole
  // character, not half a surrogate pair.
  if (first && last) return `${first} ${[...last][0].toUpperCase()}.`;
  return first || (profile.username || '').trim() || 'Account';
}

/** Point the button at `href`, whichever way this page wired it up. */
function pointAt(button, href) {
  const anchor = button.closest('a');
  if (anchor) anchor.href = href;
  button.onclick = () => { location.href = href; };
  button.style.cursor = 'pointer';
}

/* ── Counts ───────────────────────────────────────────────── */

/*
  Same rule account.js applies to its sidebar counts: a badge reading
  5 over an empty wishlist is worse than no badge, so an empty count
  hides it rather than showing a zero.
*/
function setBadge(kind, count) {
  const badge = document.querySelector(`.nav-icon-btn[data-nav="${kind}"] .nav-badge`);
  if (!badge) return;
  badge.textContent = count;
  badge.style.display = count ? '' : 'none';
}

const BADGES = ['wishlist', 'notifications', 'cart'];

async function paintBadges() {
  /*
    Hidden before anything is asked for. These were drawn with
    wireframe constants — a 5 on the wishlist and a 3 on the cart, on
    every page — and a number that is merely stale for a moment is
    still a number nobody can act on.
  */
  BADGES.forEach((kind) => setBadge(kind, 0));

  // An admin has no wishlist, notifications or cart of their own — those
  // all key on an `account_core` uuid. See mountUser() below.
  const signedIn = session.isSignedIn() && !session.isAdmin();
  /*
    The cart is the one count that survives signing out, but only once
    a guest id exists. Asking for it unconditionally would mint one for
    every visitor who ever loads a page — exactly what session.guestId()
    is careful not to do.
  */
  const hasCart = signedIn || !!session.peekGuestId();

  const [wishlist, notifications, cart] = await Promise.all([
    signedIn ? Fetcher.get({ endpoint: '/v1/wishlist', showError: false }) : null,
    signedIn ? Fetcher.get({ endpoint: '/v1/notification/list', showError: false }) : null,
    hasCart ? Fetcher.get({ endpoint: '/v1/cart', showError: false }) : null,
  ]);

  if (wishlist && wishlist.ok) {
    setBadge('wishlist', (wishlist.data || []).length);
  }
  // The endpoint answers with a bare array of the last 50 and takes no
  // filter, so the unread count is counted here — as account.js does.
  if (notifications && notifications.ok) {
    setBadge('notifications', (notifications.data || []).filter((n) => !n.is_read).length);
  }
  if (cart && cart.ok) {
    setBadge('cart', (cart.data && cart.data.item_count) || 0);
  }
}

/* ── Who is signed in ─────────────────────────────────────── */

async function mountUser() {
  const button = document.querySelector('.nav-user-btn');
  const label = button && button.querySelector('.nav-user-name');
  if (!label) return;

  /*
    Signed out, the button was still offering a name and — on the home
    page, whose inline script rewrites the link unconditionally — a
    trip to the profile page, which only bounces back to login.
  */
  if (!session.isSignedIn()) {
    label.textContent = 'Sign in';
    pointAt(button, '/login');
    return;
  }

  /*
    An administrator signing in at /admin/login stores their session on
    this same origin, so they arrive here carrying an admin token. They
    have no storefront profile — `/v1/account/profile` reads
    `account_core` and an admin lives in `admin_account` — so asking for
    one would 403 on every page and leave the button reading "Account".
    Send them where their session actually works instead.
  */
  if (session.isAdmin()) {
    label.textContent = 'Admin';
    pointAt(button, '/admin');
    return;
  }

  // Something true on the first frame, so the drawn-in name never
  // reaches the screen.
  label.textContent = readCachedName() || 'Account';
  pointAt(button, '/account/profile');

  const response = await Fetcher.get({ endpoint: '/v1/account/profile', showError: false });
  // A session the server won't honour is the page guard's problem,
  // not the navbar's — leave the neutral label rather than a toast.
  if (!response.ok) return;

  const name = shortName(response.data);
  label.textContent = name;
  writeCachedName(name);
}

mountUser();
paintBadges();
