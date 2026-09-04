/* ═══════════════════════════════════════════════════════════════
   Bambe Web — payment.js

   Step two: how it gets paid for, and where the invoice goes.

   The wireframe drew a card form — number, expiry, CVV — with a
   Visa number typed into it. That form is not wired up here, and
   deliberately so. `POST /v1/payment-method` takes a
   `provider_token`: an opaque handle a processor's own client SDK
   mints in the browser, and the endpoint stores nothing else.
   SSLCommerz's hosted page mints no such token — it takes the number
   on its own domain and hands back a one-off transaction — so there
   is still nothing to save, and a form that posted a real card number
   to this backend would be collecting card data the system is built
   specifically not to hold. The page says that plainly instead.

   What is offered is what the server can actually complete: the
   SSLCommerz hosted page — cards, mobile banking, net banking, all
   one redirect — and cash on delivery. The gateway row appears only
   while the server has credentials for it; cash on delivery is
   always there, and is the whole list when it isn't.

   Billing address defaults to the delivery address, which is what
   `place_order` does when `billing_address_id` is left out.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { draft, guard, paintSummary, selectIn, wireKeyboardActivation } from './checkout.js';

const $ = (sel) => document.querySelector(sel);

let addresses = [];
// From GET /v1/checkout/quote — false when the gateway has no credentials.
let gatewayAvailable = true;

/* ── Icons ────────────────────────────────────────────────────  */

const ICONS = {
  card: '<rect x="1" y="4" width="18" height="12" rx="2.5" stroke="var(--blue)" stroke-width="1.4"/><path d="M1 8h18" stroke="var(--blue)" stroke-width="1.4"/><circle cx="5" cy="13" r="1.5" fill="var(--blue)" opacity=".4"/><circle cx="8" cy="13" r="1.5" fill="var(--blue)"/>',
  cash: '<rect x="2" y="5" width="16" height="11" rx="2" stroke="#94908A" stroke-width="1.3"/><circle cx="10" cy="10.5" r="2.5" stroke="#94908A" stroke-width="1.2"/><path d="M5 5V4a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v1" stroke="#94908A" stroke-width="1.2"/>',
};

/* ── Rows ─────────────────────────────────────────────────────  */

function methodRow({ icon, name, note, kind, methodId, badge }) {
  const row = document.createElement('div');
  row.className = 'payment-method';
  row.dataset.kind = kind;
  if (methodId) row.dataset.methodId = methodId;
  row.setAttribute('role', 'radio');
  row.setAttribute('aria-checked', 'false');
  row.tabIndex = 0;
  row.style.cursor = 'pointer';

  row.innerHTML =
    '<div class="payment-method-head">'
      + '<div class="payment-radio" style="width:16px;height:16px;border-radius:50%;border:2px solid var(--border);display:flex;align-items:center;justify-content:center;flex-shrink:0;"></div>'
      + '<div class="payment-method-icon">'
        + '<svg width="20" height="20" viewBox="0 0 20 20" fill="none">' + icon + '</svg>'
      + '</div>'
      + '<span class="payment-method-name"></span>'
      + '<span class="payment-method-note" style="font-size:12px;color:var(--mid);"></span>'
      + '<span class="payment-method-badge"></span>'
    + '</div>';

  row.querySelector('.payment-method-name').textContent = name;
  row.querySelector('.payment-method-note').textContent = note || '';

  const badgeEl = row.querySelector('.payment-method-badge');
  if (badge) {
    badgeEl.className = 'address-card-default';
    badgeEl.textContent = badge;
  }

  return row;
}

/* Painted by hand rather than by a class, because the wireframe drew
   the selected radio as a filled ring inline and nothing in the
   stylesheet reproduces it. */
function paintRadios(rows) {
  rows.forEach((row) => {
    const dot = row.querySelector('.payment-radio');
    const on = row.classList.contains('selected');
    dot.style.borderColor = on ? 'var(--blue)' : 'var(--border)';
    dot.innerHTML = on
      ? '<div style="width:8px;height:8px;border-radius:50%;background:var(--blue);"></div>'
      : '';
  });
}



/* ── Render ───────────────────────────────────────────────────  */

function render() {
  const host = $('.payment-methods');
  host.innerHTML = '';

  /* Saved cards are not offered here.

     SSLCommerz's hosted checkout takes the card on its own page and
     returns no reusable token, so there is nothing on this server that
     could charge a saved row — picking one would send the shopper to
     the same page to type the same number, having implied otherwise.
     The rows still live on the profile, where they are a record rather
     than a promise. */

  /* PayPal and Apple Pay are gone: nothing routes them. Everything that
     isn't cash on delivery goes to SSLCommerz, whose hosted page covers
     cards, mobile banking and net banking in one redirect — so it is one
     option here, described as what it actually is. */
  const options = [];
  if (gatewayAvailable) {
    options.push({
      icon: ICONS.card,
      name: 'Card, mobile banking or net banking',
      note: "You'll pay on SSLCommerz's secure page, then come back here",
      kind: 'Card',
    });
  }
  options.push({
    icon: ICONS.cash,
    name: 'Cash on Delivery',
    note: 'Pay the courier when it arrives',
    kind: 'CashOnDelivery',
  });
  options.forEach((option) => host.appendChild(methodRow(option)));

  /* Still no card *form* — SSLCommerz's hosted page is where the number
     is typed, which is the point of using them. What changed is that
     paying by card now works; only saving one for next time doesn't. */
  const notice = document.createElement('div');
  notice.style.cssText = 'display:flex;gap:10px;padding:14px 16px;margin-top:4px;border:1px dashed var(--border);border-radius:var(--radius-md);font-size:12.5px;color:var(--mid);line-height:1.6;';
  notice.innerHTML =
    '<svg width="16" height="16" viewBox="0 0 16 16" fill="none" style="flex-shrink:0;margin-top:1px;"><circle cx="8" cy="8" r="6.5" stroke="#94908A" stroke-width="1.2"/><path d="M8 7.5v3.5M8 5.2v.6" stroke="#94908A" stroke-width="1.4" stroke-linecap="round"/></svg>'
    + (gatewayAvailable
      ? '<span><strong style="color:var(--dark);">No card number is typed into Bambe.</strong> '
        + 'You’ll finish paying on SSLCommerz’s own page and come back here. '
        + 'Cards aren’t saved for next time yet.</span>'
      : '<span><strong style="color:var(--dark);">Card payment is unavailable right now.</strong> '
        + 'The payment gateway isn’t reachable, so cash on delivery is the only option '
        + 'at the moment. Nothing is charged until the courier arrives.</span>');
  host.appendChild(notice);

  const rows = Array.prototype.slice.call(host.querySelectorAll('.payment-method'));
  const saved = draft.read();
  const pick = rows.find((row) => (
    saved.payment_method_id
      ? row.dataset.methodId === saved.payment_method_id
      : row.dataset.kind === saved.payment_method_kind && !row.dataset.methodId
  )) || rows[0];

  choose(rows, pick);

  host.addEventListener('click', (e) => {
    const row = e.target.closest('.payment-method');
    if (row) choose(rows, row);
  });
}

function choose(rows, row) {
  selectIn(rows, row);
  paintRadios(rows);
  draft.write({
    payment_method_kind: row.dataset.kind,
    payment_method_id: row.dataset.methodId || null,
  });
}

/* ── Billing address ──────────────────────────────────────────  */

function addressLine(address) {
  return [
    address.recipient_name,
    [address.street, address.city, [address.state, address.zip].filter(Boolean).join(' '), address.country]
      .filter(Boolean).join(', '),
  ].filter(Boolean).join(' · ');
}

function renderBilling() {
  const check = $('#same-billing');
  const summary = $('#billing-summary');
  const picker = $('#billing-picker');
  if (!check || !summary || !picker) return;

  const saved = draft.read();
  const delivery = addresses.find((a) => a.uuid === saved.address_id);
  const same = !saved.billing_address_id || saved.billing_address_id === saved.address_id;
  check.checked = same;

  summary.textContent = delivery ? addressLine(delivery) : 'No delivery address selected';

  picker.innerHTML = '';
  addresses.forEach((address) => {
    const option = document.createElement('div');
    option.className = 'payment-method';
    option.dataset.id = address.uuid;
    option.style.cssText = 'cursor:pointer;padding:12px 14px;font-size:13px;';
    option.textContent = (address.label ? address.label + ' — ' : '') + addressLine(address);
    picker.appendChild(option);
  });

  const options = Array.prototype.slice.call(picker.children);
  const current = options.find((o) => o.dataset.id === (saved.billing_address_id || saved.address_id));
  if (current) selectIn(options, current);

  picker.addEventListener('click', (e) => {
    const option = e.target.closest('[data-id]');
    if (!option) return;
    selectIn(options, option);
    draft.write({ billing_address_id: option.dataset.id });
  });

  const sync = () => {
    summary.style.display = check.checked ? '' : 'none';
    picker.style.display = check.checked ? 'none' : '';
    // Left out entirely when it's the same one: place_order falls back
    // to the delivery address, so there is nothing to say.
    draft.write({ billing_address_id: check.checked ? null : (saved.address_id || null) });
  };

  check.addEventListener('change', sync);
  sync();
}

/* ── Continue ─────────────────────────────────────────────────  */

function wireContinue() {
  const link = document.querySelector('a[href="/checkout/review"]');
  if (!link) return;
  link.addEventListener('click', (e) => {
    if (!draft.read().payment_method_kind) {
      e.preventDefault();
      toast.info('Pick a payment method to continue');
    }
  });
}

/* ── Boot ─────────────────────────────────────────────────────  */
/* Why the shopper is back here rather than on a receipt. SSLCommerz
   redirects to `/checkout/payment?error=...` on a decline, a cancel, or
   a callback we couldn't verify — in every case the order was released
   and they can try again. */
const RETURN_MESSAGE = {
  failed: 'That payment didn’t go through. Nothing was charged — you can try again or pay on delivery.',
  cancelled: 'Payment was cancelled. Your basket is still here.',
  verification: 'We couldn’t confirm that payment with the gateway, so nothing was charged. Please try again.',
};

(async function init() {
  const returned = new URLSearchParams(location.search).get('error');
  if (returned && RETURN_MESSAGE[returned]) {
    toast.error(RETURN_MESSAGE[returned]);
    // Drop the parameter so a refresh doesn't repeat the message.
    history.replaceState(null, '', location.pathname);
  }

  const quote = await guard();
  if (!quote) return;

  gatewayAvailable = quote.card_payment_available !== false;

  // Step one is what makes this step answerable: no address, no
  // delivery method, nothing to bill.
  if (!draft.read().address_id || !draft.read().delivery_method_id) {
    location.href = '/checkout/address';
    return;
  }

  paintSummary(quote, { showItems: false });

  const addressList = await Fetcher.get({ endpoint: '/v1/address/list' });
  addresses = addressList.ok ? (addressList.data || []) : [];

  render();
  renderBilling();
  paintDeliveryNote(quote);
  wireContinue();
  wireKeyboardActivation('.payment-method');
})();

/* The "Delivering to" card under the summary — the wireframe filled it
   with the same invented address as everything else. */
function paintDeliveryNote(quote) {
  const foot = document.querySelector('.order-summary-foot');
  if (!foot) return;
  const box = foot.firstElementChild;
  if (!box) return;

  const saved = draft.read();
  const lines = box.querySelectorAll('div');
  const where = lines[1];
  const how = lines[2];

  const address = addresses.find((a) => a.uuid === saved.address_id);
  if (where) {
    where.textContent = address
      ? [address.street, address.city, [address.state, address.zip].filter(Boolean).join(' ')].filter(Boolean).join(', ')
      : '';
  }

  const method = quote.delivery_method;
  if (how && method) {
    const days = method.min_days === method.max_days
      ? method.max_days + ' days'
      : method.min_days + '–' + method.max_days + ' days';
    how.textContent = method.name + ' · ' + (quote.shipping ? '$' + (quote.shipping / 100).toFixed(2) : 'Free') + ' · ' + days;
  }
}
