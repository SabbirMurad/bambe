/* ═══════════════════════════════════════════════════════════════
   Bambe Web — payment.js

   Step two: how it gets paid for, and where the invoice goes.

   The wireframe drew a card form — number, expiry, CVV — with a
   Visa number typed into it. That form is not wired up here, and
   deliberately so. `POST /v1/payment-method` takes a
   `provider_token`: an opaque handle a processor's own client SDK
   mints in the browser, and the endpoint stores nothing else. There
   is no processor connected, so there is nothing to mint one, and a
   form that posted a real card number to this backend would be
   collecting card data the system is built specifically not to hold.
   The page says that plainly instead.

   What does work: any card already saved on the account, PayPal,
   Apple Pay and cash on delivery. `place_order` records everything
   except cash on delivery as a mock success — so the flow is real
   end to end, and the money is not.

   Billing address defaults to the delivery address, which is what
   `place_order` does when `billing_address_id` is left out.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { draft, guard, paintSummary, selectIn, wireKeyboardActivation } from './checkout.js';

const $ = (sel) => document.querySelector(sel);

let methods = [];
let addresses = [];

/* ── Icons ────────────────────────────────────────────────────  */

const ICONS = {
  card: '<rect x="1" y="4" width="18" height="12" rx="2.5" stroke="var(--blue)" stroke-width="1.4"/><path d="M1 8h18" stroke="var(--blue)" stroke-width="1.4"/><circle cx="5" cy="13" r="1.5" fill="var(--blue)" opacity=".4"/><circle cx="8" cy="13" r="1.5" fill="var(--blue)"/>',
  paypal: '<path d="M6 15H4.5a1.5 1.5 0 0 1-1.48-1.74L5 6h6a3.5 3.5 0 0 1 3.5 3.5C14.5 11.5 13 13 11 13H9L8 15H6Z" stroke="#94908A" stroke-width="1.3" stroke-linejoin="round"/><path d="M9 13l-1 4h2l1-2h1.5a3.5 3.5 0 0 0 3.5-3.5" stroke="#94908A" stroke-width="1.3" stroke-linejoin="round"/>',
  apple: '<path d="M13 3c.5 1 .2 2-.5 2.5C11.8 6.1 11 6 10.5 5.5 10 4.5 10.3 3.5 11 3c.6-.4 1.6-.4 2 0Z" stroke="#94908A" stroke-width="1.2"/><path d="M6 8c1-1.5 2.5-2 4-2 1.2 0 2 .5 3 .5 1 0 2-.5 3.5-.2C18 7 19 9 19 11c0 2.5-1.5 5-3 6.5-.8.8-1.5 1-2.5 1-1 0-1.5-.5-2.5-.5s-1.5.5-2.5.5c-1 0-1.8-.4-2.5-1.5C4.5 15.5 3 12.5 3 10c0-1.8.5-3 3-2Z" stroke="#94908A" stroke-width="1.2"/>',
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

function cardLabel(method) {
  if (method.card_brand && method.last4) return method.card_brand + ' ending ' + method.last4;
  if (method.account_email) return method.account_email;
  return method.kind;
}

function expiryNote(method) {
  if (!method.exp_month || !method.exp_year) return '';
  return 'Expires ' + String(method.exp_month).padStart(2, '0') + '/' + String(method.exp_year).slice(-2);
}

/* ── Render ───────────────────────────────────────────────────  */

function render() {
  const host = $('.payment-methods');
  host.innerHTML = '';

  methods.forEach((method) => {
    host.appendChild(methodRow({
      icon: method.kind === 'PayPal' ? ICONS.paypal : ICONS.card,
      name: cardLabel(method),
      note: expiryNote(method),
      kind: method.kind,
      methodId: method.uuid,
      badge: method.is_default ? 'Default' : null,
    }));
  });

  [
    { icon: ICONS.paypal, name: 'PayPal', note: "You'll be redirected to approve the payment", kind: 'PayPal' },
    { icon: ICONS.apple, name: 'Apple Pay', note: 'Confirm with Touch ID or Face ID', kind: 'ApplePay' },
    { icon: ICONS.cash, name: 'Cash on Delivery', note: 'Pay the courier when it arrives', kind: 'CashOnDelivery' },
  ].forEach((option) => host.appendChild(methodRow(option)));

  /* Saying why the card form is missing, rather than leaving a gap
     where every other shop has one. */
  const notice = document.createElement('div');
  notice.style.cssText = 'display:flex;gap:10px;padding:14px 16px;margin-top:4px;border:1px dashed var(--border);border-radius:var(--radius-md);font-size:12.5px;color:var(--mid);line-height:1.6;';
  notice.innerHTML =
    '<svg width="16" height="16" viewBox="0 0 16 16" fill="none" style="flex-shrink:0;margin-top:1px;"><circle cx="8" cy="8" r="6.5" stroke="#94908A" stroke-width="1.2"/><path d="M8 7.5v3.5M8 5.2v.6" stroke="#94908A" stroke-width="1.4" stroke-linecap="round"/></svg>'
    + '<span><strong style="color:var(--dark);">Adding a new card isn’t available yet.</strong> '
    + 'Card details are tokenised by the payment processor, and none is connected to this store yet — '
    + 'so no card number is ever typed into, or stored by, Bambe. Cards already on your account still work.</span>';
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
(async function init() {
  const quote = await guard();
  if (!quote) return;

  // Step one is what makes this step answerable: no address, no
  // delivery method, nothing to bill.
  if (!draft.read().address_id || !draft.read().delivery_method_id) {
    location.href = '/checkout/address';
    return;
  }

  paintSummary(quote, { showItems: false });

  const [methodList, addressList] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/payment-method/list' }),
    Fetcher.get({ endpoint: '/v1/address/list' }),
  ]);

  methods = methodList.ok ? (methodList.data || []) : [];
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
