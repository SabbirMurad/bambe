/* ═══════════════════════════════════════════════════════════════
   Bambee Web — navbar.js

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
import { paintImage, clearImage } from './catalog.js';
import { toast } from './toast.js';
import './smooth-scroll.js';
import './nav-autohide.js';
import './nav-search.js';

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
  paintNavAvatar(response.data.profile_picture);
}

/*
  The account button's picture.

  Nothing painted this before: the navbar set the name and left the
  silhouette, so an uploaded profile picture appeared on the account
  pages and nowhere else — not even after a reload. Exported because
  profile.js has to repaint it the moment a new picture is uploaded,
  without waiting for the next navigation.
*/
export function paintNavAvatar(imageId) {
  const wrap = document.querySelector('.nav-avatar');
  if (!wrap) return;
  const svg = wrap.querySelector('svg');
  if (!imageId) {
    clearImage(wrap);
    if (svg) svg.style.display = '';
    return;
  }
  paintImage(wrap, imageId);
  if (svg) svg.style.display = 'none';
}

/* ── Deliver to ───────────────────────────────────────────────
   The "Deliver to" control beside the logo (home page). It names the
   shopper's default address; with none it offers to add one.

     signed out  →  "Add Address", to sign-in and then the add form
     no address  →  "Add Address", to the add form on the profile page
     addresses   →  the default one; a click opens a list of them, and
                    picking one makes it the default — the same default
                    checkout starts from, via POST /v1/address/{id}/default

   The label is cached per user like the account name above, so a
   returning shopper sees their address on the first frame instead of
   "Add Address" flashing first.                                    */
const ADD_ADDRESS_PATH = '/account/profile#add-address';
const deliverKey = () => `bambe.nav_deliver.${session.userId() || 'anon'}`;

function readCachedDeliver() {
  try { return localStorage.getItem(deliverKey()); } catch (e) { return null; }
}
function writeCachedDeliver(text) {
  try {
    if (text) localStorage.setItem(deliverKey(), text);
    else localStorage.removeItem(deliverKey());
  } catch (e) { /* not worth failing the navbar over */ }
}

/** "Home · Dhaka" — what the customer calls it, and where it is. */
function deliverLabel(address) {
  return [address.label, address.city].filter(Boolean).join(' · ') || 'Saved address';
}

async function mountDeliver() {
  const host = document.querySelector('.nav-deliver');
  if (!host) return;
  const text = host.querySelector('.nav-deliver-text');
  const loc = host.querySelector('.nav-deliver-loc');
  if (!text || !loc) return;

  loc.setAttribute('role', 'button');
  loc.tabIndex = 0;

  const go = (href) => { location.href = href; };
  const onActivate = (handler) => {
    loc.onclick = handler;
    loc.onkeydown = (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
    };
  };

  // Admins have no address book of their own (addresses key on an
  // account_core user), so they get the signed-out behaviour.
  if (!session.isSignedIn() || session.isAdmin()) {
    text.textContent = 'Add Address';
    onActivate(() => go('/login?next=' + encodeURIComponent(ADD_ADDRESS_PATH)));
    return;
  }

  const cached = readCachedDeliver();
  if (cached) text.textContent = cached;

  const response = await Fetcher.get({ endpoint: '/v1/address/list', showError: false });
  if (!response.ok) {
    // Keep whatever is showing; the profile page is still the way in.
    onActivate(() => go('/account/profile#addresses'));
    return;
  }

  let addresses = response.data || [];
  if (!addresses.length) {
    text.textContent = 'Add Address';
    writeCachedDeliver(null);
    onActivate(() => go(ADD_ADDRESS_PATH));
    return;
  }

  const current = () => addresses.find((a) => a.is_default) || addresses[0];
  const paint = () => {
    const label = deliverLabel(current());
    text.textContent = label;
    loc.title = 'Deliver to ' + label;
    writeCachedDeliver(label);
  };
  paint();

  loc.setAttribute('aria-haspopup', 'listbox');
  loc.setAttribute('aria-expanded', 'false');

  let menu = null;
  const onOutside = (e) => { if (!host.contains(e.target)) close(); };
  const onEscape = (e) => { if (e.key === 'Escape') { close(); loc.focus(); } };
  function close() {
    if (!menu) return;
    menu.remove();
    menu = null;
    loc.setAttribute('aria-expanded', 'false');
    document.removeEventListener('click', onOutside, true);
    document.removeEventListener('keydown', onEscape);
  }

  const choose = async (address) => {
    if (address.uuid === current().uuid) { close(); return; }
    close();
    const previous = current();
    // Optimistic: the bar changes at once, and is put back on failure.
    addresses = addresses.map((a) => ({ ...a, is_default: a.uuid === address.uuid }));
    paint();
    const result = await Fetcher.post({ endpoint: '/v1/address/' + address.uuid + '/default', body: {} });
    if (!result.ok) {
      addresses = addresses.map((a) => ({ ...a, is_default: a.uuid === previous.uuid }));
      paint();
      return;
    }
    toast.success('Delivering to ' + deliverLabel(address));
  };

  const open = () => {
    menu = document.createElement('div');
    menu.className = 'nav-deliver-menu';

    const title = document.createElement('div');
    title.className = 'nav-deliver-menu-title';
    title.textContent = 'Choose your delivery address';
    menu.appendChild(title);

    const list = document.createElement('div');
    list.setAttribute('role', 'listbox');
    list.setAttribute('aria-label', 'Saved addresses');
    addresses.forEach((address) => {
      const on = address.uuid === current().uuid;
      const option = document.createElement('div');
      option.className = 'nav-deliver-option' + (on ? ' selected' : '');
      option.setAttribute('role', 'option');
      option.setAttribute('aria-selected', String(on));
      option.tabIndex = 0;

      const name = document.createElement('div');
      name.className = 'nav-deliver-option-name';
      name.textContent = address.label || 'Address';
      const detail = document.createElement('div');
      detail.className = 'nav-deliver-option-detail';
      detail.textContent = [address.recipient_name, address.street, address.city].filter(Boolean).join(', ');
      option.append(name, detail);

      option.addEventListener('click', () => choose(address));
      option.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); choose(address); }
      });
      list.appendChild(option);
    });
    menu.appendChild(list);

    const foot = document.createElement('div');
    foot.className = 'nav-deliver-menu-foot';
    const add = document.createElement('a');
    add.href = ADD_ADDRESS_PATH;
    add.textContent = '+ Add a new address';
    const manage = document.createElement('a');
    manage.href = '/account/profile#addresses';
    manage.textContent = 'Manage addresses';
    foot.append(add, manage);
    menu.appendChild(foot);

    host.appendChild(menu);
    loc.setAttribute('aria-expanded', 'true');
    document.addEventListener('click', onOutside, true);
    document.addEventListener('keydown', onEscape);
    const selected = menu.querySelector('.nav-deliver-option.selected');
    if (selected) selected.focus();
  };

  onActivate(() => (menu ? close() : open()));
}

/*
  An admin browsing the shop sees it as a catalogue. The cart, wishlist,
  checkout and orders are customer-only on the server (require_shopper),
  so their buttons come off rather than failing on click. `is-staff` on
  <html> hides them (common.css), and the pages that are nothing but a
  cart or a checkout send the admin back to the shop.
*/
function standDownForStaff() {
  if (!session.isAdmin()) return;
  document.documentElement.classList.add('is-staff');
  if (/^\/(cart|checkout|wishlist)(\/|$)/.test(location.pathname)) location.replace('/');
}

standDownForStaff();
mountUser();
paintBadges();
mountDeliver();
