/* ═══════════════════════════════════════════════════════════════
   Bambe Web — wishlist.js

   Saved items. The cards are the shared product card with two
   things bolted on that only make sense here: the heart is a
   remove button rather than a toggle, and a price drop since the
   item was saved is called out — that is the whole point of a
   wishlist, and the wireframe drew the badge but had nothing to
   compute it from.

   Signed-out visitors have no wishlist at all (unlike the cart,
   which has a guest key), so the page asks them to sign in rather
   than showing an empty grid that looks like data loss.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { money, paintImage, emptyState, hideSkeleton } from './catalog.js';

const $ = (sel) => document.querySelector(sel);

let rows = [];

/* ── Card ─────────────────────────────────────────────────────  */
function wishlistCard(row) {
  const card = document.createElement('a');
  card.className = 'product-card';
  card.href = '/product?slug=' + encodeURIComponent(row.slug || '');
  card.style.textDecoration = 'none';
  card.style.color = 'inherit';

  const dropped = row.price_dropped && row.price_when_added > row.price_min;
  const off = dropped
    ? Math.round((1 - row.price_min / row.price_when_added) * 100)
    : null;

  const badge = !row.in_stock
    ? '<div class="product-card-badge" style="background:var(--input-bg);color:var(--mid);top:10px;left:10px;">Out of Stock</div>'
    : (off ? '<span class="product-card-badge badge-sale">–' + off + '%</span>' : '');

  card.innerHTML =
    '<div class="product-card-img">' + badge
      + '<div class="product-card-wish" style="background:rgba(211,47,47,0.1);" role="button" tabindex="0" aria-label="Remove from wishlist">'
        + '<svg width="14" height="13" viewBox="0 0 14 13" fill="none"><path d="M7 11.5S1 7.5 1 4a3 3 0 0 1 6-1 3 3 0 0 1 6 1c0 3.5-6 7.5-6 7.5Z" '
        + 'fill="var(--error)" stroke="var(--error)" stroke-width="1.3" stroke-linejoin="round"/></svg>'
      + '</div>'
    + '</div>'
    + '<div class="product-card-body">'
      + '<div class="product-card-name"></div>'
      + '<div class="product-card-price"></div>'
      + '<div class="wishlist-drop" style="font-size:11.5px;color:var(--error);font-weight:600;margin-top:4px;"></div>'
      + '<button class="btn btn-primary btn-sm" style="width:100%;margin-top:10px;border-radius:var(--radius-sm);"></button>'
    + '</div>';

  paintImage(card.querySelector('.product-card-img'), row.image);
  card.querySelector('.product-card-name').textContent = row.title || 'Unavailable';

  const price = card.querySelector('.product-card-price');
  const now = document.createElement('span');
  now.className = 'price';
  now.textContent = money(row.price_min);
  if (!row.in_stock) now.style.color = 'var(--subtle)';
  price.appendChild(now);

  if (dropped) {
    const then = document.createElement('span');
    then.className = 'price-old';
    then.textContent = money(row.price_when_added);
    price.appendChild(then);
  }

  /* Named in money, not just as a percentage — "$15.00 less than when
     you saved it" is the sentence the shopper is actually asking. */
  const drop = card.querySelector('.wishlist-drop');
  if (dropped) {
    drop.textContent = money(row.price_when_added - row.price_min) + ' less since you saved it';
  } else {
    drop.style.display = 'none';
  }

  const action = card.querySelector('.btn-primary');
  action.textContent = row.in_stock ? 'Add to Cart' : 'Notify Me';
  action.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (row.in_stock) addToCart(row, action);
    else notifyMe(row, action);
  });

  const heart = card.querySelector('.product-card-wish');
  heart.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    remove(row, card);
  });
  heart.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); heart.click(); }
  });

  return card;
}

/* ── Actions ──────────────────────────────────────────────────  */

/*
  The cart holds variants, the wishlist holds products — so a saved
  item with several colourways has no single thing to add. Rows saved
  against a specific variant go straight in; the rest go to the
  product page to choose, rather than picking a colour on the
  shopper's behalf.
*/
async function addToCart(row, button) {
  if (!row.variant_id) {
    location.href = '/product?slug=' + encodeURIComponent(row.slug || '');
    return;
  }

  button.classList.add('is-busy');
  const response = await Fetcher.post({
    endpoint: '/v1/cart/item',
    body: { variant_id: row.variant_id, quantity: 1 },
  });
  button.classList.remove('is-busy');

  if (response.ok) toast.success('Added to cart');
}

/*
  `notify_on_restock` is already on the row and defaults to true, so
  the alert is set the moment something is saved. The button says so
  rather than pretending to arrange something new.
*/
function notifyMe(row, button) {
  toast.info(row.notify_on_restock
    ? "You'll be emailed when this is back in stock"
    : 'Restock alerts are off for this item');
  button.disabled = true;
}

async function remove(row, card) {
  const response = await Fetcher.delete({ endpoint: '/v1/wishlist/' + row.uuid });
  if (!response.ok) return;

  rows = rows.filter((r) => r.uuid !== row.uuid);
  card.remove();
  renderHeader();
  if (!rows.length) render(); // fall through to the empty state
  toast.success('Removed from wishlist');
}

async function addAll() {
  const buyable = rows.filter((row) => row.in_stock && row.variant_id);
  if (!buyable.length) {
    toast.info('Nothing here can be added without picking a size first');
    return;
  }

  const results = await Promise.all(buyable.map((row) => Fetcher.post({
    endpoint: '/v1/cart/item',
    body: { variant_id: row.variant_id, quantity: 1 },
    showError: false,
  })));

  const added = results.filter((r) => r.ok).length;
  if (added) toast.success(added === 1 ? 'Added 1 item to cart' : 'Added ' + added + ' items to cart');
  if (added < buyable.length) toast.error((buyable.length - added) + " couldn't be added");
}

/* ── Header ───────────────────────────────────────────────────  */
function renderHeader() {
  const dropped = rows.filter((row) => row.price_dropped && row.price_when_added > row.price_min);

  const subtitle = $('.wishlist-subtitle');
  if (subtitle) {
    const saved = rows.length === 1 ? '1 item saved' : rows.length + ' items saved';
    subtitle.textContent = dropped.length
      ? saved + ' · ' + dropped.length + (dropped.length === 1 ? ' item on sale' : ' items on sale')
      : saved;
  }

  const addAllBtn = $('.wishlist-header .btn-primary');
  if (addAllBtn) {
    const buyable = rows.filter((row) => row.in_stock).length;
    addAllBtn.textContent = 'Add All to Cart (' + buyable + ')';
    addAllBtn.style.display = buyable ? '' : 'none';
  }

  // The sale banner is the block right after the header.
  const banner = $('.wishlist-header').nextElementSibling;
  if (banner) {
    banner.style.display = dropped.length ? '' : 'none';
    const text = banner.querySelector('span');
    if (text && dropped.length) {
      text.textContent = dropped.length === 1
        ? '1 item in your wishlist is now on sale!'
        : dropped.length + ' items in your wishlist are now on sale!';
    }
  }
}

/* ── Render ───────────────────────────────────────────────────  */
function grid() {
  return $('.product-grid');
}

function render() {
  const host = grid();
  if (!host) return;
  hideSkeleton(host);
  host.innerHTML = '';

  if (!rows.length) {
    emptyState(host, 'Your wishlist is empty', 'Tap the heart on anything you want to keep an eye on.');
    return;
  }

  const frag = document.createDocumentFragment();
  rows.forEach((row) => frag.appendChild(wishlistCard(row)));
  host.appendChild(frag);
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const host = grid();

  if (!session.isSignedIn()) {
    if (host) {
      hideSkeleton(host);
      emptyState(host, 'Sign in to see your wishlist', 'Saved items are kept on your account.');
    }
    renderHeaderSignedOut();
    setTimeout(() => session.requireSignedIn(), 1200);
    return;
  }

  const shareBtn = $('.wishlist-header .btn-ghost');
  if (shareBtn) {
    // Nothing on the backend produces a shareable wishlist link, and
    // a button that silently does nothing is worse than one that says
    // so. Hidden rather than wired to a URL that would 404.
    shareBtn.style.display = 'none';
  }

  const addAllBtn = $('.wishlist-header .btn-primary');
  if (addAllBtn) addAllBtn.onclick = addAll;

  const response = await Fetcher.get({ endpoint: '/v1/wishlist' });
  if (host) hideSkeleton(host);

  if (!response.ok) {
    if (host) emptyState(host, "Couldn't load your wishlist", 'Please try again in a moment.');
    return;
  }

  rows = response.data || [];
  renderHeader();
  render();
})();

function renderHeaderSignedOut() {
  const subtitle = $('.wishlist-subtitle');
  if (subtitle) subtitle.textContent = '';

  ['.wishlist-header .btn-primary', '.wishlist-header .btn-ghost'].forEach((sel) => {
    const el = $(sel);
    if (el) el.style.display = 'none';
  });

  const banner = $('.wishlist-header') ? $('.wishlist-header').nextElementSibling : null;
  if (banner) banner.style.display = 'none';
}
