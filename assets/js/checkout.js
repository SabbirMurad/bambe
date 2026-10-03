/* ═══════════════════════════════════════════════════════════════
   Bambe Web — checkout.js

   The three checkout pages are one transaction split across three
   navigations, and the server holds no draft — `place-order` takes
   every choice at once, at the end. So the choices made on the way
   have to be carried by the client: this module owns that carrier,
   and the summary panel all three pages show.

   sessionStorage, not localStorage: a checkout abandoned last week
   should not come back to life, and a second tab should be able to
   run its own without the two overwriting each other's address.

   The panel is painted from `GET /v1/checkout/quote`, never from
   arithmetic done here. Shipping depends on the delivery method,
   tax on the store's rate, and the discount on a coupon that may
   have expired since the cart page — the server settles all of it,
   and `place-order` prices the order with the same function, so the
   number on screen is the number that gets charged.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { session } from './session.js';
import { money, paintImage } from './catalog.js';

const KEY = 'bambe.checkout';

/* ── The draft ────────────────────────────────────────────────  */

export const draft = {
  read() {
    try {
      return JSON.parse(sessionStorage.getItem(KEY)) || {};
    } catch (e) {
      return {};
    }
  },

  write(patch) {
    const next = Object.assign(this.read(), patch);
    sessionStorage.setItem(KEY, JSON.stringify(next));
    return next;
  },

  clear() {
    sessionStorage.removeItem(KEY);
  },
};

/* ── The quote ────────────────────────────────────────────────  */

export async function fetchQuote() {
  const { delivery_method_id, packaging_option_id } = draft.read();
  const response = await Fetcher.get({
    endpoint: '/v1/checkout/quote',
    query: {
      delivery_method_id: delivery_method_id || '',
      packaging_option_id: packaging_option_id || '',
    },
  });
  return response.ok ? response.data : null;
}

/*
  Every checkout page needs the same two things to be true before it
  can render: somebody is signed in, and there is something to buy.
  Neither is worth discovering three screens in.
*/
export async function guard() {
  if (!session.isSignedIn()) {
    session.requireSignedIn();
    return null;
  }

  const quote = await fetchQuote();
  if (!quote || !quote.item_count) {
    location.href = '/cart';
    return null;
  }
  return quote;
}

/* ── The summary panel ────────────────────────────────────────  */

function row(label, value, className) {
  const el = document.createElement('div');
  el.className = 'summary-row' + (className ? ' ' + className : '');
  const left = document.createElement('span');
  left.textContent = label;
  const right = document.createElement('strong');
  right.textContent = value;
  el.append(left, right);
  return el;
}

function preview(line) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'display:flex;gap:10px;align-items:center;';

  const thumb = document.createElement('div');
  thumb.style.cssText = 'width:48px;height:48px;border-radius:var(--radius-sm);background:var(--input-bg);flex-shrink:0;';
  paintImage(thumb, line.image_id);

  const middle = document.createElement('div');
  middle.style.cssText = 'flex:1;min-width:0;';
  const title = document.createElement('div');
  title.style.cssText = 'font-size:12px;font-weight:600;color:var(--dark);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
  title.textContent = line.title || 'Unavailable';
  const meta = document.createElement('div');
  meta.style.cssText = 'font-size:11px;color:var(--mid);';
  meta.textContent = [line.color, line.size].filter(Boolean).join(' · ')
    + (line.size || line.color ? ' · ' : '') + '×' + line.quantity;
  middle.append(title, meta);

  const price = document.createElement('div');
  price.style.cssText = 'font-size:13px;font-weight:700;';
  price.textContent = money(line.line_total);

  wrap.append(thumb, middle, price);
  return wrap;
}

/*
  One panel, three pages. `showItems` is off on the review step,
  which lists every line in full above it and doesn't need them
  twice.
*/
export function paintSummary(quote, options = {}) {
  const body = document.querySelector('.order-summary-body');
  if (!body || !quote) return;
  body.innerHTML = '';

  /* The review step lists the lines in full on the left, so its panel
     names them as priced rows instead of repeating the thumbnails. */
  if (options.itemsAsRows) {
    (quote.items || []).forEach((line) => {
      body.appendChild(row(line.title + ' ×' + line.quantity, money(line.line_total)));
    });
    const divider = document.createElement('div');
    divider.className = 'divider';
    divider.style.margin = '10px 0';
    body.appendChild(divider);
  }

  if (options.showItems !== false && !options.itemsAsRows) {
    const list = document.createElement('div');
    list.style.cssText = 'margin-bottom:16px;display:flex;flex-direction:column;gap:10px;';
    (quote.items || []).forEach((line) => list.appendChild(preview(line)));
    body.appendChild(list);

    const divider = document.createElement('div');
    divider.className = 'divider';
    divider.style.margin = '0 0 14px';
    body.appendChild(divider);
  }

  const units = (quote.items || []).reduce((sum, line) => sum + line.quantity, 0);
  body.appendChild(row('Subtotal (' + units + (units === 1 ? ' item)' : ' items)'), money(quote.subtotal)));

  if (quote.coupon_code && !quote.coupon_error) {
    body.appendChild(quote.discount > 0
      ? row('Coupon (' + quote.coupon_code + ')', '–' + money(quote.discount), 'saving')
      : row('Coupon (' + quote.coupon_code + ')', 'Free shipping', 'saving'));
  }

  /* A coupon that went stale between the cart and here is dropped by
     `place_order` rather than blocking the sale — so it is said
     plainly now, not discovered on the receipt. */
  if (quote.coupon_error) {
    const warning = row('Coupon (' + (quote.coupon_code || '') + ')', 'Not applied');
    warning.style.color = 'var(--error)';
    body.appendChild(warning);
  }

  const shipping = quote.delivery_method
    ? (quote.shipping ? money(quote.shipping) : 'Free')
    : 'Pick a method';
  const shippingRow = row('Shipping', shipping);
  if (quote.delivery_method && !quote.shipping) {
    shippingRow.querySelector('strong').style.color = 'var(--success)';
  }
  body.appendChild(shippingRow);

  if (quote.packaging_option && quote.packaging) {
    body.appendChild(row('Packaging (' + quote.packaging_option.name + ')', money(quote.packaging)));
  }

  // The rate comes from the store settings, not from a constant here —
  // the wireframe's "Tax (8%)" was hardcoded in two places.
  const taxLabel = quote.tax_rate_bps
    ? 'Tax (' + (quote.tax_rate_bps / 100).toFixed(quote.tax_rate_bps % 100 ? 2 : 0) + '%)'
    : 'Tax';
  body.appendChild(row(taxLabel, money(quote.tax)));

  const total = row('Total', money(quote.grand_total), 'total');
  total.querySelector('strong').style.color = 'var(--blue)';
  body.appendChild(total);
}

/* ── Shared chrome ────────────────────────────────────────────  */

/*
  Divs standing in for radios and buttons still have to answer to a
  keyboard. Every checkout page draws its choices that way, so the
  handler lives here rather than three times over.
*/
export function wireKeyboardActivation(selector) {
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    const el = document.activeElement;
    if (el && el.matches && el.matches(selector)) {
      e.preventDefault();
      el.click();
    }
  });
}

/* Single-select within a group of cards. */
export function selectIn(group, chosen) {
  group.forEach((el) => {
    const on = el === chosen;
    el.classList.toggle('selected', on);
    el.setAttribute('aria-checked', on ? 'true' : 'false');
  });
}

export { money };
