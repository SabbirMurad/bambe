/* ═══════════════════════════════════════════════════════════════
   Bambe Web — address.js

   Step one of checkout: where it goes, how it gets there, and what
   it's wrapped in. The page was drawn with two invented addresses
   and three invented delivery options, and the selections went
   nowhere. All three groups are now real:

     · saved addresses     GET/POST/PATCH/DELETE /v1/address
     · delivery method     GET /v1/checkout/delivery-methods
     · packaging           GET /v1/checkout/packaging-options

   The delivery list is priced against this cart, so a method waived
   by a "free over $50" rule shows as free here rather than at the
   end. The summary comes from /v1/checkout/quote — the same
   arithmetic place-order will run — and is refetched on every
   choice rather than adjusted in place.

   What is picked here is written to the checkout draft; the payment
   and review steps read it back. Nothing is sent to the server until
   the order is placed, because there is no draft-order endpoint to
   send it to.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { attachCountrySuggest } from './utils/country-suggest.js';
import { draft, fetchQuote, guard, paintSummary, selectIn, wireKeyboardActivation, money } from './checkout.js';

const $ = (sel) => document.querySelector(sel);

const list = $('#addr-list');
const form = $('#addr-form');
const formTitle = $('#addr-form-title');
const message = $('#addr-msg');
const defaultCheck = $('#a-default');
const addTile = $('#addr-add');

const FIELDS = ['label', 'name', 'street', 'city', 'state', 'zip', 'country', 'phone'];

let addresses = [];
let methods = [];
let packages = [];
let editing = null;   // the address being edited, or null when adding

function input(field) { return document.getElementById('a-' + field); }

/* ── Addresses ────────────────────────────────────────────────  */

/* The API and the form disagree on one name — the API calls it
   `recipient_name`, the form calls it `name` — so the mapping is
   written once, here, in both directions. */
function toForm(address) {
  return {
    label: address.label || '',
    name: address.recipient_name || '',
    street: address.street || '',
    city: address.city || '',
    state: address.state || '',
    zip: address.zip || '',
    country: address.country || '',
    phone: address.phone || '',
  };
}

function fromForm(values) {
  return {
    label: values.label || 'Address',
    recipient_name: values.name,
    street: values.street,
    city: values.city,
    state: values.state || null,
    zip: values.zip || null,
    country: values.country,
    phone: values.phone,
  };
}

function esc(text) {
  return String(text == null ? '' : text).replace(/[&<>"]/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]
  ));
}

function addressCard(address) {
  const card = document.createElement('div');
  card.className = 'address-card';
  card.dataset.addr = '';
  card.dataset.id = address.uuid;
  card.setAttribute('role', 'radio');
  card.setAttribute('aria-checked', 'false');
  card.tabIndex = 0;

  // "Brooklyn, New York 11201" — comma after the city, space before the
  // postcode, and neither left dangling when a country has no state.
  const region = [address.state, address.zip].filter(Boolean).join(' ');
  const cityLine = [address.city, region].filter(Boolean).join(', ');

  card.innerHTML =
    '<div class="address-card-type">'
      + '<span data-field="label"></span>'
      + '<span class="address-card-default" data-field="badge">Default</span>'
    + '</div>'
    + '<span class="address-card-edit" data-edit role="button" tabindex="0">Edit</span>'
    + '<div class="address-card-name" data-field="name"></div>'
    + '<div class="address-card-text" data-field="text"></div>'
    + '<div class="address-card-phone" data-field="phone"></div>'
    + '<div class="address-select-dot">'
      + '<svg width="8" height="7" viewBox="0 0 10 8" fill="none" aria-hidden="true"><path d="M1 4l3 3 5-6" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>'
    + '</div>';

  card.querySelector('[data-field="label"]').textContent = address.label || 'Address';
  card.querySelector('[data-field="name"]').textContent = address.recipient_name;
  card.querySelector('[data-field="phone"]').textContent = address.phone;
  card.querySelector('[data-field="text"]').innerHTML =
    esc(address.street) + '<br>' + esc(cityLine) + '<br>' + esc(address.country);
  card.querySelector('[data-field="badge"]').hidden = !address.is_default;

  return card;
}

function cards() {
  return Array.prototype.slice.call(list.querySelectorAll('[data-addr]'));
}

function chosenId() {
  const card = list.querySelector('[data-addr].selected');
  return card ? card.dataset.id : null;
}

/*
  Renders the book and settles which one is picked: whatever the
  draft already said, otherwise the default, otherwise the first.
  Re-run after every change, so a new address never leaves the page
  with nothing selected.
*/
function renderAddresses() {
  cards().forEach((card) => card.remove());

  const frag = document.createDocumentFragment();
  addresses.forEach((address) => frag.appendChild(addressCard(address)));
  list.insertBefore(frag, addTile);

  if (!addresses.length) {
    draft.write({ address_id: null });
    return;
  }

  const wanted = draft.read().address_id;
  const pick = addresses.find((a) => a.uuid === wanted)
    || addresses.find((a) => a.is_default)
    || addresses[0];

  const card = cards().find((c) => c.dataset.id === pick.uuid);
  if (card) selectIn(cards(), card);
  draft.write({ address_id: pick.uuid });
}

list.addEventListener('click', (e) => {
  const edit = e.target.closest('[data-edit]');
  if (edit) {
    // Editing a card shouldn't also select it — separate intents.
    e.stopPropagation();
    const card = edit.closest('[data-addr]');
    open(addresses.find((a) => a.uuid === card.dataset.id));
    return;
  }
  const card = e.target.closest('[data-addr]');
  if (!card) return;
  selectIn(cards(), card);
  draft.write({ address_id: card.dataset.id });
});

/* ── The form ─────────────────────────────────────────────────  */

function say(text, ok) {
  message.textContent = text || '';
  message.classList.toggle('ok', !!ok);
}

function setChecked(on) {
  defaultCheck.querySelector('.form-check-input').classList.toggle('checked', on);
  defaultCheck.setAttribute('aria-checked', on ? 'true' : 'false');
}
function isChecked() {
  return defaultCheck.querySelector('.form-check-input').classList.contains('checked');
}

function open(address) {
  editing = address || null;
  formTitle.textContent = address ? 'Edit address' : 'Add a new address';

  const values = address ? toForm(address) : {};
  FIELDS.forEach((field) => {
    const el = input(field);
    el.value = values[field] || '';
    /* A <select> has no blank state: assigning '' leaves it on
       selectedIndex -1, which reads back as empty and fails the
       required check. */
    if (el.tagName === 'SELECT' && el.selectedIndex < 0) el.selectedIndex = 0;
  });

  // Something has to be the default, so the box is locked while this
  // address is the one holding that job.
  const isDefault = !!(address && address.is_default);
  setChecked(isDefault);
  defaultCheck.classList.toggle('is-locked', isDefault);

  say('');
  form.hidden = false;
  form.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  input('label').focus();
}

function close() {
  form.hidden = true;
  editing = null;
  say('');
}

addTile.onclick = () => open(null);
attachCountrySuggest(input('country'), { phoneInput: input('phone') });
$('#addr-cancel').onclick = close;
$('#addr-discard').onclick = close;

defaultCheck.onclick = () => {
  if (defaultCheck.classList.contains('is-locked')) return;
  setChecked(!isChecked());
};

$('#addr-save').onclick = async function save() {
  const values = {};
  let missing = null;

  // Label and the state/ZIP pair are the only optional parts — the
  // create handler requires the rest.
  ['name', 'street', 'city', 'country', 'phone'].forEach((field) => {
    values[field] = input(field).value.trim();
    if (!values[field] && !missing) missing = field;
  });
  ['state', 'zip', 'label'].forEach((field) => { values[field] = input(field).value.trim(); });

  if (missing) {
    say('Fill in the ' + missing.replace('zip', 'ZIP') + ' before saving.');
    input(missing).focus();
    return;
  }

  const body = fromForm(values);
  const button = $('#addr-save');
  button.disabled = true;

  const response = editing
    ? await Fetcher.patch({ endpoint: '/v1/address/' + editing.uuid, body, showError: false })
    : await Fetcher.post({ endpoint: '/v1/address', body, showError: false });

  button.disabled = false;

  if (!response.ok) {
    say(response.error || "That couldn't be saved.");
    return;
  }

  const saved = response.data;

  // The default flag is its own endpoint, not a field on the address,
  // so that a book can never end up with two defaults or none.
  if (isChecked() && !saved.is_default) {
    await Fetcher.post({ endpoint: '/v1/address/' + saved.uuid + '/default', showError: false });
  }

  // Whatever was just saved is what they meant to ship to.
  draft.write({ address_id: saved.uuid });
  close();
  await loadAddresses();
  toast.success(editing ? 'Address updated' : 'Address saved');
};

async function loadAddresses() {
  const response = await Fetcher.get({ endpoint: '/v1/address/list' });
  addresses = response.ok ? (response.data || []) : [];
  renderAddresses();
}

/* ── Delivery and packaging ───────────────────────────────────  */

const ICONS = {
  truck: '<path d="M2 5.5h10v7H2v-7ZM12 8h3.2l2.8 2.6v1.9h-6V8Z" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/><circle cx="5.5" cy="14.5" r="1.6" stroke="var(--warn)" stroke-width="1.4"/><circle cx="14.5" cy="14.5" r="1.6" stroke="var(--warn)" stroke-width="1.4"/>',
  bolt: '<path d="M11 2 4 11h4.5l-1 7 7.5-9.5h-4.6L11 2Z" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/>',
  clock: '<circle cx="10" cy="10" r="7.5" stroke="var(--warn)" stroke-width="1.4"/><path d="M10 5.5V10l3 2" stroke="var(--warn)" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>',
  box: '<path d="M2.5 6.2 10 3l7.5 3.2v7.6L10 17l-7.5-3.2V6.2Z" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/><path d="m2.5 6.2 7.5 3.3 7.5-3.3M10 9.5V17" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/>',
  gift: '<path d="M2.5 7.5h15v9h-15v-9ZM2 5h16v2.5H2V5Z" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/><path d="M10 5v11.5" stroke="var(--warn)" stroke-width="1.4"/><path d="M10 5S8.4 2.5 6.8 2.5A1.6 1.6 0 0 0 6.8 5H10Zm0 0s1.6-2.5 3.2-2.5A1.6 1.6 0 0 1 13.2 5H10Z" stroke="var(--warn)" stroke-width="1.4" stroke-linejoin="round"/>',
};

/* Fastest first gets the bolt, the timed one the clock — picked from
   the method's own day range rather than from its name, which is
   whatever an admin typed. */
function deliveryIcon(method) {
  if (method.cutoff_minutes !== null && method.cutoff_minutes !== undefined) return ICONS.clock;
  if (method.max_days <= 2) return ICONS.bolt;
  return ICONS.truck;
}

function optionCard(icon, name, detail, priceText, free) {
  const card = document.createElement('div');
  card.className = 'delivery-option';
  card.setAttribute('role', 'radio');
  card.setAttribute('aria-checked', 'false');
  card.tabIndex = 0;
  card.innerHTML =
    '<span class="delivery-option-icon">'
      + '<svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">' + icon + '</svg>'
    + '</span>'
    + '<div class="delivery-option-info">'
      + '<div class="delivery-option-name"></div>'
      + '<div class="delivery-option-date"></div>'
    + '</div>'
    + '<div class="delivery-option-price' + (free ? ' free' : '') + '"></div>';
  card.querySelector('.delivery-option-name').textContent = name;
  card.querySelector('.delivery-option-date').textContent = detail;
  card.querySelector('.delivery-option-price').textContent = priceText;
  return card;
}

/* "Free over $50" is the reason a $4.99 method is showing as free,
   and saying so is the difference between a discount and a bug. */
function deliveryDetail(method) {
  const parts = [method.description];
  if (method.effective_price === 0 && method.price > 0 && method.free_above) {
    parts.push('Free over ' + money(method.free_above));
  }
  return parts.filter(Boolean).join(' · ');
}

async function loadDelivery() {
  const host = $('#delivery-options');
  const response = await Fetcher.get({
    endpoint: '/v1/checkout/delivery-methods',
    // Priced against this cart, so a waived fee shows as waived here.
    query: { subtotal: currentSubtotal },
  });
  methods = response.ok ? (response.data || []) : [];

  host.innerHTML = '';
  if (!methods.length) {
    host.innerHTML = '<div style="font-size:13px;color:var(--subtle);padding:14px 0;">No delivery methods are available right now.</div>';
    draft.write({ delivery_method_id: null });
    return;
  }

  methods.forEach((method) => {
    const card = optionCard(
      deliveryIcon(method),
      method.name,
      deliveryDetail(method),
      method.effective_price ? money(method.effective_price) : 'Free',
      !method.effective_price,
    );
    card.dataset.id = method.uuid;
    host.appendChild(card);
  });

  const options = Array.prototype.slice.call(host.querySelectorAll('.delivery-option'));
  const wanted = draft.read().delivery_method_id;
  const pick = options.find((o) => o.dataset.id === wanted) || options[0];
  selectIn(options, pick);
  draft.write({ delivery_method_id: pick.dataset.id });

  host.addEventListener('click', (e) => {
    const option = e.target.closest('.delivery-option');
    if (!option) return;
    selectIn(options, option);
    draft.write({ delivery_method_id: option.dataset.id });
    refreshQuote();
  });
}

async function loadPackaging() {
  const host = $('#packaging-options');
  const response = await Fetcher.get({ endpoint: '/v1/checkout/packaging-options' });
  packages = response.ok ? (response.data || []) : [];

  host.innerHTML = '';
  if (!packages.length) {
    // Nothing to choose between — hide the section and its heading
    // rather than leaving an empty numbered step on the page.
    const heading = host.previousElementSibling;
    if (heading && heading.classList.contains('ck-section-title')) heading.style.display = 'none';
    host.style.display = 'none';
    draft.write({ packaging_option_id: null });
    return;
  }

  packages.forEach((option, index) => {
    const card = optionCard(
      index === 0 ? ICONS.box : ICONS.gift,
      option.name,
      option.description,
      option.price ? money(option.price) : 'Free',
      !option.price,
    );
    card.dataset.id = option.uuid;
    host.appendChild(card);
  });

  const options = Array.prototype.slice.call(host.querySelectorAll('.delivery-option'));
  const wanted = draft.read().packaging_option_id;
  const pick = options.find((o) => o.dataset.id === wanted) || options[0];
  selectIn(options, pick);
  draft.write({ packaging_option_id: pick.dataset.id });

  host.addEventListener('click', (e) => {
    const option = e.target.closest('.delivery-option');
    if (!option) return;
    selectIn(options, option);
    draft.write({ packaging_option_id: option.dataset.id });
    refreshQuote();
  });
}

/* ── Summary ──────────────────────────────────────────────────  */

let currentSubtotal = 0;

async function refreshQuote() {
  const quote = await fetchQuote();
  if (!quote) return;
  currentSubtotal = quote.subtotal;
  paintSummary(quote);
}

/* ── Continue ─────────────────────────────────────────────────  */

function wireContinue() {
  const note = document.querySelector('.form-textarea');
  const link = document.querySelector('a[href="/checkout/payment"]');
  if (!link) return;

  link.addEventListener('click', (e) => {
    if (!chosenId()) {
      e.preventDefault();
      toast.info('Add a delivery address to continue');
      addTile.click();
      return;
    }
    if (!draft.read().delivery_method_id) {
      e.preventDefault();
      toast.info('Pick a delivery method to continue');
      return;
    }
    // The note is the shopper's message to the packer; it rides along
    // to place-order as `customer_note`.
    draft.write({ customer_note: note ? note.value.trim() : '' });
  });
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const quote = await guard();
  if (!quote) return;

  currentSubtotal = quote.subtotal;

  const note = document.querySelector('.form-textarea');
  if (note) note.value = draft.read().customer_note || '';

  await loadAddresses();
  await loadDelivery();
  await loadPackaging();
  await refreshQuote();

  wireContinue();
  wireKeyboardActivation('[data-addr], .add-address-card, .delivery-option, [data-edit], .addr-form-close, #a-default');

  if (!addresses.length) open(null);
})();
