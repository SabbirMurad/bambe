/* ═══════════════════════════════════════════════════════════════
   Bambee Web — product.js

   The detail page. Everything here hangs off one call to
   `GET /v1/product/{slug}` plus a separate reviews page.

   The central idea is that the page shows a *variant*, not a
   product. The title, description and gallery belong to the
   product, but the price, the stock line, the discount badge and
   what "Add to Cart" actually adds all come from whichever
   colour/size pair is selected. The wireframe drew those as fixed
   numbers; here they move together, so the page can never show
   $29.99 beside a size that costs $32.99.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import {
  money, paintImage, stars, renderGrid, toggleWishlist, loadWishlist, isWished,
  emptyState,
} from './catalog.js';
import { openReviewComposer } from './review-form.js';

/* ── State ────────────────────────────────────────────────────  */
let product = null;
let selected = null;      // the chosen ProductVariant
let quantity = 1;
let galleryIndex = 0;
let reviewPage = 1;

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => [...document.querySelectorAll(sel)];

/* /product/<slug>, the canonical address. The old ?slug= form is
   redirected by the server, but is still read here in case a cached
   page asks. */
function slugFromUrl() {
  const match = location.pathname.match(/^\/product\/([^/]+)/);
  if (match) return decodeURIComponent(match[1]);
  return new URLSearchParams(location.search).get('slug') || '';
}

/* ── Gallery ──────────────────────────────────────────────────
   Every photo of every variant, in the order `product.gallery` holds
   them (variant by variant, main photo first). Picking an option
   brings the chosen variant's main photo up; picking a photo that
   belongs to other variants switches the options to one of them. A
   photo the selected variant already has, or one shared by several
   variants, leaves the options alone.                           */
function photos() {
  return product.gallery || [];
}

// The variants a photo belongs to: as its main photo or as an extra.
function ownersOf(imageId) {
  return (product.variants || []).filter((v) =>
    v.image_id === imageId || (v.extra_image_ids || []).includes(imageId));
}

function renderGallery() {
  const thumbs = $('.pd-thumbnails');
  const main = $('.pd-main-img-bg');
  const dots = $('.pd-img-dots');
  if (!thumbs || !main) return;

  const images = photos();
  thumbs.innerHTML = '';
  if (dots) dots.innerHTML = '';

  // No photos: a plain frame, not a loading shimmer that never ends.
  if (!images.length) main.style.background = 'var(--input-bg)';

  images.forEach((entry, index) => {
    const thumb = document.createElement('div');
    thumb.className = 'pd-thumb' + (index === galleryIndex ? ' active' : '');
    thumb.setAttribute('role', 'button');
    thumb.setAttribute('tabindex', '0');
    thumb.setAttribute('aria-label', 'View image ' + (index + 1));
    paintImage(thumb, entry.image || entry.image_id);
    thumb.addEventListener('click', () => pickImage(index));
    thumb.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickImage(index); }
    });
    thumbs.appendChild(thumb);

    if (dots) {
      const dot = document.createElement('div');
      dot.className = 'pd-dot' + (index === galleryIndex ? ' active' : '');
      dot.addEventListener('click', () => pickImage(index));
      dots.appendChild(dot);
    }
  });

  showImage(galleryIndex);
}

function showImage(index) {
  const images = photos();
  if (!images.length) return;

  galleryIndex = Math.max(0, Math.min(index, images.length - 1));
  paintImage($('.pd-main-img-bg'), images[galleryIndex].image || images[galleryIndex].image_id);

  $$('.pd-thumb').forEach((el, i) => el.classList.toggle('active', i === galleryIndex));
  $$('.pd-dot').forEach((el, i) => el.classList.toggle('active', i === galleryIndex));
  // Keep the active photo in view inside the strip, without moving the page.
  const strip = $('.pd-thumbnails');
  const active = $$('.pd-thumb')[galleryIndex];
  if (strip && active) {
    const top = active.offsetTop - strip.offsetTop;
    if (top < strip.scrollTop) strip.scrollTop = top;
    else if (top + active.offsetHeight > strip.scrollTop + strip.clientHeight) {
      strip.scrollTop = top + active.offsetHeight - strip.clientHeight;
    }
  }
}

/* A photo picked by the shopper. If the selected variant isn't in it,
   switch to one that is: the one agreeing with the most current
   choices, in stock if any is. The photo picked stays on show. */
function pickImage(index) {
  showImage(index);
  const entry = photos()[galleryIndex];
  if (!entry || !selected) return;

  const owners = ownersOf(entry.image_id);
  if (!owners.length || owners.includes(selected)) return;

  const agreement = (v) => productOptions()
    .filter((o) => valueOf(v, o.name) === valueOf(selected, o.name)).length;
  const best = owners
    .map((v, i) => ({ v, i, score: agreement(v) * 2 + (v.stock > 0 ? 1 : 0) }))
    .sort((x, y) => y.score - x.score || x.i - y.i)[0].v;
  pickVariant(best, { keepImage: true });
}

/* Picking an option brings up the chosen variant's main photo. */
function jumpGalleryToVariant() {
  if (!selected) return;
  const ids = [selected.image_id, ...(selected.extra_image_ids || [])];
  const index = photos().findIndex((g) => ids.includes(g.image_id));
  if (index >= 0) showImage(index);
}

/* ── Full-screen viewer ───────────────────────────────────────
   Clicking the main photo opens every photo full screen, at the one
   on show (PhotoSwipe, vendored in ./vendor/photoswipe.mjs — MIT):
   swipe or arrow keys to move, pinch, double-tap or click to zoom,
   Esc to close. It is fetched on first use, not with the page.
   Moving between photos in the viewer picks them on the page too,
   so the options under it are right when it closes.            */
async function openViewer() {
  const images = photos();
  if (!images.length) return;

  const { default: PhotoSwipe } = await import('./vendor/photoswipe.mjs');
  const viewer = new PhotoSwipe({
    dataSource: images.map((entry) => {
      const image = entry.image || {};
      const src = '/image/webp/' + entry.image_id;
      return {
        src,
        msrc: src,
        // Uploads record their size; a missing one is guessed square,
        // which PhotoSwipe corrects once the photo has loaded.
        width: image.width || 1200,
        height: image.height || 1200,
        alt: product.title,
      };
    }),
    index: galleryIndex,
    bgOpacity: 0.92,
    showHideAnimationType: 'fade',
    wheelToZoom: true,
    closeTitle: 'Close',
    zoomTitle: 'Zoom',
    arrowPrevTitle: 'Previous photo',
    arrowNextTitle: 'Next photo',
  });

  // The page's smooth scrolling would otherwise take the wheel.
  viewer.on('openingAnimationStart', () => window.lenis && window.lenis.stop());
  viewer.on('destroy', () => window.lenis && window.lenis.start());
  viewer.on('change', () => pickImage(viewer.currIndex));
  viewer.init();
}

function wireViewer() {
  const main = $('.pd-main-img');
  if (!main) return;
  main.setAttribute('role', 'button');
  main.setAttribute('tabindex', '0');
  main.setAttribute('aria-label', 'View photos full screen');
  main.addEventListener('click', (e) => {
    // The dots under the photo pick a photo; they don't open the viewer.
    if (e.target.closest('.pd-img-dots')) return;
    openViewer();
  });
  main.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openViewer(); }
  });
}

/* ── Header block ─────────────────────────────────────────────  */
function renderHeader() {
  const crumb = $('.breadcrumb');
  if (crumb) {
    crumb.innerHTML = '';
    const home = document.createElement('a');
    home.className = 'breadcrumb-item';
    home.href = '/';
    home.textContent = 'Home';
    crumb.appendChild(home);

    (product.breadcrumb || []).forEach((node) => {
      crumb.appendChild(sep());
      const link = document.createElement('a');
      link.className = 'breadcrumb-item';
      link.href = '/listing?category=' + encodeURIComponent(node.slug);
      link.textContent = node.name;
      crumb.appendChild(link);
    });

    crumb.appendChild(sep());
    const current = document.createElement('span');
    current.className = 'breadcrumb-item current';
    current.textContent = product.title;
    crumb.appendChild(current);
  }

  const brand = $('.pd-brand-name');
  if (brand) {
    brand.textContent = product.brand ? product.brand.name : '';
    // Hidden rather than left empty: an empty one is the loading bar.
    brand.style.display = product.brand ? '' : 'none';
    if (product.brand) {
      brand.style.cursor = 'pointer';
      brand.onclick = () => { location.href = '/listing?brand=' + encodeURIComponent(product.brand.uuid); };
    }
  }

  // Shown only to an Owner or Manager previewing an unpublished product.
  const draftTag = $('.pd-draft-tag');
  if (draftTag) draftTag.style.display = product.status === 'Draft' ? '' : 'none';

  const title = $('.pd-title');
  if (title) title.textContent = product.title;
  document.title = product.title + ' — Bambee';

  const score = $('.pd-rating-score');
  if (score) score.textContent = (product.rating_average || 0).toFixed(1);

  const starRow = $('.pd-rating-row .stars');
  if (starRow) starRow.innerHTML = stars(product.rating_average);

  const reviewCount = $('.pd-review-count');
  if (reviewCount) {
    reviewCount.textContent = product.rating_count === 1
      ? '1 review' : (product.rating_count || 0).toLocaleString() + ' reviews';
  }

  const sold = $('.pd-sold');
  if (sold) {
    sold.textContent = (product.sold_count || 0).toLocaleString() + ' sold';
    sold.style.display = product.sold_count ? '' : 'none';
  }

  const description = $('.body-lg');
  if (description) {
    description.textContent = product.description || '';
    description.style.display = product.description ? '' : 'none';
  }

  // The bullet list under the description is the product's tags.
  const bullets = description ? description.parentElement.querySelector('ul') : null;
  if (bullets) {
    bullets.innerHTML = '';
    (product.tags || []).forEach((tag) => {
      const li = document.createElement('li');
      li.textContent = tag;
      bullets.appendChild(li);
    });
    bullets.style.display = (product.tags || []).length ? '' : 'none';
  }
}

function sep() {
  const s = document.createElement('span');
  s.className = 'breadcrumb-sep';
  s.textContent = '/';
  return s;
}

/* ── Variant pickers ──────────────────────────────────────────
   One picker per option the product has, named by the admin
   ("Shade", "Volume", "Skin type"), in the product's order. Each
   picker offers the values found among the variants that match the
   choices made above it, so a later picker never offers a
   combination that doesn't exist. The swatch option draws dots in
   each variant's colour; the rest draw text buttons.            */
function productOptions() {
  return product.options || [];
}

function valueOf(variant, name) {
  const pair = (variant.options || []).find((o) => o.name === name);
  return pair ? pair.value : '';
}

// Variants agreeing with the current selection on every option before `level`.
function candidates(level) {
  const opts = productOptions();
  return (product.variants || []).filter((v) =>
    opts.slice(0, level).every((o) => valueOf(v, o.name) === valueOf(selected, o.name)));
}

/* The variant to switch to when `value` is picked at `level`: keep the
   later choices where that combination exists, and prefer something
   in stock, so the picker never lands on a sold-out variant if it
   doesn't have to. */
function bestFor(level, value) {
  const opts = productOptions();
  const pool = candidates(level).filter((v) => valueOf(v, opts[level].name) === value);
  const keeps = (v) => opts.slice(level + 1).every((o) => valueOf(v, o.name) === valueOf(selected, o.name));
  return pool.find((v) => keeps(v) && v.stock > 0)
    || pool.find((v) => v.stock > 0)
    || pool.find(keeps)
    || pool[0];
}

function renderOptions() {
  const host = $('.pd-options');
  if (!host) return;
  host.innerHTML = '';
  if (!selected) return;

  productOptions().forEach((option, level) => {
    const pool = candidates(level);
    const values = [];
    pool.forEach((v) => {
      const value = valueOf(v, option.name);
      if (value && !values.includes(value)) values.push(value);
    });
    if (!values.length) return;

    const current = valueOf(selected, option.name);
    const label = document.createElement('div');
    label.className = 'pd-section-label';
    label.textContent = option.name + ' — ';
    const strong = document.createElement('strong');
    strong.style.cssText = 'color:var(--dark);font-weight:700;';
    strong.textContent = current;
    label.appendChild(strong);
    host.appendChild(label);

    const row = document.createElement('div');
    row.className = option.swatch ? 'pd-swatches' : 'pd-sizes';
    row.setAttribute('role', 'radiogroup');
    row.setAttribute('aria-label', option.name);

    values.forEach((value) => {
      const withValue = pool.filter((v) => valueOf(v, option.name) === value);
      const out = withValue.every((v) => v.stock <= 0);
      const active = value === current;
      const el = document.createElement('div');
      el.setAttribute('role', 'radio');
      el.setAttribute('aria-checked', String(active));
      el.setAttribute('aria-label', value + (out ? ' (out of stock)' : ''));
      el.tabIndex = active ? 0 : -1;

      if (option.swatch) {
        el.className = 'pd-swatch' + (active ? ' active' : '') + (out ? ' out' : '');
        el.style.background = (withValue.find((v) => v.color_hex) || {}).color_hex || '#ccc';
        el.title = value + (out ? ' — out of stock' : '');
        // A white shade on a white card needs its own outline.
        el.style.boxShadow = 'inset 0 0 0 1px var(--border)';
      } else {
        el.className = 'pd-size' + (active ? ' active' : '') + (out ? ' out' : '');
        el.textContent = value;
        if (out) el.title = 'Out of stock';
      }

      const pick = () => pickVariant(bestFor(level, value));
      el.addEventListener('click', pick);
      el.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
      });
      row.appendChild(el);
    });
    host.appendChild(row);
  });
}

function pickVariant(variant, { keepImage = false } = {}) {
  if (!variant) return;
  selected = variant;
  quantity = Math.min(quantity, Math.max(1, variant.stock));
  renderOptions();
  renderPrice();
  renderQuantity();
  renderStock();
  if (!keepImage) jumpGalleryToVariant();
}

/* ── Price, stock, quantity ───────────────────────────────────  */
function renderPrice() {
  const price = $('.pd-price');
  const old = $('.pd-price-old');
  const badge = $('.pd-discount-badge');
  if (!price || !selected) return;

  price.textContent = money(selected.price);

  const compare = selected.compare_at_price;
  const discounted = compare && compare > selected.price;

  if (old) {
    old.textContent = discounted ? money(compare) : '';
    old.style.display = discounted ? '' : 'none';
  }
  if (badge) {
    badge.textContent = discounted
      ? Math.round((1 - selected.price / compare) * 100) + '% off' : '';
    badge.style.display = discounted ? '' : 'none';
  }
}

function renderQuantity() {
  const value = $('.qty-val');
  const stockLine = $('.qty-stock');
  if (value) value.textContent = String(quantity);

  if (stockLine && selected) {
    const icon = stockLine.querySelector('svg');
    stockLine.innerHTML = '';
    if (icon) stockLine.appendChild(icon);

    const text = document.createTextNode(stockText());
    stockLine.appendChild(text);
    stockLine.style.color = selected.stock <= 0 ? 'var(--error)' : '';
    if (icon) icon.style.display = selected.stock <= 0 ? 'none' : '';
  }
}

/* The badge by the brand and the buy buttons follow the option picked,
   not the product as a whole: a sold-out size reads "Out of stock" and
   can't be added, while its in-stock siblings still can. "Sold out"
   is kept for when no option has any left. */
function renderStock() {
  if (!selected) return;
  const available = selected.stock > 0;

  const tag = $('.pd-stock-tag');
  if (tag) {
    const anyLeft = (product.variants || []).some((v) => v.stock > 0);
    tag.classList.toggle('tag-green', available);
    tag.classList.toggle('tag-gray', !available);
    tag.querySelector('.pd-stock-label').textContent =
      available ? 'In Stock' : anyLeft ? 'Out of stock' : 'Sold out';
    const tick = tag.querySelector('svg');
    if (tick) tick.style.display = available ? '' : 'none';
    tag.style.display = '';
  }

  ['.pd-add-cart', '.pd-buy-now'].forEach((sel) => {
    const button = $(sel);
    if (!button) return;
    button.disabled = !available;
    button.title = available ? '' : 'This option is out of stock';
  });
}

/*
  Low stock is the merchant's own threshold, not a made-up number:
  the variant carries `low_stock_threshold`, so "Only 3 left" appears
  exactly when they wanted it to.
*/
function stockText() {
  if (selected.stock <= 0) return ' Out of stock';
  const threshold = selected.low_stock_threshold || 0;
  if (threshold && selected.stock <= threshold) {
    return ' Only ' + selected.stock + ' left in stock';
  }
  return ' In stock';
}

function wireQuantity() {
  const buttons = $$('.qty-ctrl .qty-btn');
  if (buttons.length < 2) return;

  buttons[0].addEventListener('click', () => {
    if (quantity > 1) { quantity--; renderQuantity(); }
  });
  buttons[1].addEventListener('click', () => {
    // Capped at what is actually on the shelf — the server rejects
    // more than that anyway, and failing at checkout is a worse
    // place to learn it than here.
    if (selected && quantity >= selected.stock) {
      toast.info('Only ' + selected.stock + ' in stock');
      return;
    }
    quantity++;
    renderQuantity();
  });
}

/* ── Cart and wishlist ────────────────────────────────────────  */
async function addToCart() {
  if (!selected) return false;
  if (selected.stock <= 0) {
    toast.error('That option is out of stock');
    return false;
  }

  const button = $('.pd-add-cart');
  if (button) button.classList.add('is-busy');

  const response = await Fetcher.post({
    endpoint: '/v1/cart/item',
    body: { variant_id: selected.uuid, quantity },
  });

  if (button) button.classList.remove('is-busy');
  if (response.ok) toast.success('Added to cart');
  return response.ok;
}

function wireCta() {
  const add = $('.pd-add-cart');
  if (add) add.onclick = addToCart;

  const buy = $('.pd-buy-now');
  if (buy) {
    buy.onclick = async () => {
      // Buy Now is Add to Cart plus a redirect. It must not jump to
      // the cart when the add failed, or the cart looks empty for no
      // stated reason.
      if (await addToCart()) location.href = '/cart';
    };
  }

  const heart = $('.pd-wish-btn');
  if (heart) {
    heart.setAttribute('role', 'button');
    heart.setAttribute('tabindex', '0');
    heart.classList.remove('active');
    paintHeart(false);
    heart.onclick = toggleWish;
    heart.onkeydown = (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleWish(); }
    };
  }
}

function paintHeart(on) {
  const heart = $('.pd-wish-btn');
  if (!heart) return;
  const path = heart.querySelector('path');
  heart.classList.toggle('active', on);
  heart.setAttribute('aria-pressed', String(on));
  if (path) {
    path.setAttribute('fill', on ? 'var(--error)' : 'none');
    path.setAttribute('stroke', on ? 'var(--error)' : '#C4C0B9');
  }
}

/*
  The heart saves the *product*, not the selected variant. Adding a
  variant id would make each colourway its own wishlist row, so
  saving a tee in white and then switching to black would show an
  empty heart on a product already saved.
*/
async function toggleWish() {
  await toggleWishlist({ uuid: product.uuid }, $('.pd-wish-btn'));
  paintHeart(isWished(product.uuid));
}

/* Reflects a wishlist the visitor already has, so the heart isn't
   empty on a product they saved last week. */
async function syncWishState() {
  await loadWishlist();
  paintHeart(isWished(product.uuid));
}

/* ── Reviews ──────────────────────────────────────────────────  */
function reviewCard(review) {
  const card = document.createElement('div');
  card.className = 'review-card-web';
  card.innerHTML =
    '<div class="review-header">'
      + '<div class="reviewer-info">'
        + '<div class="reviewer-name"></div>'
        + '<div class="review-meta"></div>'
      + '</div>'
      + '<div class="stars">' + stars(review.rating) + '</div>'
    + '</div>'
    + '<p class="review-title" style="font-size:13.5px;font-weight:700;color:var(--dark);margin:0 0 6px;"></p>'
    + '<p class="review-text"></p>';

  // The list endpoint calls this field `author` (review/list.rs), not
  // `author_name` — reading the wrong one signed every review "Anonymous".
  card.querySelector('.reviewer-name').textContent = review.author || 'Anonymous';
  card.querySelector('.review-text').textContent = review.body || '';

  const headline = card.querySelector('.review-title');
  if (review.title) headline.textContent = review.title;
  else headline.remove();

  const bought = [];
  if (review.variant_label) bought.push(review.variant_label);
  if (review.created_at) bought.push(new Date(review.created_at).toLocaleDateString());
  if (review.scope === 'Order') bought.push('Reviewed with the whole order');
  card.querySelector('.review-meta').textContent = bought.length ? 'Purchased: ' + bought.join(' · ') : '';

  if (review.verified_purchase) {
    const badge = document.createElement('div');
    badge.className = 'review-verified-web';
    badge.innerHTML = '<svg width="12" height="12" viewBox="0 0 12 12" fill="none">'
      + '<circle cx="6" cy="6" r="5.5" fill="var(--success)" opacity=".15"/>'
      + '<path d="M3.5 6l2 2 3-3.5" stroke="var(--success)" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>'
      + ' Verified Purchase';
    card.appendChild(badge);
  }

  /* A staff reply is stored on the review itself and was never drawn —
     so a merchant who answered a complaint answered into the void. */
  if (review.reply && review.reply.body) {
    const reply = document.createElement('div');
    reply.style.cssText = 'margin-top:12px;padding:12px 14px;background:var(--input-bg);'
      + 'border-radius:var(--radius-sm);border-left:2px solid var(--blue);';
    reply.innerHTML = '<div style="font-size:12px;font-weight:700;color:var(--blue);margin-bottom:4px;">'
      + 'Response from Bambee</div>'
      + '<div class="review-reply-body" style="font-size:13px;color:var(--subtle);line-height:1.6;"></div>';
    reply.querySelector('.review-reply-body').textContent = review.reply.body;
    card.appendChild(reply);
  }

  card.appendChild(helpfulButton(review));
  return card;
}

/* ── Helpful votes ────────────────────────────
   `POST /v1/review/{id}/vote` toggles the caller's vote and answers
   with `{ helpful }` — the state *after* the click — so the count moves
   by what the server says happened rather than by what was assumed.

   The list endpoint returns `helpful_count` but not whether this
   caller has already voted, so a freshly loaded card always starts
   unpressed. Clicking it twice still withdraws the vote correctly;
   it just cannot be drawn as pressed on load.                       */
function helpfulButton(review) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;align-items:center;gap:8px;margin-top:12px;';

  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'btn btn-ghost btn-sm';
  button.setAttribute('aria-pressed', 'false');
  button.style.cssText = 'display:inline-flex;align-items:center;gap:6px;';
  button.innerHTML = '<svg width="13" height="13" viewBox="0 0 14 14" fill="none">'
    + '<path d="M4 6.5v6H2v-6h2Zm0 0L7 1a1.6 1.6 0 0 1 1.6 1.6V5h3a1.2 1.2 0 0 1 1.2 1.4l-.8 4.4A1.6 1.6 0 0 1 10.4 12H4"'
    + ' stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>'
    + '<span class="helpful-label"></span>';

  let count = review.helpful_count || 0;
  let voted = false;

  const paint = () => {
    button.querySelector('.helpful-label').textContent =
      'Helpful' + (count > 0 ? ' (' + count + ')' : '');
    button.setAttribute('aria-pressed', String(voted));
    button.style.color = voted ? 'var(--blue)' : '';
  };
  paint();

  button.addEventListener('click', async () => {
    if (!session.isSignedIn()) {
      toast.info('Sign in to vote on reviews');
      setTimeout(() => session.requireSignedIn(), 700);
      return;
    }

    button.disabled = true;
    const response = await Fetcher.post({ endpoint: '/v1/review/' + review.uuid + '/vote', body: {} });
    button.disabled = false;
    if (!response.ok) return;

    const nowVoted = !!response.data.helpful;
    if (nowVoted !== voted) count += nowVoted ? 1 : -1;
    voted = nowVoted;
    paint();
  });

  row.appendChild(button);
  return row;
}

function renderRatingSummary(total, breakdown) {
  /* Worked out from the breakdown rather than read off the product, so
     a review written on this page moves the number without a reload. */
  if (breakdown && total) {
    const sum = [1, 2, 3, 4, 5].reduce((acc, n) => acc + n * (breakdown[n] || breakdown[String(n)] || 0), 0);
    product.rating_average = sum / total;
  }
  const big = $('.rating-big');
  if (big) big.textContent = (product.rating_average || 0).toFixed(1);

  const summaryStars = $('.rating-summary .stars');
  if (summaryStars) summaryStars.innerHTML = stars(product.rating_average);

  const sub = $('.rating-sub');
  if (sub) sub.textContent = total === 1 ? '1 review' : total.toLocaleString() + ' reviews';

  /* The bars are drawn 5 down to 1, and their widths are shares of the
     largest bucket rather than of the total — a 60/40 split should
     look like 60/40, not like two stubs. */
  const rows = $$('.rbar-row');
  const counts = [5, 4, 3, 2, 1].map((n) => (breakdown ? breakdown[n] || breakdown[String(n)] || 0 : 0));
  const peak = Math.max(1, ...counts);

  rows.forEach((row, index) => {
    const count = counts[index] || 0;
    row.querySelector('.rbar-fill').style.width = Math.round((count / peak) * 100) + '%';
    row.querySelector('.rbar-count').textContent = String(count);
  });
}

async function loadReviews(append = false) {
  const host = $('.pd-review-list');
  if (!host) return;

  const response = await Fetcher.get({
    endpoint: '/v1/review/list',
    query: { product_id: product.uuid, sort: 'helpful', page: reviewPage },
    showError: false,
  });

  if (!response.ok) return;
  const data = response.data;

  if (!append) {
    host.innerHTML = '';
    renderRatingSummary(data.total, data.rating_breakdown);
  }

  const loadMore = $('.btn-ghost');

  if (!data.items.length && !append) {
    const empty = document.createElement('div');
    empty.className = 'review-card-web';
    empty.style.textAlign = 'center';
    empty.style.color = 'var(--subtle)';
    empty.textContent = 'No reviews yet — be the first to write one.';
    host.appendChild(empty);
  } else {
    data.items.forEach((review) => host.appendChild(reviewCard(review)));
  }

  const shown = data.page * data.limit;
  const remaining = Math.max(0, data.total - shown);
  if (loadMore) {
    loadMore.style.display = remaining > 0 ? '' : 'none';
    loadMore.textContent = 'Load More Reviews (' + remaining + ' remaining)';
    loadMore.onclick = () => { reviewPage++; loadReviews(true); };
  }

  const tab = $$('.pd-tab')[1];
  if (tab) tab.textContent = 'Reviews (' + data.total + ')';
}

/*
  Writing a review.

  Only customers who bought the product can review it. The button
  shows only when `GET /v1/review/eligible` finds a delivered order of
  the caller's with this product still unreviewed on it, and the
  review is filed against the newest such order.
*/
async function wireWriteReview() {
  const button = $('.section-head .btn-outline');
  if (!button) return;
  button.style.display = 'none';
  if (!session.isSignedIn()) return;

  const response = await Fetcher.get({
    endpoint: '/v1/review/eligible',
    query: { product_id: product.uuid },
    showError: false,
  });
  const orders = response.ok ? (response.data || []) : [];
  if (!orders.length) return;

  button.style.display = '';
  button.onclick = () => {
    openReviewComposer({
      orderId: orders[0].order_id,
      productId: product.uuid,
      productTitle: product.title,
      image: (product.gallery || [])[0] || product.cover_image_id,
      onDone: () => {
        reviewPage = 1;
        loadReviews();
        // That order is used up; re-ask in case another one is left.
        wireWriteReview();
      },
    });
  };
}

/* ── Related ──────────────────────────────────────────────────  */
function loadRelated() {
  const grid = $('.pd-related');
  if (!grid) return;
  grid.removeAttribute('aria-busy');

  // `image` is the whole ImageStruct (blur hash included); the bare id
  // is the fallback for a cover whose document couldn't be read.
  const items = (product.related || []).map((item) => ({ ...item, image: item.image || item.image_id, in_stock: true }));

  if (!items.length) {
    grid.closest('div').previousElementSibling?.remove?.();
    emptyState(grid, 'Nothing similar yet', 'Related products appear as the catalogue grows.');
    return;
  }

  renderGrid(grid, items);
}

/* Everything below the product itself. When there is no product to
   show, these would sit there as loading placeholders for good. */
function hideDetailSections() {
  ['.pd-tabs', '.pd-description', '.rating-summary', '.pd-review-list', '.pd-related']
    .forEach((sel) => { const el = $(sel); if (el) el.style.display = 'none'; });
  $$('.section-head').forEach((el) => { el.style.display = 'none'; });
  const loadMore = $('.btn-ghost');
  if (loadMore) loadMore.parentElement.style.display = 'none';
}

/* ── Tabs ─────────────────────────────────────────────────────
   Description and Reviews each have a section on the page, so the
   tabs move between them.                                       */
function wireTabs() {
  const tabs = $$('.pd-tab');
  const description = $('.body-lg') ? $('.body-lg').closest('div') : null;
  const reviews = $('.rating-summary');

  tabs.forEach((tab, index) => {
    tab.style.cursor = 'pointer';
    tab.onclick = () => {
      tabs.forEach((t) => t.classList.toggle('active', t === tab));
      const target = index === 0 ? description : reviews;
      if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
    };
  });

  const reviewLink = $('.pd-review-count');
  if (reviewLink) {
    reviewLink.style.cursor = 'pointer';
    reviewLink.onclick = () => tabs[1] && tabs[1].click();
  }
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const layout = $('.product-detail-layout');
  const slug = slugFromUrl();

  if (!slug) {
    hideDetailSections();
    if (layout) {
      emptyState(layout, 'No product selected', 'Pick something from the catalogue to see it here.');
    }
    return;
  }

  const response = await Fetcher.get({ endpoint: '/v1/product/' + encodeURIComponent(slug), showError: false });

  if (!response.ok) {
    hideDetailSections();
    if (layout) {
      emptyState(layout,
        response.status === 404 ? 'Product not found' : "Couldn't load this product",
        response.status === 404
          ? 'It may have been removed. Try browsing the catalogue.'
          : 'Please try again in a moment.');
    }
    document.title = 'Product not found — Bambee';
    return;
  }

  product = response.data;

  // The first in-stock variant, so the page opens on something that
  // can actually be bought.
  const variants = product.variants || [];
  selected = variants.find((v) => v.stock > 0) || variants[0] || null;

  renderHeader();
  renderGallery();
  jumpGalleryToVariant();
  renderOptions();
  renderPrice();
  renderQuantity();
  renderStock();
  wireViewer();
  wireQuantity();
  wireCta();
  wireTabs();
  wireWriteReview();
  loadRelated();
  loadReviews();
  syncWishState();

  // Fire-and-forget: a signed-out visitor has nowhere to record this,
  // and a failure here must never surface on the page.
  if (session.isSignedIn()) {
    Fetcher.post({
      endpoint: '/v1/recently-viewed',
      body: { product_id: product.uuid },
      showError: false,
    });
  }
})();
