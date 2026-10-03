/* ═══════════════════════════════════════════════════════════════
   Bambe Web — product.js

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

function slugFromUrl() {
  return new URLSearchParams(location.search).get('slug') || '';
}

/* ── Gallery ──────────────────────────────────────────────────  */
function renderGallery() {
  const thumbs = $('.pd-thumbnails');
  const main = $('.pd-main-img-bg');
  const dots = $('.pd-img-dots');
  if (!thumbs || !main) return;

  const images = product.gallery || [];
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
    thumb.addEventListener('click', () => showImage(index));
    thumb.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); showImage(index); }
    });
    thumbs.appendChild(thumb);

    if (dots) {
      const dot = document.createElement('div');
      dot.className = 'pd-dot' + (index === galleryIndex ? ' active' : '');
      dot.addEventListener('click', () => showImage(index));
      dots.appendChild(dot);
    }
  });

  showImage(galleryIndex);
}

function showImage(index) {
  const images = product.gallery || [];
  if (!images.length) return;

  galleryIndex = Math.max(0, Math.min(index, images.length - 1));
  paintImage($('.pd-main-img-bg'), images[galleryIndex].image || images[galleryIndex].image_id);

  $$('.pd-thumb').forEach((el, i) => el.classList.toggle('active', i === galleryIndex));
  $$('.pd-dot').forEach((el, i) => el.classList.toggle('active', i === galleryIndex));
}

/* Picking a colour should move the gallery to that colourway rather
   than leaving the previous photo up beside the new swatch. */
function jumpGalleryToVariant() {
  if (!selected) return;
  const index = (product.gallery || []).findIndex((g) => g.image_id === selected.image_id);
  if (index >= 0) showImage(index);
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

  const stockTag = $('.pd-stock-tag');
  if (stockTag) {
    const on = !!product.in_stock;
    stockTag.classList.toggle('tag-green', on);
    stockTag.classList.toggle('tag-grey', !on);
    stockTag.querySelector('.pd-stock-label').textContent = on ? 'In Stock' : 'Sold out';
    stockTag.style.display = '';
  }

  const title = $('.pd-title');
  if (title) title.textContent = product.title;
  document.title = product.title + ' — Bambe';

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

/* ── Variant pickers ──────────────────────────────────────────  */
function colours() {
  const seen = new Map();
  (product.variants || []).forEach((v) => {
    if (v.color && !seen.has(v.color)) seen.set(v.color, v);
  });
  return [...seen.values()];
}

function sizesForColour(colour) {
  const seen = new Map();
  (product.variants || [])
    .filter((v) => v.color === colour)
    .forEach((v) => { if (!seen.has(v.size)) seen.set(v.size, v); });
  return [...seen.values()];
}

function renderColours() {
  const row = $('.pd-swatches');
  const label = $('.pd-section-label');
  if (!row) return;

  const options = colours();
  row.innerHTML = '';
  row.style.display = options.length > 1 ? '' : 'none';

  options.forEach((variant) => {
    const swatch = document.createElement('div');
    const active = selected && variant.color === selected.color;
    swatch.className = 'pd-swatch' + (active ? ' active' : '');
    swatch.style.background = variant.color_hex || '#ccc';
    swatch.setAttribute('role', 'button');
    swatch.setAttribute('tabindex', '0');
    swatch.setAttribute('aria-label', variant.color);
    swatch.title = variant.color;

    if (active) {
      swatch.style.borderColor = 'var(--blue)';
      // A white swatch on a white card needs its own outline or the
      // selected state is the only thing that renders.
      swatch.style.boxShadow = 'inset 0 0 0 1px var(--border)';
    }

    swatch.addEventListener('click', () => pickColour(variant.color));
    swatch.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickColour(variant.color); }
    });
    row.appendChild(swatch);
  });

  if (label && selected) {
    label.innerHTML = 'Color — <strong style="color:var(--dark);font-weight:700;"></strong>';
    label.querySelector('strong').textContent = selected.color || '';
    label.style.display = options.length > 1 ? '' : 'none';
  }
}

function renderSizes() {
  const row = $('.pd-sizes');
  const label = $$('.pd-section-label')[1];
  if (!row || !selected) return;

  const options = sizesForColour(selected.color);
  row.innerHTML = '';
  row.style.display = options.length > 1 ? '' : 'none';

  options.forEach((variant) => {
    const cell = document.createElement('div');
    const out = variant.stock <= 0;
    cell.className = 'pd-size' + (out ? ' out' : '') + (variant.uuid === selected.uuid ? ' active' : '');
    cell.textContent = variant.size;
    cell.setAttribute('role', 'button');
    cell.setAttribute('tabindex', out ? '-1' : '0');
    cell.setAttribute('aria-disabled', String(out));
    if (out) cell.title = 'Out of stock';

    if (!out) {
      cell.addEventListener('click', () => pickVariant(variant));
      cell.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickVariant(variant); }
      });
    }
    row.appendChild(cell);
  });

  if (label) {
    label.innerHTML = 'Size — <strong style="color:var(--dark);"></strong>';
    label.querySelector('strong').textContent = selected.size || '';
    label.style.display = options.length > 1 ? '' : 'none';
  }
}

function pickColour(colour) {
  const options = sizesForColour(colour);
  // Keep the size if the new colour has it; otherwise take the first
  // one that is actually buyable, so the picker never lands on "out".
  const sameSize = options.find((v) => v.size === selected.size && v.stock > 0);
  pickVariant(sameSize || options.find((v) => v.stock > 0) || options[0]);
}

function pickVariant(variant) {
  if (!variant) return;
  selected = variant;
  quantity = Math.min(quantity, Math.max(1, variant.stock));
  renderColours();
  renderSizes();
  renderPrice();
  renderQuantity();
  jumpGalleryToVariant();
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
  }
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

  const buy = $('.btn-secondary');
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
  if (review.size) bought.push('Size ' + review.size);
  if (review.color) bought.push(review.color);
  if (review.created_at) bought.push(new Date(review.created_at).toLocaleDateString());
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
      + 'Response from Bambe</div>'
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

  `POST /v1/review` accepts one from any signed-in customer; naming an
  order is what earns the Verified Purchase badge, and the backend
  re-checks that the order is the caller's own and contains this
  product. So the page goes looking for a delivered order itself
  rather than sending the customer off to find one — and when there
  isn't one, the review is still accepted, just unbadged.
*/
async function findPurchase() {
  if (!session.isSignedIn()) return null;

  const response = await Fetcher.get({
    endpoint: '/v1/order/list',
    query: { status: 'delivered', limit: 50 },
    showError: false,
  });
  if (!response.ok) return null;

  const order = (response.data.items || [])
    .find((o) => (o.items || []).some((i) => i.product_id === product.uuid));
  return order ? order.uuid : null;
}

function wireWriteReview() {
  const button = $('.section-head .btn-outline');
  if (!button) return;

  button.onclick = async () => {
    if (!session.isSignedIn()) {
      toast.info('Sign in to write a review');
      setTimeout(() => session.requireSignedIn(), 700);
      return;
    }

    button.disabled = true;
    const orderId = await findPurchase();
    button.disabled = false;

    openReviewComposer({
      productId: product.uuid,
      productTitle: product.title,
      orderId,
      variantId: selected ? selected.uuid : null,
      image: (product.gallery || [])[0] || product.cover_image_id,
      onDone: () => { reviewPage = 1; loadReviews(); },
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

/* ── Delivery estimate ────────────────────────────────────────
   Drawn from the store's own delivery methods: the lowest
   free-delivery threshold, and a date window from the first
   method's min/max days. With no methods configured there is
   nothing true to say, so the row stays hidden.                */
async function loadDeliveryEstimate() {
  const row = $('.pd-delivery-eta');
  if (!row) return;

  const response = await Fetcher.get({ endpoint: '/v1/checkout/delivery-methods', showError: false });
  const methods = response.ok ? response.data || [] : [];
  if (!methods.length) return;

  const text = row.querySelector('div');
  text.textContent = '';
  const strong = (value) => {
    const el = document.createElement('strong');
    el.textContent = value;
    return el;
  };

  const thresholds = methods.map((m) => m.free_above).filter((v) => v != null);
  if (thresholds.length) {
    text.append(strong('Free delivery'), ' on orders over ' + money(Math.min(...thresholds)));
  }

  const first = methods[0];
  if (first.min_days != null && first.max_days != null) {
    const day = (offset) => {
      const d = new Date();
      d.setDate(d.getDate() + offset);
      return d;
    };
    const from = day(first.min_days);
    const to = day(first.max_days);
    const month = (d) => d.toLocaleDateString(undefined, { month: 'short' });
    const span = from.getMonth() === to.getMonth()
      ? month(from) + ' ' + from.getDate() + (from.getDate() === to.getDate() ? '' : '–' + to.getDate())
      : month(from) + ' ' + from.getDate() + ' – ' + month(to) + ' ' + to.getDate();
    text.append(thresholds.length ? ' · Estimated ' : 'Estimated delivery ', strong(span));
  }

  if (text.childNodes.length) row.style.display = '';
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
   tabs move between them. Size Guide and Shipping & Returns have no
   content behind them and no endpoint to fill them from, so they
   are left inert rather than given invented copy.               */
function wireTabs() {
  const tabs = $$('.pd-tab');
  const description = $('.body-lg') ? $('.body-lg').closest('div') : null;
  const reviews = $('.rating-summary');

  tabs.forEach((tab, index) => {
    tab.style.cursor = index < 2 ? 'pointer' : 'default';
    tab.onclick = () => {
      if (index > 1) return;
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
    document.title = 'Product not found — Bambe';
    return;
  }

  product = response.data;

  // The first in-stock variant, so the page opens on something that
  // can actually be bought.
  const variants = product.variants || [];
  selected = variants.find((v) => v.stock > 0) || variants[0] || null;

  renderHeader();
  renderGallery();
  renderColours();
  renderSizes();
  renderPrice();
  renderQuantity();
  wireQuantity();
  wireCta();
  wireTabs();
  wireWriteReview();
  loadRelated();
  loadReviews();
  loadDeliveryEstimate();
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
