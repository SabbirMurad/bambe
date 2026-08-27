/* ═══════════════════════════════════════════════════════════════
   Bambe Web — cart.js

   The cart is the one page where the server's arithmetic is the only
   arithmetic. The wireframe worked its own totals out in the browser
   — a hardcoded 10% coupon and 8% tax — and that is exactly the sum
   that must not be guessed at: `GET /v1/cart` returns `subtotal`,
   `discount` and `total`, and every mutation returns the whole cart
   again, so the panel is repainted from the response rather than
   patched up locally.

   Three things the wireframe drew nothing for, and the API answers
   on every line: `available` (the product or variant is gone),
   `in_stock` (there isn't enough left for the quantity asked) and
   `price_changed` (it costs something else now than when it was
   added). All three are things a shopper has to be told before they
   reach the payment step, so each one is called out on its line and
   the ones that block checkout also disable the button.

   Shipping and tax are deliberately not shown. They come from the
   delivery method and the store's tax rate, neither of which is
   chosen yet at this point — an "estimated total" that quietly
   excludes them is honest; a "Total" that pretends to be final is
   not.

   Per-item selection is gone. `POST /v1/checkout/place-order` takes
   the cart, whole — there is no way to check out three of five
   lines, so tick boxes that imply otherwise would be a promise the
   backend cannot keep. Removing a line is the way to leave it behind.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { money, imageUrl, emptyState, hideSkeleton } from './catalog.js';

const $ = (sel) => document.querySelector(sel);

/* The last cart the server sent. Never edited in place except by the
   quantity stepper, which owns its own line until its PATCH lands. */
let cart = null;

/* Per-line debounce timers, so holding + on a stepper sends one
   request rather than one per click. */
const pending = new Map();
const QUIET_MS = 350;

/* ── Line rendering ───────────────────────────────────────────  */

function itemsHost() {
  return $('.cart-items');
}

/* "TrendCo · Size: M · Color: White", minus whatever is missing —
   a one-variant product has neither size nor colour, and the meta
   line should not read "Size: · Color:". */
function metaLine(line) {
  const bits = [];
  if (line.brand_name) bits.push(line.brand_name);
  if (line.size) bits.push('Size: ' + line.size);
  if (line.color) bits.push('Color: ' + line.color);
  return bits.join('  ·  ');
}

/*
  The warnings that live under the name. Order matters: an item that
  is gone entirely makes its stock and its price irrelevant, so it is
  the only thing said about that line.
*/
function noteFor(line) {
  if (!line.available) {
    return { text: 'No longer available — remove it to continue', tone: 'error' };
  }
  if (!line.in_stock) {
    const left = line.stock || 0;
    return {
      text: left > 0
        ? 'Only ' + left + ' left — reduce the quantity to continue'
        : 'Out of stock — remove it to continue',
      tone: 'error',
    };
  }
  if (line.price_changed) {
    const up = line.unit_price > line.price_when_added;
    return {
      text: (up ? 'Price went up from ' : 'Price dropped from ')
        + money(line.price_when_added) + ' since you added it',
      tone: up ? 'warn' : 'good',
    };
  }
  return null;
}

const NOTE_COLOUR = { error: 'var(--error)', warn: 'var(--warning)', good: 'var(--success)' };

function lineRow(line) {
  const row = document.createElement('div');
  row.className = 'cart-item';
  row.dataset.itemId = line.uuid;
  if (!line.available) row.classList.add('unselected');

  const href = line.slug ? '/product?slug=' + encodeURIComponent(line.slug) : null;

  row.innerHTML =
    '<div class="cart-item-info">'
      + '<div class="cart-item-img"></div>'
      + '<div style="min-width:0;">'
        + '<div class="cart-item-name"></div>'
        + '<div class="cart-item-meta"></div>'
        + '<div class="cart-item-note" style="font-size:12px;font-weight:600;margin:-4px 0 10px;"></div>'
        + '<div class="cart-item-actions">'
          + '<div class="qty-ctrl" style="height:32px;">'
            + '<div class="qty-btn" style="width:30px;height:30px;font-size:16px;">−</div>'
            + '<div class="qty-val" style="width:36px;height:30px;font-size:13px;"></div>'
            + '<div class="qty-btn" style="width:30px;height:30px;font-size:16px;">+</div>'
          + '</div>'
          + '<div class="cart-item-action wishlist" role="button" tabindex="0">'
            + '<svg width="13" height="13" viewBox="0 0 14 14" fill="none"><path d="M7 12.2S1.4 8.4 1.4 4.9a2.9 2.9 0 0 1 5.6-1 2.9 2.9 0 0 1 5.6 1c0 3.5-5.6 7.3-5.6 7.3Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>'
            + 'Move to wishlist'
          + '</div>'
        + '</div>'
      + '</div>'
    + '</div>'
    + '<div class="cart-item-price-col"><span class="cart-unit-price"></span></div>'
    + '<div class="cart-total-price"></div>'
    + '<div class="cart-remove-col">'
      + '<button class="cart-remove-btn" type="button" aria-label="Remove from cart">'
        + '<svg width="13" height="13" viewBox="0 0 13 13" fill="none"><path d="M2 2l9 9M11 2 2 11" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>'
      + '</button>'
    + '</div>';

  const img = row.querySelector('.cart-item-img');
  const url = imageUrl(line.image_id);
  if (url) img.style.background = "url('" + url + "') center / cover no-repeat";

  const name = row.querySelector('.cart-item-name');
  name.textContent = line.title || 'This item is no longer in the catalogue';
  row.querySelector('.cart-item-meta').textContent = metaLine(line);

  if (href) {
    [img, name].forEach((el) => {
      el.style.cursor = 'pointer';
      el.addEventListener('click', () => { location.href = href; });
    });
  }

  const note = row.querySelector('.cart-item-note');
  const warning = noteFor(line);
  if (warning) {
    note.textContent = warning.text;
    note.style.color = NOTE_COLOUR[warning.tone];
  } else {
    note.style.display = 'none';
  }

  row.querySelector('.cart-unit-price').textContent = line.unit_price === null ? '—' : money(line.unit_price);
  row.querySelector('.cart-total-price').textContent = line.line_total === null ? '—' : money(line.line_total);

  wireQuantity(row, line);
  row.querySelector('.cart-remove-btn').addEventListener('click', () => removeLine(line, row));

  const wish = row.querySelector('.cart-item-action.wishlist');
  wish.addEventListener('click', () => moveToWishlist(line, row));
  wish.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); wish.click(); }
  });

  return row;
}

/* ── Quantity ─────────────────────────────────────────────────
   The stepper repaints its own line at once and tells the server a
   moment later, so holding + doesn't fire a request per click. The
   ceiling is the variant's own stock, not a flat 99 — offering 40 of
   something with 5 left is a promise the checkout will refuse.     */
function wireQuantity(row, line) {
  const ctrl = row.querySelector('.qty-ctrl');
  const [minus, plus] = ctrl.querySelectorAll('.qty-btn');
  const val = ctrl.querySelector('.qty-val');
  const ceiling = line.available ? Math.max(1, Math.min(99, line.stock || 99)) : 1;

  if (!line.available) {
    ctrl.style.opacity = '0.45';
    ctrl.style.pointerEvents = 'none';
  }

  const paint = () => {
    val.textContent = line.quantity;
    minus.classList.toggle('disabled', line.quantity <= 1);
    plus.classList.toggle('disabled', line.quantity >= ceiling);
    if (line.unit_price !== null) {
      row.querySelector('.cart-total-price').textContent = money(line.unit_price * line.quantity);
    }
  };

  const step = (delta) => {
    const next = Math.min(ceiling, Math.max(1, line.quantity + delta));
    if (next === line.quantity) {
      if (delta > 0 && line.stock) toast.info('Only ' + line.stock + ' left in stock');
      return;
    }
    line.quantity = next;
    paint();
    paintSummaryFromLines();

    clearTimeout(pending.get(line.uuid));
    pending.set(line.uuid, setTimeout(() => {
      pending.delete(line.uuid);
      commitQuantity(line);
    }, QUIET_MS));
  };

  minus.addEventListener('click', () => step(-1));
  plus.addEventListener('click', () => step(1));
  paint();
}

function flagPrint(cart) {
  return (cart.items || []).map((line) => line.uuid + ':' + line.available + ':' + line.in_stock).join('|');
}

async function commitQuantity(line) {
  const before = flagPrint(cart);

  const response = await Fetcher.patch({
    endpoint: '/v1/cart/item/' + line.uuid,
    body: { quantity: line.quantity },
  });
  if (!response.ok) {
    // The optimistic number is now a guess — take the server's word.
    return reload();
  }

  cart = response.data;
  /* Dropping to what's left in stock clears that line's warning, and
     the warning is drawn on the row rather than in the panel — so the
     rows are rebuilt when a flag moves, and left alone when the change
     was only arithmetic. */
  if (flagPrint(cart) !== before) render();
  paintSummary();
  paintCount();
}

/* ── Row actions ──────────────────────────────────────────────  */

async function removeLine(line, row) {
  const response = await Fetcher.delete({ endpoint: '/v1/cart/item/' + line.uuid });
  if (!response.ok) return;

  cart = response.data;
  row.remove();
  paintSummary();
  paintCount();
  if (!cart.items.length) render();
  toast.success('Removed from cart');
}

/*
  Saved against the product, not the variant: the wishlist heart on
  every other surface is per product, and a saved row that only
  matches one colourway would show as unsaved the moment the shopper
  opens the product page.
*/
async function moveToWishlist(line, row) {
  if (!session.isSignedIn()) {
    toast.info('Sign in to save items');
    setTimeout(() => session.requireSignedIn(), 700);
    return;
  }

  const saved = await Fetcher.post({
    endpoint: '/v1/wishlist',
    body: { product_id: line.product_id },
  });
  if (!saved.ok) return;

  const response = await Fetcher.delete({ endpoint: '/v1/cart/item/' + line.uuid, showError: false });
  if (response.ok) {
    cart = response.data;
    row.remove();
    paintSummary();
    paintCount();
    if (!cart.items.length) render();
  }
  toast.success('Moved to your wishlist');
}

/* ── Coupon ───────────────────────────────────────────────────
   A code that has gone stale comes back as `coupon_error` on the
   cart body, not as a failed request — the cart still loaded, it
   just isn't worth what it was. So it is shown inline where the
   discount would have been, rather than as a toast that disappears. */
function paintCoupon() {
  const input = $('.coupon-input');
  const applied = $('.coupon-applied');
  const button = $('.coupon-row .btn');
  if (!input || !applied || !button) return;

  const code = cart && cart.coupon_code;
  const error = cart && cart.coupon_error;

  input.value = code || '';
  input.readOnly = !!code && !error;

  if (error) {
    applied.style.display = '';
    applied.style.color = 'var(--error)';
    applied.textContent = error;
  } else if (code) {
    applied.style.display = '';
    applied.style.color = '';
    applied.textContent = cart.discount > 0
      ? 'Applied: –' + money(cart.discount)
      : 'Applied';
  } else {
    applied.style.display = 'none';
  }

  button.textContent = code ? 'Remove' : 'Apply';
  button.onclick = code ? removeCoupon : applyCoupon;
}

async function applyCoupon() {
  const input = $('.coupon-input');
  const code = (input.value || '').trim();
  if (!code) {
    toast.info('Enter a coupon code first');
    return;
  }

  // The failure is about the code the shopper just typed, so it is
  // shown against the field rather than in the corner of the screen.
  const response = await Fetcher.post({
    endpoint: '/v1/cart/coupon',
    body: { code },
    showError: false,
  });

  if (!response.ok) {
    const applied = $('.coupon-applied');
    applied.style.display = '';
    applied.style.color = 'var(--error)';
    applied.textContent = response.error || "That code can't be used";
    return;
  }

  cart = response.data;
  paintCoupon();
  paintSummary();
  toast.success('Coupon applied');
}

async function removeCoupon() {
  const response = await Fetcher.delete({ endpoint: '/v1/cart/coupon' });
  if (!response.ok) return;
  cart = response.data;
  paintCoupon();
  paintSummary();
  toast.success('Coupon removed');
}

/* ── Summary ──────────────────────────────────────────────────  */

function summaryRow(label, value, className) {
  const row = document.createElement('div');
  row.className = 'summary-row' + (className ? ' ' + className : '');
  const left = document.createElement('span');
  left.textContent = label;
  const right = document.createElement('strong');
  right.textContent = value;
  row.append(left, right);
  return row;
}

/* Units, not lines: "Subtotal (4 items)" over three lines where one
   of them is a pair is what the wireframe drew, and it is the count
   that matches the money beside it. */
function unitCount() {
  return (cart.items || []).reduce((sum, line) => sum + (line.available ? line.quantity : 0), 0);
}

function paintSummary() {
  const body = $('.order-summary-body');
  if (!body || !cart) return;

  const units = unitCount();
  body.innerHTML = '';
  body.appendChild(summaryRow('Subtotal (' + units + (units === 1 ? ' item)' : ' items)'), money(cart.subtotal)));

  if (cart.coupon_code && !cart.coupon_error) {
    body.appendChild(cart.discount > 0
      ? summaryRow('Coupon (' + cart.coupon_code + ')', '–' + money(cart.discount), 'saving')
      : summaryRow('Coupon (' + cart.coupon_code + ')', 'Free shipping', 'saving'));
  }

  /* Neither is knowable here: shipping depends on the delivery method
     and tax on the store's rate against the delivery address, and
     both are picked two steps later. */
  body.appendChild(summaryRow('Shipping', 'At checkout'));
  body.appendChild(summaryRow('Tax', 'At checkout'));
  body.appendChild(summaryRow('Estimated total', money(cart.total), 'total'));

  const note = document.createElement('div');
  note.style.cssText = 'font-size:11.5px;color:var(--subtle);padding-top:10px;line-height:1.5;';
  note.textContent = 'Shipping and tax are added at checkout, once you pick a delivery method.';
  body.appendChild(note);

  paintCheckoutButton();
}

/* The optimistic half of the stepper: the same sum the server will
   send back, computed locally so the panel moves with the click
   instead of a third of a second later. */
function paintSummaryFromLines() {
  if (!cart) return;
  cart.subtotal = (cart.items || []).reduce(
    (sum, line) => sum + (line.available ? line.unit_price * line.quantity : 0), 0);
  cart.total = cart.subtotal - (cart.discount || 0);
  paintSummary();
}

function paintCheckoutButton() {
  const button = $('.checkout-btn');
  const hint = $('#checkoutHint');
  const link = $('#checkoutLink');
  if (!button || !link) return;

  const empty = !cart || !cart.items.length;
  const blocked = !empty && cart.items.some((line) => !line.available || !line.in_stock);

  let reason = null;
  if (empty) reason = 'Your cart is empty';
  else if (blocked) reason = 'Sort the flagged items out to continue';

  button.classList.toggle('disabled', !!reason);
  if (hint) {
    hint.style.display = reason ? 'block' : 'none';
    hint.textContent = reason || '';
  }

  /* Placing an order needs an account — `place_order` reads the cart
     off the user, so a guest cart has to be merged into one first.
     Say so on the button rather than letting the next page bounce
     them to a login they weren't expecting. */
  const label = button.querySelector('.checkout-label');
  if (!session.isSignedIn()) {
    if (label) label.textContent = 'Sign In to Checkout';
    link.href = '/login?next=' + encodeURIComponent('/checkout/address');
  } else {
    if (label) label.textContent = 'Proceed to Checkout';
    link.href = '/checkout/address';
  }

  link.onclick = (e) => { if (reason) e.preventDefault(); };
}

function paintCount() {
  const count = $('.cart-panel-count');
  if (count && cart) {
    const n = cart.items.length;
    count.textContent = n === 1 ? '1 item' : n + ' items';
  }

  const badge = document.querySelectorAll('.nav-badge')[1];
  if (badge && cart) {
    badge.textContent = cart.item_count;
    badge.style.display = cart.item_count ? '' : 'none';
  }
}

/* ── Render ───────────────────────────────────────────────────  */

function render() {
  const host = itemsHost();
  if (!host) return;
  hideSkeleton(host);
  host.innerHTML = '';

  if (!cart || !cart.items.length) {
    emptyState(host, 'Your cart is empty', 'Anything you add will show up here.');
    const link = document.createElement('a');
    link.href = '/listing';
    link.className = 'btn btn-primary';
    link.textContent = 'Browse products';
    link.style.cssText = 'display:block;width:max-content;margin:0 auto 40px;text-decoration:none;';
    host.appendChild(link);

    const coupon = $('.coupon-row');
    if (coupon) coupon.style.display = 'none';
    return;
  }

  const frag = document.createDocumentFragment();
  cart.items.forEach((line) => frag.appendChild(lineRow(line)));
  host.appendChild(frag);
}

async function reload() {
  const response = await Fetcher.get({ endpoint: '/v1/cart' });
  const host = itemsHost();
  if (host) hideSkeleton(host);

  if (!response.ok) {
    if (host) emptyState(host, "Couldn't load your cart", 'Please try again in a moment.');
    return;
  }

  cart = response.data;
  render();
  paintCoupon();
  paintSummary();
  paintCount();
}

/* ── Boot ─────────────────────────────────────────────────────  */
(function init() {
  /* The wishlist badge in the nav is a wireframe constant on every
     page. This page knows the real cart count, so at least that one
     stops lying; the other is hidden rather than left at "5". */
  const wishBadge = document.querySelectorAll('.nav-badge')[0];
  if (wishBadge) wishBadge.style.display = 'none';

  reload();
})();
