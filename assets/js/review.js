/* ═══════════════════════════════════════════════════════════════
   Bambee Web — review.js

   The last screen before the money moves, and the only one where
   being wrong actually costs something. So nothing here is carried
   over from the previous steps as a *number*: the draft holds ids,
   and every figure on this page is re-fetched from
   `GET /v1/checkout/quote` on arrival. If a price moved or a coupon
   expired while the shopper was picking a delivery method, this is
   where they find out — not on the receipt.

   Placing the order sends the whole draft at once, because that is
   the shape `place-order` takes; there is no server-side draft to
   have been building up. On success the draft is cleared, so a back
   button or a second tab cannot replay it.

   **The order is not paid when this returns.** Cash on delivery owes
   nothing yet; every other method comes back with a `payment_url` and
   the browser goes to SSLCommerz. Only their validated callback marks
   the order paid, so there is no path from this file to a paid order.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { draft, fetchQuote, guard, paintSummary, money } from './checkout.js';
import { paintImage } from './catalog.js';

const $ = (sel) => document.querySelector(sel);

const DAY_MS = 24 * 60 * 60 * 1000;

let addresses = [];
let quote = null;

/* ── Items ────────────────────────────────────────────────────  */

function reviewItem(line) {
  const row = document.createElement('div');
  row.className = 'review-item';

  const thumb = document.createElement('div');
  thumb.style.cssText = 'width:64px;height:64px;border-radius:var(--radius-sm);background:var(--input-bg);flex-shrink:0;';
  paintImage(thumb, line.image_id);

  const middle = document.createElement('div');
  middle.style.cssText = 'flex:1;min-width:0;';
  const title = document.createElement('div');
  title.style.cssText = 'font-weight:700;font-size:14px;color:var(--dark);';
  title.textContent = line.title || 'Unavailable';
  const meta = document.createElement('div');
  meta.style.cssText = 'font-size:12px;color:var(--mid);margin-top:3px;';
  meta.textContent = [
    line.brand_name && 'Brand: ' + line.brand_name,
    line.size && 'Size: ' + line.size,
    line.color && 'Color: ' + line.color,
    'Qty: ' + line.quantity,
  ].filter(Boolean).join(' · ');
  middle.append(title, meta);

  const right = document.createElement('div');
  right.style.textAlign = 'right';
  const total = document.createElement('div');
  total.style.cssText = 'font-weight:700;font-size:15px;';
  total.textContent = money(line.line_total);
  right.appendChild(total);
  // Only worth saying when there is more than one of them.
  if (line.quantity > 1) {
    const each = document.createElement('div');
    each.style.cssText = 'font-size:12px;color:var(--mid);';
    each.textContent = money(line.unit_price) + ' each';
    right.appendChild(each);
  }

  row.append(thumb, middle, right);
  return row;
}

function renderItems() {
  const host = $('.review-items-list');
  host.innerHTML = '';
  quote.items.forEach((line) => host.appendChild(reviewItem(line)));

  const heading = $('.ck-section-title');
  if (heading) {
    // The count sits as a bare text node between the tick and the Edit
    // link, so it is replaced in place rather than by rewriting the row.
    heading.childNodes.forEach((node) => {
      if (node.nodeType === 3 && node.data.trim().startsWith('Items')) {
        node.data = ' Items (' + quote.items.length + ') ';
      }
    });
  }
}

/* ── Delivery ─────────────────────────────────────────────────  */

function estimateText(method) {
  if (!method) return '';
  const now = Date.now();
  const from = new Date(now + method.min_days * DAY_MS);
  const to = new Date(now + method.max_days * DAY_MS);

  /* Both ends written out, the year only once. Abbreviating the second
     date to a bare number reads as "Aug 13–14" in the US and as
     "13 Aug–14" everywhere else, which is not a date at all. */
  const day = { month: 'short', day: 'numeric' };
  const full = { month: 'short', day: 'numeric', year: 'numeric' };
  return 'Est. ' + (method.min_days === method.max_days
    ? to.toLocaleDateString(undefined, full)
    : from.toLocaleDateString(undefined, day) + ' – ' + to.toLocaleDateString(undefined, full));
}

function renderDelivery() {
  const section = document.querySelectorAll('.ck-summary-section')[0];
  if (!section) return;

  const address = addresses.find((a) => a.uuid === draft.read().address_id);
  const columns = section.querySelectorAll(':scope > div > div');
  const shipTo = columns[0];
  const delivery = columns[1];

  if (shipTo && address) {
    shipTo.children[1].textContent = address.recipient_name;
    shipTo.children[2].innerHTML = '';
    [
      address.street,
      [address.city, [address.state, address.zip].filter(Boolean).join(' ')].filter(Boolean).join(', ')
        + (address.country ? ', ' + address.country : ''),
      address.phone,
    ].filter(Boolean).forEach((line, index) => {
      if (index) shipTo.children[2].appendChild(document.createElement('br'));
      shipTo.children[2].appendChild(document.createTextNode(line));
    });
  }

  const method = quote.delivery_method;
  if (delivery && method) {
    delivery.children[1].textContent = method.name;
    delivery.children[2].textContent = quote.shipping ? money(quote.shipping) : 'Free';
    delivery.children[2].style.color = quote.shipping ? 'var(--mid)' : 'var(--success)';
    delivery.children[3].textContent = estimateText(method);
  }
}

/* ── Payment ──────────────────────────────────────────────────  */

const KIND_LABEL = {
  Card: 'Card',
  PayPal: 'PayPal',
  ApplePay: 'Apple Pay',
  GooglePay: 'Google Pay',
  CashOnDelivery: 'Cash on Delivery',
};

async function renderPayment() {
  const section = document.querySelectorAll('.ck-summary-section')[1];
  if (!section) return;

  const saved = draft.read();
  const lines = section.querySelectorAll(':scope > div > div:last-child > div');
  const title = lines[0];
  const detail = lines[1];

  let label = KIND_LABEL[saved.payment_method_kind] || saved.payment_method_kind || 'Not chosen';
  let note = '';

  if (saved.payment_method_id) {
    const response = await Fetcher.get({ endpoint: '/v1/payment-method/list', showError: false });
    const method = (response.data || []).find((m) => m.uuid === saved.payment_method_id);
    if (method) {
      label = method.card_brand && method.last4
        ? method.card_brand + ' ending ' + method.last4
        : (method.account_email || label);
      if (method.exp_month && method.exp_year) {
        note = 'Expires ' + String(method.exp_month).padStart(2, '0') + '/' + String(method.exp_year).slice(-2);
      }
    }
  } else if (saved.payment_method_kind === 'CashOnDelivery') {
    note = 'Pay the courier on arrival';
  }

  const billing = saved.billing_address_id && saved.billing_address_id !== saved.address_id
    ? 'Billing: ' + (addresses.find((a) => a.uuid === saved.billing_address_id) || {}).label
    : 'Billing: Same as delivery';

  if (title) title.textContent = label;
  if (detail) detail.textContent = [note, billing].filter(Boolean).join(' · ');

  /* The wireframe put a Visa mark here and left it there whatever was
     picked. A card keeps it; anything else gets a neutral glyph rather
     than a brand it has nothing to do with. */
  if (!saved.payment_method_id) {
    const badge = section.querySelector(':scope > div > div:first-child');
    if (badge) {
      badge.innerHTML = '<svg width="20" height="20" viewBox="0 0 20 20" fill="none">'
        + '<rect x="2" y="5" width="16" height="11" rx="2" stroke="#94908A" stroke-width="1.3"/>'
        + '<path d="M2 9h16" stroke="#94908A" stroke-width="1.3"/></svg>';
    }
  }
}

/* ── Totals panel ─────────────────────────────────────────────  */

function renderTotals() {
  paintSummary(quote, { itemsAsRows: true });

  const foot = $('.order-summary-foot');
  if (!foot) return;
  const savings = foot.firstElementChild;
  if (!savings) return;

  const saved = quote.discount;
  savings.style.display = saved > 0 ? '' : 'none';
  const text = savings.querySelector('span');
  if (text && saved > 0) text.textContent = 'You save ' + money(saved) + ' on this order!';
}

/* ── Placing the order ────────────────────────────────────────  */

function placeButton() {
  return document.querySelector('.checkout-btn');
}

function paintPlaceButton() {
  const button = placeButton();
  if (!button) return;
  const label = button.querySelector('.place-label');
  if (label) label.textContent = 'Place Order · ' + money(quote.grand_total);
}

async function placeOrder(e) {
  e.preventDefault();

  const saved = draft.read();
  const button = placeButton();
  const label = button.querySelector('.place-label');

  button.classList.add('disabled');
  if (label) label.textContent = 'Placing your order…';

  const response = await Fetcher.post({
    endpoint: '/v1/checkout/place-order',
    body: {
      address_id: saved.address_id,
      billing_address_id: saved.billing_address_id || null,
      delivery_method_id: saved.delivery_method_id,
      packaging_option_id: saved.packaging_option_id || null,
      gift_message: saved.gift_message || null,
      payment_method_kind: saved.payment_method_kind,
      payment_method_id: saved.payment_method_id || null,
      customer_note: saved.customer_note || null,
    },
    showError: false,
  });

  if (!response.ok) {
    button.classList.remove('disabled');
    paintPlaceButton();
    /* Stock ran out, an item was pulled, the address vanished — all of
       these come back as a plain message, and all of them mean going
       back rather than trying again. */
    toast.error(response.error || "That order couldn't be placed");
    return;
  }

  // Cleared only once the order exists: a failed attempt must leave the
  // draft intact so the shopper can fix one thing and retry.
  draft.clear();

  const { order, payment_url: paymentUrl, requires_payment: requiresPayment } = response.data;

  /* Cash on delivery is done — the order stands and nothing is owed yet.
     Everything else is unpaid until SSLCommerz says otherwise, so the
     browser goes to the gateway rather than to a receipt. */
  if (requiresPayment && paymentUrl) {
    if (label) label.textContent = 'Taking you to payment…';
    location.href = paymentUrl;
    return;
  }

  if (requiresPayment) {
    /* The order exists but the gateway couldn't be reached. Saying so
       beats a confirmation page for an order nobody has paid for; the
       order sits Pending and can be paid or cancelled from the account. */
    toast.error('Your order was placed but payment couldn’t be started. You can pay it from your orders.');
    location.href = '/account/orders';
    return;
  }

  location.href = '/checkout/confirmation?id=' + encodeURIComponent(order.uuid);
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  quote = await guard();
  if (!quote) return;

  const saved = draft.read();
  if (!saved.address_id || !saved.delivery_method_id) {
    location.href = '/checkout/address';
    return;
  }
  if (!saved.payment_method_kind) {
    location.href = '/checkout/payment';
    return;
  }

  const response = await Fetcher.get({ endpoint: '/v1/address/list' });
  addresses = response.ok ? (response.data || []) : [];

  renderItems();
  renderDelivery();
  await renderPayment();
  renderTotals();
  paintPlaceButton();

  const link = document.querySelector('a[href="/checkout/confirmation"]');
  if (link) link.addEventListener('click', placeOrder);

  /* A line that went out of stock while the shopper was choosing a
     delivery method will be refused by place-order — better to say so
     here, where the Edit links are. */
  const blocked = quote.items.some((line) => !line.available || !line.in_stock);
  if (blocked) {
    const button = placeButton();
    if (button) button.classList.add('disabled');
    toast.error('Something in your cart is no longer available — check the items above');
  }
})();

/* Re-quote when the tab is returned to: a price or a coupon can move
   while this page sits open, and the total on the button is a promise. */
document.addEventListener('visibilitychange', async () => {
  if (document.hidden || !quote) return;
  const fresh = await fetchQuote();
  if (!fresh) return;
  const changed = fresh.grand_total !== quote.grand_total;
  quote = fresh;
  renderTotals();
  paintPlaceButton();
  if (changed) toast.info('Your total changed while this page was open');
});
