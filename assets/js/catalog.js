/* ═══════════════════════════════════════════════════════════════
   Bambe Web — catalog.js

   The pieces every product surface needs: money, images, stars and
   the product card itself. The listing grid, search results,
   wishlist, recently-viewed and the home page rows all render the
   same card, so it is built once here rather than five times in
   five files that would drift apart.

   The markup produced matches what the wireframe drew by hand —
   same classes, same nesting — so the stylesheet needs no changes.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { toDataURL as blurhashToDataURL } from './utils/blurhash.js';

/* ── Money ────────────────────────────────────────────────────
   Prices cross the wire as `Cents` (i64) — never as floats — so
   they are integers here too until the last moment.            */
export function money(cents) {
  if (cents === null || cents === undefined) return '';
  const sign = cents < 0 ? '-' : '';
  const abs = Math.abs(cents);
  return sign + '$' + Math.floor(abs / 100).toLocaleString() + '.' + String(abs % 100).padStart(2, '0');
}

/** "$29.99", or "$29.99 – $54.99" when the variants disagree. */
export function priceRange(item) {
  if (item.price_min === item.price_max) return money(item.price_min);
  return money(item.price_min) + ' – ' + money(item.price_max);
}

/* ── Images ───────────────────────────────────────────────────
   The image pipeline serves by id, outside /api. `image` comes
   back as a whole ImageStruct on some endpoints and a bare id on
   others, so both are accepted.                                */
function imageId(image) {
  return !image ? null : (typeof image === 'string' ? image : image.uuid);
}

export function imageUrl(image) {
  const id = imageId(image);
  return id ? '/image/webp/' + id : null;
}

/* ── Blur hashes by id ────────────────────────────────────────
   Many endpoints hand back a bare image id — cart lines, orders,
   categories, avatars — with no blur hash beside it. Those are
   looked up through `POST /image/metadata`, which takes a list of
   ids, so every image asked for in one render pass shares a
   single request. Known hashes are kept for the page's lifetime;
   `null` records an image that has none, so it isn't asked again. */
const blurHashes = new Map();   // id → hash | null
const waiting = new Map();      // id → [resolve, …] for the batch in flight
let flushTimer = null;

function flushBlurHashes() {
  flushTimer = null;
  const batch = new Map(waiting);
  waiting.clear();
  const ids = [...batch.keys()];

  const settle = (found) => {
    ids.forEach((id) => {
      const hash = found.get(id) || null;
      blurHashes.set(id, hash);
      batch.get(id).forEach((resolve) => resolve(hash));
    });
  };

  fetch((window.projectDomain || '') + '/image/metadata', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(ids),
  })
    .then((response) => (response.ok ? response.json() : []))
    .then((images) => settle(new Map((images || []).map((i) => [i.uuid, i.blur_hash]))))
    // No hash is not an error worth showing: the photo still loads.
    .catch(() => settle(new Map()));
}

function lookupBlurHash(id) {
  if (blurHashes.has(id)) return Promise.resolve(blurHashes.get(id));
  return new Promise((resolve) => {
    if (!waiting.has(id)) waiting.set(id, []);
    waiting.get(id).push(resolve);
    if (!flushTimer) flushTimer = setTimeout(flushBlurHashes, 0);
  });
}

/*
  Paints an element with an image from the database — every one of
  them goes through here. The photo's blur hash is painted under it
  as a second background layer, so the space shows the photo's own
  colours instead of a grey hole while the file downloads, and the
  photo covers it as it arrives.

  `image` is a whole ImageStruct or a bare id. A struct brings its
  hash along and the blur shows on the first frame; for a bare id the
  photo starts loading at once and the hash is fetched beside it, the
  blur sliding in underneath if the photo hasn't beaten it.

  Once the photo has loaded the blur layer is dropped, so a shot with
  transparency doesn't show colour through it. The element remembers
  which url it was last asked for: the gallery repaints the same
  element as you click through, and a slow earlier photo must not
  land over a later one.

  `options.property` writes a custom property instead of the
  `background` shorthand — for elements whose stylesheet places the
  picture itself (the home page's `--img`). It receives the layers
  as a `background-image` list.
*/
export function paintImage(el, image, options = {}) {
  const id = imageId(image);
  if (!id) return;
  const url = '/image/webp/' + id;

  if (image && typeof image === 'object' && image.blur_hash) blurHashes.set(id, image.blur_hash);

  let loaded = false;
  const paint = (hash) => {
    if (el.dataset.imageUrl !== url) return;
    const placeholder = !loaded && hash ? blurhashToDataURL(hash) : null;
    const layers = ["url('" + url + "')"];
    if (placeholder) layers.push("url('" + placeholder + "')");

    if (options.property) {
      el.style.setProperty(options.property, layers.join(', '));
    } else {
      el.style.background = layers.map((layer) => layer + ' center / cover no-repeat').join(', ');
    }
  };

  el.dataset.imageUrl = url;
  const known = blurHashes.get(id);
  paint(known || null);

  const loader = new Image();
  loader.onload = () => { loaded = true; paint(null); };
  loader.src = url;

  if (known === undefined) {
    lookupBlurHash(id).then((hash) => { if (!loaded) paint(hash); });
  }
}

/* Takes a picture back off an element painted by paintImage(), so a
   late-arriving photo or blur can't land on it afterwards. */
export function clearImage(el, options = {}) {
  delete el.dataset.imageUrl;
  if (options.property) el.style.removeProperty(options.property);
  else el.style.background = '';
}

/* ── Stars ────────────────────────────────────────────────────
   Whole stars only, matching how the wireframe drew them: filled
   in the brand amber, empty in the border grey.                */
const STAR_PATH = 'M6 1l1.1 3.4H11L8.2 6.5l1 3.4L6 8.1 2.8 9.9l1-3.4L1 4.4h3.9L6 1Z';

export function stars(average) {
  const filled = Math.round(average || 0);
  let out = '';
  for (let i = 1; i <= 5; i++) {
    out += '<svg class="star" viewBox="0 0 12 12" fill="' + (i <= filled ? '#C8A772' : '#E8E5E0') + '">'
         + '<path d="' + STAR_PATH + '"/></svg>';
  }
  return out;
}

/* ── Discount badge ───────────────────────────────────────────
   Derived, never stored: the badge is just what compare_at_price
   says against the real price, so it cannot contradict them.   */
function discountPercent(item) {
  if (!item.compare_at_price || item.compare_at_price <= item.price_min) return null;
  return Math.round((1 - item.price_min / item.compare_at_price) * 100);
}

/* ── Product card ─────────────────────────────────────────────
   One card, built to the same shape the wireframe drew.        */
export function productCard(item, options = {}) {
  const card = document.createElement('a');
  card.className = 'product-card';
  card.href = '/product?slug=' + encodeURIComponent(item.slug);
  card.style.textDecoration = 'none';
  card.style.color = 'inherit';
  card.dataset.productId = item.uuid;
  card.dataset.slug = item.slug;

  const off = discountPercent(item);
  const badge = off ? '<span class="product-card-badge badge-sale">–' + off + '%</span>'
              : (!item.in_stock ? '<span class="product-card-badge">Sold out</span>' : '');

  /* Per item, not per grid — `options.wished` would paint every card
     in the grid the same. Falls back to the shared index, which is
     empty (so: not saved) until someone loads it. */
  const wished = options.wished !== undefined ? !!options.wished : isWished(item.uuid);
  const heart = '<div class="product-card-wish" role="button" tabindex="0" aria-pressed="' + wished + '" aria-label="Save for later">'
    + '<svg width="14" height="13" viewBox="0 0 14 13" fill="none"><path d="M7 11.5S1 7.5 1 4a3 3 0 0 1 6-1 3 3 0 0 1 6 1c0 3.5-6 7.5-6 7.5Z" '
    + 'fill="' + (wished ? 'var(--blue)' : 'none') + '" stroke="' + (wished ? 'var(--blue)' : '#C4C0B9') + '" stroke-width="1.3" stroke-linejoin="round"/></svg></div>';

  const oldPrice = item.compare_at_price && item.compare_at_price > item.price_min
    ? '<span class="price-old">' + money(item.compare_at_price) + '</span>' : '';

  card.innerHTML =
    '<div class="product-card-img">' + badge + heart
      + '<div class="product-card-quick"><button class="quick-add-btn" type="button">Quick Add</button></div>'
    + '</div>'
    + '<div class="product-card-body">'
      + '<div class="product-card-brand"></div>'
      + '<div class="product-card-name"></div>'
      + '<div class="product-card-rating"><div class="stars">' + stars(item.rating_average) + '</div>'
        + '<span class="rating-count"></span></div>'
      + '<div class="product-card-price"><span class="price">' + priceRange(item) + '</span>' + oldPrice + '</div>'
    + '</div>';

  // Titles and brand names are user data, so they go in as text —
  // innerHTML above only ever carries markup this file wrote.
  card.querySelector('.product-card-brand').textContent = item.brand ? item.brand.name : '';
  card.querySelector('.product-card-name').textContent = item.title;
  card.querySelector('.rating-count').textContent = '(' + (item.rating_count || 0) + ')';

  paintImage(card.querySelector('.product-card-img'), item.image);

  /* The heart and Quick Add sit inside a link, so both have to stop
     the click from also navigating to the product. */
  const wish = card.querySelector('.product-card-wish');
  wish.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    toggleWishlist(item, wish);
  });
  wish.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); wish.click(); }
  });

  card.querySelector('.quick-add-btn').addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (options.onQuickAdd) options.onQuickAdd(item, card);
    else location.href = card.href;
  });

  return card;
}

/* ── Wishlist ─────────────────────────────────────────────────
   `DELETE /v1/wishlist/{id}` takes the id of the *wishlist row*,
   not of the product — so removing something needs the row id,
   which only `GET /v1/wishlist` knows. This index holds that
   mapping for the page, and every heart on every surface reads
   the same copy, so a product saved from the listing shows as
   saved on its detail page without a second round trip.        */
const wishlistRows = new Map(); // product_id -> wishlist row uuid
let wishlistLoaded = false;

export async function loadWishlist({ force = false } = {}) {
  if (!session.isSignedIn()) {
    wishlistRows.clear();
    wishlistLoaded = true;
    return wishlistRows;
  }
  if (wishlistLoaded && !force) return wishlistRows;

  const response = await Fetcher.get({ endpoint: '/v1/wishlist', showError: false });
  wishlistRows.clear();
  // The endpoint answers with a bare array, not a paged envelope.
  if (response.ok) (response.data || []).forEach((row) => wishlistRows.set(row.product_id, row.uuid));

  wishlistLoaded = true;
  return wishlistRows;
}

export function isWished(productId) {
  return wishlistRows.has(productId);
}

export function paintHeart(heartEl, on) {
  if (!heartEl) return;
  heartEl.setAttribute('aria-pressed', String(on));
  const path = heartEl.querySelector('path');
  if (!path) return;
  path.setAttribute('fill', on ? 'var(--blue)' : 'none');
  path.setAttribute('stroke', on ? 'var(--blue)' : '#C4C0B9');
}

/*
  Wishlist needs an account — it is stored against the user, with no
  guest equivalent the way the cart has one. Rather than let the
  heart fail with a 401, send them to sign in and come back.
*/
export async function toggleWishlist(item, heartEl) {
  if (!session.isSignedIn()) {
    toast.info('Sign in to save items');
    setTimeout(() => session.requireSignedIn(), 700);
    return;
  }

  const on = heartEl.getAttribute('aria-pressed') === 'true';

  if (on) {
    const rowId = wishlistRows.get(item.uuid);
    if (!rowId) return; // nothing to remove — the index and the heart disagree
    const response = await Fetcher.delete({ endpoint: '/v1/wishlist/' + rowId });
    if (!response.ok) return;
    wishlistRows.delete(item.uuid);
  } else {
    const response = await Fetcher.post({
      endpoint: '/v1/wishlist',
      body: { product_id: item.uuid },
    });
    if (!response.ok) return;
    // The endpoint is idempotent and returns the row either way, so
    // this is the id a later removal needs.
    wishlistRows.set(item.uuid, response.data.uuid);
  }

  paintHeart(heartEl, !on);
  toast.success(!on ? 'Saved to wishlist' : 'Removed from wishlist');
}

/* ── Empty and error states ───────────────────────────────────
   A grid that just goes blank reads as a broken page, so say
   which of the two happened.                                   */
export function emptyState(container, title, detail) {
  container.innerHTML = '';
  const box = document.createElement('div');
  box.className = 'catalog-empty';
  box.style.cssText = 'grid-column:1/-1;padding:56px 20px;text-align:center;';
  box.innerHTML = '<div style="font-family:var(--font-head);font-size:17px;font-weight:600;color:var(--dark);margin-bottom:6px;"></div>'
                + '<div style="font-size:13.5px;color:var(--subtle);"></div>';
  box.children[0].textContent = title;
  box.children[1].textContent = detail || '';
  container.appendChild(box);
}

/* ── Skeletons ────────────────────────────────────────────────
   skeleton.js mounts a placeholder over anything marked
   `data-skeleton` and takes it down on a fixed timer — which was
   right when there was no request to wait for. Now there is one,
   so `hideSkeleton` takes it down the moment the data lands, and
   `showSkeleton` puts it back for a reload the timer has already
   passed (a facet click on a page that has been open a while).  */
export function showSkeleton(container) {
  if (container && container.dataset.skeleton !== undefined) {
    container.classList.add('is-loading');
  }
}

export function hideSkeleton(container) {
  if (!container) return;
  container.classList.remove('is-loading');
  if (window.Skeleton) window.Skeleton.clear(container);
}

/*
  Fills a container with cards, or the right empty state. Returns
  the response so the caller can still read `total` for its count
  and pagination.
*/
export function renderGrid(container, items, options = {}) {
  // Real content is about to replace the sample cards, so the
  // placeholder goes — the same thing fillTable() does for tables.
  if (window.Skeleton) window.Skeleton.clear(container);
  container.innerHTML = '';
  if (!items || !items.length) {
    emptyState(container,
      options.emptyTitle || 'Nothing here yet',
      options.emptyDetail || 'Try removing a filter, or check back soon.');
    return;
  }
  const frag = document.createDocumentFragment();
  items.forEach((item) => frag.appendChild(productCard(item, options)));
  container.appendChild(frag);
}
