/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-settings.js

   Five panels behind five tabs, over the nine
   `/v1/admin/settings/*` endpoints plus the deletion queue.

   The wireframe's five tabs were decorative — every panel was on
   screen at once and the tabs only moved a highlight. They now
   switch panels, because five stacked panels is a long scroll for
   a page you visit to change one field.

   The Payments panel is the interesting one. Its five switches
   were pure decoration: nothing in the model stores which payment
   methods are enabled, and `place_order.rs` accepts whatever the
   checkout sends. Rather than leave switches that flip and mean
   nothing, the panel reports what checkout can actually complete —
   SSLCommerz and cash on delivery — and says which of the two is
   real money.

   The Notifications panel goes the same way: nothing sends staff
   email, and `notification` rows are never written by any handler
   (the customer bell is permanently empty for the same reason).

   In their place, two panels that *are* backed: the account
   deletion queue, and the audit log — both complete endpoints
   with no screen until now.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, ago, fullDate, dateTime, spaced,
  cell, stacked, fillTable, paintPager, rowButton, tag, mountAdminChrome,
  openModal, confirmModal, field, textInput, selectInput, uploadImage,
} from './admin-core.js';
import { paintImage, clearImage } from './catalog.js';

const TABS = ['Store', 'Payments', 'Shipping', 'Staff', 'Activity'];

const CURRENCIES = ['BDT', 'USD', 'GBP', 'EUR', 'CAD', 'AUD', 'JPY'];

const STAFF_ROLES = ['Owner', 'Manager', 'SupportAgent', 'Analyst'];
const STAFF_STATUS = ['Pending', 'Active', 'Suspended'];
const STATUS_TAG = { Active: 'tag-green', Pending: 'tag-gold', Suspended: 'tag-red' };

const RATE_KINDS = [
  { value: 'Flat', label: 'Flat rate' },
  { value: 'Free', label: 'Free' },
  { value: 'Calculated', label: 'Calculated at checkout' },
];

let store = null;
let zones = [];
let panels = [];
let auditPage = 1;

/* ── Tabs ─────────────────────────────────────────────────────  */

function showPanel(index) {
  panels.forEach((panel, i) => { panel.style.display = i === index ? '' : 'none'; });
  $$('.admin-tabs .admin-tab').forEach((tab, i) => {
    tab.classList.toggle('active', i === index);
  });
  const audit = $('#audit-body');
  if (index === 4 && audit && !audit.dataset.loaded) loadAudit();
}

/* ── Store panel ──────────────────────────────────────────────  */

/* ── Theme ───────────────────────────────────────────────────
   Picking an option saves it straight away — it is one decision, and
   the panel's Save button sits in the header, often scrolled out of
   view, so a preview that waited for it read as done when it wasn't.
   The console switches at once (the attribute the server stamps on
   <html>); a failed save puts the previous theme back. */
function selectedTheme() {
  const on = $('#store-theme .theme-option[aria-checked="true"]');
  return on ? on.dataset.themeValue : 'orange';
}

function paintTheme(theme) {
  $$('#store-theme .theme-option').forEach((option) => {
    option.setAttribute('aria-checked', String(option.dataset.themeValue === theme));
  });
  document.documentElement.dataset.storeTheme = theme;
}

const THEME_NAMES = { orange: 'Orange', crimson: 'Crimson', green: 'Green' };

async function chooseTheme(theme) {
  const previous = selectedTheme();
  if (theme === previous) return;
  paintTheme(theme);

  const response = await Fetcher.patch({ endpoint: '/v1/admin/settings/store', body: { theme } });
  if (!response.ok) {
    paintTheme(previous);
    return;
  }
  if (store) store.theme = theme;
  toast.success('Theme set to ' + (THEME_NAMES[theme] || theme) + ' for the whole store');
}

function wireTheme() {
  const options = $$('#store-theme .theme-option');
  options.forEach((option, index) => {
    option.addEventListener('click', () => chooseTheme(option.dataset.themeValue));
    option.addEventListener('keydown', (e) => {
      if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
      e.preventDefault();
      const next = options[(index + (e.key === 'ArrowRight' ? 1 : options.length - 1)) % options.length];
      next.focus();
      chooseTheme(next.dataset.themeValue);
    });
  });
}

function paintStore() {
  if (!store) return;
  paintTheme(store.theme || 'orange');

  const set = (id, value) => { const el = $(id); if (el) el.value = value === null || value === undefined ? '' : value; };
  set('#store-name', store.store_name);
  set('#store-email', store.support_email);
  set('#store-phone', store.support_phone);
  set('#store-facebook', store.facebook_url);
  set('#store-instagram', store.instagram_url);
  set('#store-address', store.business_address);

  const currency = $('#store-currency');
  if (currency) {
    currency.innerHTML = '';
    CURRENCIES.forEach((code) => {
      const option = document.createElement('option');
      option.value = code;
      option.textContent = code;
      if (code === store.default_currency) option.selected = true;
      currency.appendChild(option);
    });
  }
}

/*
  Four fields the checkout depends on and the wireframe never drew:
  the tax rate, whether prices include it, the order-number prefix
  and maintenance mode. `pricing::totals` reads the first two on
  every quote and every order, so leaving them uneditable meant the
  only way to change the store's tax rate was in Mongo.
*/
function addStoreExtras(panel) {
  const body = panel.querySelector('.admin-panel-body');
  if (!body) return;

  const grid = document.createElement('div');
  grid.className = 'settings-grid';
  grid.style.marginTop = '18px';

  const taxGroup = document.createElement('div');
  taxGroup.className = 'form-group';
  const taxLabel = document.createElement('label');
  taxLabel.className = 'form-label';
  taxLabel.textContent = 'Tax rate';
  const tax = document.createElement('input');
  tax.className = 'form-input';
  tax.id = 'store-tax';
  tax.type = 'text';
  tax.value = ((store.tax_rate_bps || 0) / 100).toString();
  const taxHint = document.createElement('div');
  taxHint.className = 'form-hint';
  taxHint.textContent = 'Percent, applied after any discount. 8.75 means 8.75%.';
  taxGroup.append(taxLabel, tax, taxHint);

  const prefixGroup = document.createElement('div');
  prefixGroup.className = 'form-group';
  const prefixLabel = document.createElement('label');
  prefixLabel.className = 'form-label';
  prefixLabel.textContent = 'Order number prefix';
  const prefix = document.createElement('input');
  prefix.className = 'form-input';
  prefix.id = 'store-prefix';
  prefix.type = 'text';
  prefix.value = store.order_number_prefix || '';
  const prefixHint = document.createElement('div');
  prefixHint.className = 'form-hint';
  prefixHint.textContent = 'Existing orders keep the prefix they were given.';
  prefixGroup.append(prefixLabel, prefix, prefixHint);

  grid.append(taxGroup, prefixGroup);
  body.appendChild(grid);

  body.appendChild(switchRow(
    'Prices include tax',
    'When on, the tax shown at checkout is already inside the listed price.',
    'store-includes-tax',
    store.prices_include_tax));

  body.appendChild(switchRow(
    'Maintenance mode',
    'Takes the storefront down. The admin console stays reachable.',
    'store-maintenance',
    store.maintenance_mode));
}

/* A settings row with a switch this file owns — `toggle.js` is told
   to keep out, so there is one owner per switch. */
function switchRow(label, description, id, on) {
  const row = document.createElement('div');
  row.className = 'settings-row';

  const text = document.createElement('div');
  const heading = document.createElement('div');
  heading.className = 'settings-row-label';
  heading.textContent = label;
  const detail = document.createElement('div');
  detail.className = 'settings-row-desc';
  detail.textContent = description;
  text.append(heading, detail);

  const control = document.createElement('div');
  control.className = 'settings-row-control';
  control.style.cssText = 'display:flex;justify-content:flex-end;';

  const toggle = document.createElement('div');
  toggle.className = 'toggle' + (on ? ' on' : '');
  toggle.id = id;
  toggle.dataset.readonly = 'true';
  toggle.setAttribute('role', 'switch');
  toggle.tabIndex = 0;
  toggle.setAttribute('aria-checked', String(!!on));
  toggle.setAttribute('aria-label', label);

  const flip = () => {
    const next = !toggle.classList.contains('on');
    toggle.classList.toggle('on', next);
    toggle.setAttribute('aria-checked', String(next));
  };
  toggle.addEventListener('click', flip);
  toggle.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
  });

  control.appendChild(toggle);
  row.append(text, control);
  return row;
}

async function saveStore(button) {
  const value = (id) => { const el = $(id); return el ? el.value.trim() : null; };
  const on = (id) => { const el = $(id); return el ? el.classList.contains('on') : false; };

  const taxPercent = Number(value('#store-tax'));
  if (!Number.isFinite(taxPercent) || taxPercent < 0) {
    toast.error('The tax rate has to be a positive number.');
    return;
  }

  button.disabled = true;
  const response = await Fetcher.patch({
    endpoint: '/v1/admin/settings/store',
    body: {
      store_name: value('#store-name'),
      support_email: value('#store-email'),
      // '' clears a field; the footer hides a button whose link is empty.
      support_phone: value('#store-phone') || '',
      facebook_url: value('#store-facebook') || '',
      instagram_url: value('#store-instagram') || '',
      business_address: value('#store-address'),
      default_currency: value('#store-currency'),
      order_number_prefix: value('#store-prefix'),
      tax_rate_bps: Math.round(taxPercent * 100),
      prices_include_tax: on('#store-includes-tax'),
      maintenance_mode: on('#store-maintenance'),
      theme: selectedTheme(),
    },
  });
  button.disabled = false;
  if (!response.ok) return;

  toast.success('Store settings saved');
  loadStore();
}

/* ── Payments panel ───────────────────────────────────────────  */

function rebuildPayments(panel) {
  const head = panel.querySelector('.admin-panel-head');
  panel.innerHTML = '';
  if (head) {
    const link = head.querySelector('.admin-panel-link');
    if (link) link.remove();
    panel.appendChild(head);
  }

  const body = document.createElement('div');
  body.className = 'admin-panel-body';

  const notice = document.createElement('div');
  notice.style.cssText = 'padding:16px 18px;border:1px dashed var(--border);'
    + 'border-radius:var(--radius-md);background:var(--input-bg);'
    + 'font-size:13px;color:var(--subtle);line-height:1.7;';
  notice.textContent = 'Checkout offers two things: SSLCommerz, whose hosted page '
    + 'takes cards, mobile banking and net banking, and cash on delivery. '
    + 'An SSLCommerz payment is real — the order is only marked paid once their '
    + 'callback validates it — while a refund recorded in the order drawer is '
    + 'still bookkeeping and does not reach the gateway. There is no setting '
    + 'here to change that: which methods are offered is not stored anywhere, '
    + 'and the gateway credentials are server-side configuration.';
  body.appendChild(notice);

  const list = document.createElement('div');
  list.style.cssText = 'margin-top:18px;display:flex;flex-direction:column;gap:10px;';

  [
    ['SSLCommerz', 'Card, mobile banking and net banking, on their hosted page. Shown at checkout only while the server has credentials for it.', 'Available'],
    ['Cash on delivery', 'Always offered. The order stays unpaid until the courier collects.', 'Available'],
    ['Saved cards', 'Not offered. The hosted page returns no reusable token, so a saved row could never be charged.', 'Not offered'],
  ].forEach(([name, detail, state]) => {
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;justify-content:space-between;align-items:center;'
      + 'gap:16px;padding:10px 0;border-bottom:1px solid var(--border);';

    const text = document.createElement('div');
    const heading = document.createElement('div');
    heading.className = 'settings-row-label';
    heading.textContent = name;
    const description = document.createElement('div');
    description.className = 'settings-row-desc';
    description.textContent = detail;
    text.append(heading, description);

    row.appendChild(text);
    row.appendChild(tag(state, state === 'Available' ? 'tag-green' : 'tag-gray'));
    list.appendChild(row);
  });

  body.appendChild(list);
  panel.appendChild(body);
}

/* ── Delivery methods and packaging ───────────────────────────
   These are the two collections checkout actually charges from —
   `delivery_method` sets the shipping line, `packaging_option`
   the packaging line. Neither had an admin endpoint until now, so
   the only way to add or reprice one was to insert it into Mongo
   by hand, and the Shipping tab could only show *zones*, which
   checkout never reads.

   Both live under the Shipping tab because that is where someone
   goes looking for them. The zones panel keeps its footnote, but
   it no longer has to apologise for being the only thing there.  */

let deliveryMethods = [];
let packagingOptions = [];

function daysLabel(method) {
  if (method.min_days === method.max_days) {
    return method.min_days === 1 ? '1 day' : method.min_days + ' days';
  }
  return method.min_days + '–' + method.max_days + ' days';
}

function cutoffLabel(minutes) {
  if (minutes === null || minutes === undefined) return null;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return 'Order by ' + String(h).padStart(2, '0') + ':' + String(m).padStart(2, '0');
}

function methodRow(method) {
  const row = document.createElement('div');
  row.className = 'zone-row';
  if (!method.enabled) row.style.opacity = '.55';

  const text = document.createElement('div');
  text.style.flex = '1';
  const name = document.createElement('div');
  name.className = 'zone-name';
  name.textContent = method.name;
  const detail = document.createElement('div');
  detail.className = 'zone-desc';
  const bits = [daysLabel(method)];
  if (method.description) bits.unshift(method.description);
  const cutoff = cutoffLabel(method.cutoff_minutes);
  if (cutoff) bits.push(cutoff);
  detail.textContent = bits.join(' · ');
  text.append(name, detail);
  row.appendChild(text);

  row.appendChild(tag(method.price === 0 ? 'Free' : money(method.price),
    method.price === 0 ? 'tag-green' : 'tag-blue'));
  if (method.free_above) row.appendChild(tag('Free over ' + money(method.free_above), 'tag-green'));
  if (!method.enabled) row.appendChild(tag('Disabled', 'tag-red'));

  row.appendChild(rowButton('Edit', () => editMethod(method), { needs: 'shipping' }));
  row.appendChild(rowButton(method.enabled ? 'Disable' : 'Enable',
    () => toggleMethod(method)));
  row.appendChild(rowButton('Delete', () => removeMethod(method), { danger: true, needs: 'shipping' }));

  return row;
}

function methodForm(method) {
  const form = document.createElement('div');

  const name = textInput({ value: method ? method.name : '', placeholder: 'Express Delivery' });
  form.appendChild(field('Name', name));

  const description = textInput({
    value: method ? method.description : '',
    placeholder: 'Estimated 1–2 business days',
  });
  form.appendChild(field('Description', description, 'Shown to the shopper at checkout.'));

  const price = textInput({ value: method ? (method.price / 100).toFixed(2) : '0.00' });
  form.appendChild(field('Price', price));

  const freeAbove = textInput({
    value: method && method.free_above ? (method.free_above / 100).toFixed(2) : '',
  });
  form.appendChild(field('Free above', freeAbove, 'Blank means the price always applies.'));

  const minDays = textInput({ value: method ? String(method.min_days) : '1' });
  form.appendChild(field('Earliest (days)', minDays));

  const maxDays = textInput({ value: method ? String(method.max_days) : '3' });
  form.appendChild(field('Latest (days)', maxDays));

  const cutoff = document.createElement('input');
  cutoff.type = 'time';
  cutoff.className = 'form-input';
  if (method && method.cutoff_minutes !== null && method.cutoff_minutes !== undefined) {
    const h = Math.floor(method.cutoff_minutes / 60);
    const m = method.cutoff_minutes % 60;
    cutoff.value = String(h).padStart(2, '0') + ':' + String(m).padStart(2, '0');
  }
  form.appendChild(field('Same-day cutoff', cutoff,
    'Blank for none. Orders after this time ship the next day.'));

  return {
    form,
    read: () => {
      const [h, m] = (cutoff.value || '').split(':');
      return {
        name: name.value.trim(),
        description: description.value.trim(),
        price: Math.round(Number(price.value || 0) * 100),
        free_above: freeAbove.value.trim() ? Math.round(Number(freeAbove.value) * 100) : null,
        min_days: Number(minDays.value || 0),
        max_days: Number(maxDays.value || 0),
        cutoff_minutes: cutoff.value ? Number(h) * 60 + Number(m) : null,
        enabled: method ? method.enabled : true,
        position: method ? method.position : deliveryMethods.length,
      };
    },
    /* The endpoint checks all of this too — this is only so a
       mistake is caught before a round trip. */
    validate: () => {
      if (!name.value.trim()) return 'The method needs a name.';
      if (Number(minDays.value) > Number(maxDays.value)) {
        return 'The earliest day can’t be after the latest.';
      }
      if (Number(price.value) < 0) return 'Price can’t be negative.';
      return null;
    },
  };
}

function editMethod(method) {
  const { form, read, validate } = methodForm(method);

  openModal({
    title: method ? 'Edit ' + method.name : 'Add a delivery method',
    body: form,
    width: '500px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: method ? 'Save' : 'Add method',
        className: 'btn-primary',
        onClick: async (close, button) => {
          const problem = validate();
          if (problem) { toast.error(problem); return; }

          button.disabled = true;
          const response = method
            ? await Fetcher.patch({ endpoint: '/v1/admin/settings/delivery-method/' + method.uuid, body: read() })
            : await Fetcher.post({ endpoint: '/v1/admin/settings/delivery-method', body: read() });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success(method ? 'Method updated' : 'Method added');
          loadFulfilment();
        },
      },
    ],
  });
}

async function toggleMethod(method) {
  const response = await Fetcher.patch({
    endpoint: '/v1/admin/settings/delivery-method/' + method.uuid,
    body: { enabled: !method.enabled },
  });
  if (!response.ok) return;
  toast.success(method.enabled ? 'Method disabled' : 'Method enabled');
  loadFulfilment();
}

async function removeMethod(method) {
  const confirmed = await confirmModal({
    title: 'Delete ' + method.name + '?',
    message: 'Orders already placed keep the delivery they were charged for.',
    confirmLabel: 'Delete method',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/settings/delivery-method/' + method.uuid });
  if (!response.ok) return;
  toast.success('Method deleted');
  loadFulfilment();
}

function packagingRow(option) {
  const row = document.createElement('div');
  row.className = 'zone-row';
  if (!option.enabled) row.style.opacity = '.55';

  const thumb = document.createElement('div');
  thumb.className = 'pack-thumb';
  thumb.title = option.image ? '' : 'No picture';
  paintPicture(thumb, option.image);
  row.appendChild(thumb);

  const text = document.createElement('div');
  text.style.flex = '1';
  const name = document.createElement('div');
  name.className = 'zone-name';
  name.textContent = option.name;
  const detail = document.createElement('div');
  detail.className = 'zone-desc';
  detail.textContent = option.description || 'No description';
  text.append(name, detail);
  row.appendChild(text);

  row.appendChild(tag(option.price === 0 ? 'Free' : money(option.price),
    option.price === 0 ? 'tag-green' : 'tag-blue'));
  if (option.multi_item_only) row.appendChild(tag('2+ items', 'tag-blue'));
  if (!option.enabled) row.appendChild(tag('Disabled', 'tag-red'));

  row.appendChild(rowButton('Edit', () => editPackaging(option), { needs: 'shipping' }));
  row.appendChild(rowButton('Delete', () => removePackaging(option), { danger: true, needs: 'shipping' }));

  return row;
}

/* ── Packaging picture ────────────────────────────────────────
   Uploaded the moment a file is picked (as a PackagingImage) and kept
   on the option as its served path, "/image/webp/<id>" — the cart
   puts that straight into an <img>. Saved with the form, like every
   other field. A path from before uploads (/assets/...) still shows. */
const UPLOAD_PREFIX = '/image/webp/';

function paintPicture(el, path) {
  if (path && path.startsWith(UPLOAD_PREFIX)) {
    paintImage(el, path.slice(UPLOAD_PREFIX.length));
  } else if (path) {
    el.style.background = "url('" + path.replace(/'/g, '%27') + "') center / cover no-repeat";
  } else {
    clearImage(el);
    el.style.background = '';
  }
}

function packagingPicture(initial) {
  let path = initial || '';

  const el = document.createElement('div');
  el.className = 'cat-image-picker';

  const preview = document.createElement('div');
  preview.className = 'cat-image-preview';
  preview.setAttribute('role', 'button');
  preview.tabIndex = 0;
  preview.setAttribute('aria-label', 'Choose a picture');

  const actions = document.createElement('div');
  actions.className = 'cat-image-actions';
  const choose = document.createElement('button');
  choose.type = 'button';
  choose.className = 'btn btn-ghost btn-sm';
  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'btn btn-ghost btn-sm';
  remove.textContent = 'Remove';
  actions.append(choose, remove);

  const input = document.createElement('input');
  input.type = 'file';
  input.accept = 'image/*';
  input.hidden = true;

  const paint = () => {
    preview.textContent = path ? '' : 'No picture';
    preview.classList.toggle('has-image', !!path);
    paintPicture(preview, path);
    choose.textContent = path ? 'Replace' : 'Upload picture';
    remove.style.display = path ? '' : 'none';
  };

  const open = () => input.click();
  choose.addEventListener('click', open);
  preview.addEventListener('click', open);
  preview.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });
  remove.addEventListener('click', () => { path = ''; paint(); });

  input.addEventListener('change', async () => {
    const file = input.files && input.files[0];
    input.value = '';
    if (!file) return;
    if (!file.type.startsWith('image/')) { toast.error('That file isn’t an image.'); return; }

    choose.disabled = true;
    choose.textContent = 'Uploading…';
    const id = await uploadImage(file, 'PackagingImage');
    choose.disabled = false;
    if (id) path = UPLOAD_PREFIX + id;
    paint();
  });

  el.append(preview, actions, input);
  paint();
  return { el, value: () => path };
}

function editPackaging(option) {
  const form = document.createElement('div');

  const name = textInput({ value: option ? option.name : '', placeholder: 'Gift wrapping' });
  form.appendChild(field('Name', name));

  const description = textInput({
    value: option ? option.description : '',
    placeholder: 'Kraft paper, ribbon and a handwritten card',
  });
  form.appendChild(field('Description', description));

  const price = textInput({ value: option ? (option.price / 100).toFixed(2) : '0.00' });
  form.appendChild(field('Price', price));

  const image = packagingPicture(option ? option.image : '');
  form.appendChild(field('Picture', image.el, 'Shown to shoppers beside this option on the cart page.'));

  const multiItem = document.createElement('input');
  multiItem.type = 'checkbox';
  multiItem.checked = !!(option && option.multi_item_only);
  const multiLabel = document.createElement('label');
  multiLabel.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:13px;cursor:pointer;';
  multiLabel.append(multiItem, document.createTextNode('Only offer when the cart has two or more items'));
  form.appendChild(field('Availability', multiLabel));

  openModal({
    title: option ? 'Edit ' + option.name : 'Add a packaging option',
    body: form,
    width: '460px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: option ? 'Save' : 'Add option',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (!name.value.trim()) { toast.error('The option needs a name.'); return; }

          const payload = {
            name: name.value.trim(),
            description: description.value.trim(),
            price: Math.round(Number(price.value || 0) * 100),
            image: image.value(),
            multi_item_only: multiItem.checked,
            enabled: option ? option.enabled : true,
            position: option ? option.position : packagingOptions.length,
          };

          button.disabled = true;
          const response = option
            ? await Fetcher.patch({ endpoint: '/v1/admin/settings/packaging-option/' + option.uuid, body: payload })
            : await Fetcher.post({ endpoint: '/v1/admin/settings/packaging-option', body: payload });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success(option ? 'Option updated' : 'Option added');
          loadFulfilment();
        },
      },
    ],
  });
}

async function removePackaging(option) {
  const confirmed = await confirmModal({
    title: 'Delete ' + option.name + '?',
    message: 'Orders already placed keep the packaging they were charged for.',
    confirmLabel: 'Delete option',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/settings/packaging-option/' + option.uuid });
  if (!response.ok) return;
  toast.success('Option deleted');
  loadFulfilment();
}

/** An empty `.admin-panel` with a head, a body and an "Add" link. */
function buildPanel(title, addLabel, onAdd, blurb) {
  const panel = document.createElement('div');
  panel.className = 'admin-panel';

  const head = document.createElement('div');
  head.className = 'admin-panel-head';
  const heading = document.createElement('div');
  heading.className = 'admin-panel-title';
  heading.textContent = title;
  head.appendChild(heading);

  const link = document.createElement('span');
  link.className = 'admin-panel-link';
  link.dataset.needs = 'shipping';   // both panels built here are shipping's
  link.tabIndex = 0;
  link.textContent = addLabel;
  link.addEventListener('click', onAdd);
  link.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onAdd(); }
  });
  head.appendChild(link);
  panel.appendChild(head);

  const body = document.createElement('div');
  body.className = 'admin-panel-body';
  panel.appendChild(body);

  if (blurb) {
    const note = document.createElement('div');
    note.style.cssText = 'padding:0 18px 14px;font-size:12px;color:var(--subtle);line-height:1.6;';
    note.textContent = blurb;
    body.appendChild(note);
  }

  return { panel, body };
}

async function loadFulfilment() {
  const [methods, packaging] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/admin/settings/delivery-method/list' }),
    Fetcher.get({ endpoint: '/v1/admin/settings/packaging-option/list' }),
  ]);

  deliveryMethods = methods.ok ? (methods.data || []) : [];
  packagingOptions = packaging.ok ? (packaging.data || []) : [];

  const methodBody = $('#delivery-body');
  const packagingBody = $('#packaging-body');
  if (!methodBody || !packagingBody) return;

  $$('.zone-row', methodBody).forEach((row) => row.remove());
  $$('.zone-row', packagingBody).forEach((row) => row.remove());

  if (deliveryMethods.length) {
    deliveryMethods.forEach((m) => methodBody.appendChild(methodRow(m)));
  } else {
    const empty = document.createElement('div');
    empty.className = 'zone-row';
    empty.style.cssText = 'color:var(--subtle);font-size:13px;';
    empty.textContent = 'No delivery methods — checkout cannot quote a price until there is one.';
    methodBody.appendChild(empty);
  }

  if (packagingOptions.length) {
    packagingOptions.forEach((o) => packagingBody.appendChild(packagingRow(o)));
  } else {
    const empty = document.createElement('div');
    empty.className = 'zone-row';
    empty.style.cssText = 'color:var(--subtle);font-size:13px;';
    empty.textContent = 'No packaging options. Checkout offers “No packaging” regardless.';
    packagingBody.appendChild(empty);
  }
}

/* ── Shipping panel ───────────────────────────────────────────  */

function rateLabel(zone) {
  if (zone.rate_kind === 'Free') return 'Free';
  if (zone.rate_kind === 'Calculated') return 'Calculated';
  return money(zone.rate) + ' flat';
}

function zoneRow(zone) {
  const row = document.createElement('div');
  row.className = 'zone-row';
  if (!zone.enabled) row.style.opacity = '.55';

  const text = document.createElement('div');
  text.style.flex = '1';
  const name = document.createElement('div');
  name.className = 'zone-name';
  name.textContent = zone.name;
  const detail = document.createElement('div');
  detail.className = 'zone-desc';
  detail.textContent = (zone.countries || []).length
    ? (zone.countries || []).join(', ')
    : 'Everywhere not matched by another zone';
  text.append(name, detail);
  row.appendChild(text);

  row.appendChild(tag(
    zone.free_above ? 'Free over ' + money(zone.free_above) : rateLabel(zone),
    zone.rate_kind === 'Free' || zone.free_above ? 'tag-green' : 'tag-blue'));

  if (!zone.enabled) row.appendChild(tag('Disabled', 'tag-red'));

  row.appendChild(rowButton('Edit', () => editZone(zone), { needs: 'shipping' }));
  row.appendChild(rowButton('Delete', () => removeZone(zone), { danger: true, needs: 'shipping' }));

  return row;
}

function zoneForm(zone) {
  const form = document.createElement('div');

  const name = textInput({ value: zone ? zone.name : '', placeholder: 'United Kingdom' });
  form.appendChild(field('Zone name', name));

  const countries = textInput({
    value: zone ? (zone.countries || []).join(', ') : '',
    placeholder: 'GB, IE',
  });
  form.appendChild(field('Countries', countries,
    'ISO two-letter codes, comma separated. Leave blank for everywhere else.'));

  const kind = selectInput(RATE_KINDS, zone ? zone.rate_kind : 'Flat');
  form.appendChild(field('Rate', kind));

  const rate = textInput({ value: zone ? (zone.rate / 100).toFixed(2) : '0.00' });
  form.appendChild(field('Amount', rate, 'Ignored for Free and Calculated.'));

  const freeAbove = textInput({
    value: zone && zone.free_above ? (zone.free_above / 100).toFixed(2) : '',
  });
  form.appendChild(field('Free above', freeAbove, 'Blank means the rate always applies.'));

  return {
    form,
    read: () => ({
      name: name.value.trim(),
      countries: countries.value.split(',').map((c) => c.trim().toUpperCase()).filter(Boolean),
      rate_kind: kind.value,
      rate: Math.round(Number(rate.value || 0) * 100),
      free_above: freeAbove.value.trim() ? Math.round(Number(freeAbove.value) * 100) : null,
      enabled: zone ? zone.enabled : true,
      position: zone ? zone.position : zones.length,
    }),
    valid: () => !!name.value.trim(),
  };
}

function editZone(zone) {
  const { form, read, valid } = zoneForm(zone);

  openModal({
    title: zone ? 'Edit ' + zone.name : 'Add a shipping zone',
    body: form,
    width: '500px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: zone ? 'Save' : 'Add zone',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (!valid()) { toast.error('The zone needs a name.'); return; }

          button.disabled = true;
          const response = zone
            ? await Fetcher.patch({ endpoint: '/v1/admin/settings/shipping-zone/' + zone.uuid, body: read() })
            : await Fetcher.post({ endpoint: '/v1/admin/settings/shipping-zone', body: read() });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success(zone ? 'Zone updated' : 'Zone added');
          loadZones();
        },
      },
    ],
  });
}

async function removeZone(zone) {
  const confirmed = await confirmModal({
    title: 'Delete ' + zone.name + '?',
    message: 'Orders already placed keep the shipping they were charged.',
    confirmLabel: 'Delete zone',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/settings/shipping-zone/' + zone.uuid });
  if (!response.ok) return;
  toast.success('Zone deleted');
  loadZones();
}

async function loadZones() {
  const response = await Fetcher.get({ endpoint: '/v1/admin/settings/shipping-zone/list' });
  // panels[2] is the Shipping wrapper; the zones panel is its last child.
  const panel = panels[2] && panels[2].lastElementChild;
  if (!panel) return;

  zones = response.ok ? (response.data || []) : [];

  $$('.zone-row', panel).forEach((row) => row.remove());

  /* Zones are still not what checkout charges — `delivery_method`
     is, and it now has its own panel directly above this one. The
     footnote stays so nobody edits a zone expecting the shipping
     line to move.

     Created once and kept as the last child: the rows are rebuilt on
     every load, so appending them would otherwise leave the footnote
     stranded above the list it is a footnote to. */
  let note = panel.querySelector('.zone-note');
  if (!note) {
    note = document.createElement('div');
    note.className = 'zone-note';
    note.style.cssText = 'padding:14px 18px;font-size:12.5px;color:var(--subtle);'
      + 'line-height:1.7;border-top:1px solid var(--border);';
    note.textContent = 'Zones describe where you ship. What a customer is actually '
      + 'charged comes from the delivery methods offered at checkout, which are a '
      + 'separate collection with no screen here yet.';
    panel.appendChild(note);
  }

  if (!zones.length) {
    const empty = document.createElement('div');
    empty.className = 'zone-row';
    empty.style.cssText = 'color:var(--subtle);font-size:13px;justify-content:center;';
    empty.textContent = 'No shipping zones yet.';
    panel.insertBefore(empty, note);
  } else {
    zones.forEach((zone) => panel.insertBefore(zoneRow(zone), note));
  }
}

/* ── Staff panel ──────────────────────────────────────────────  */

function staffRow(member) {
  const tr = document.createElement('tr');

  const media = document.createElement('div');
  media.className = 'cell-media';
  const avatar = document.createElement('div');
  avatar.className = 'cell-avatar';
  avatar.textContent = (member.name || member.email || '?').slice(0, 2).toUpperCase();
  const text = document.createElement('div');
  const name = document.createElement('div');
  name.className = 'cell-strong';
  name.textContent = member.name || '—';
  const email = document.createElement('div');
  email.className = 'cell-sub';
  email.textContent = member.email || '';
  text.append(name, email);
  media.append(avatar, text);
  tr.appendChild(cell(media));

  tr.appendChild(cell(spaced(member.role)));
  tr.appendChild(cell(member.last_active_at ? ago(member.last_active_at) : 'Never signed in', 'nowrap'));
  tr.appendChild(cell(tag(member.status, STATUS_TAG[member.status] || 'tag-blue')));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Edit', () => editStaff(member), { needs: 'staff' }));
  tr.appendChild(cell(actions, 'col-actions'));

  return tr;
}

function editStaff(member) {
  const form = document.createElement('div');

  const role = selectInput(STAFF_ROLES.map((value) => ({ value, label: spaced(value) })), member.role);
  form.appendChild(field('Role', role));

  const status = selectInput(STAFF_STATUS.map((value) => ({ value, label: value })), member.status);
  form.appendChild(field('Status', status,
    'Suspended keeps the account but refuses it entry to the console.'));

  openModal({
    title: member.name || member.email,
    body: form,
    width: '440px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Save',
        className: 'btn-primary',
        onClick: async (close, button) => {
          button.disabled = true;
          const response = await Fetcher.patch({
            endpoint: '/v1/admin/settings/staff/' + member.uuid,
            body: { role: role.value, status: status.value },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Staff member updated');
          loadStaff();
        },
      },
    ],
  });
}

function inviteStaff() {
  const form = document.createElement('div');

  const notice = document.createElement('div');
  notice.style.cssText = 'font-size:12.5px;color:var(--subtle);line-height:1.7;margin-bottom:16px;';
  notice.textContent = 'This attaches a console role to an admin account that already exists. '
    + 'The account itself is created through admin sign-up — no email is sent from here.';
  form.appendChild(notice);

  const email = textInput({ placeholder: 'name@bambee.store' });
  form.appendChild(field('Admin account email', email));

  const role = selectInput(STAFF_ROLES.map((value) => ({ value, label: spaced(value) })), 'Manager');
  form.appendChild(field('Role', role));

  openModal({
    title: 'Add a staff member',
    body: form,
    width: '460px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Add',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (!email.value.trim()) { toast.error('An email is required.'); return; }

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/admin/settings/staff/invite',
            body: { email: email.value.trim(), role: role.value },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Staff member added');
          loadStaff();
        },
      },
    ],
  });
}

async function loadStaff() {
  const panel = panels[3];
  if (!panel) return;
  const tbody = panel.querySelector('tbody');
  if (!tbody) return;

  const response = await Fetcher.get({ endpoint: '/v1/admin/settings/staff/list' });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load the staff list" });
    return;
  }

  fillTable(tbody, (response.data || []).map(staffRow), {
    empty: 'No staff profiles yet — add one against an existing admin account.',
  });
}

/* ── Activity panel: deletion queue + audit log ───────────────  */

function buildActivityPanel() {
  const panel = document.createElement('div');
  panel.style.display = 'none';

  /* Account deletion requests. Three endpoints, no screen until
     now — so a customer asking to be deleted was filed nowhere
     anybody could see. */
  const deletions = document.createElement('div');
  deletions.className = 'admin-panel';
  deletions.innerHTML =
    '<div class="admin-panel-head">'
      + '<div>'
        + '<div class="admin-panel-title">Account deletion requests</div>'
        + '<div class="admin-subtitle">Raised by customers from their profile page</div>'
      + '</div>'
    + '</div>'
    + '<div class="table-scroll"><table class="admin-table">'
      + '<thead><tr>'
        + '<th scope="col">Customer</th><th scope="col">Reason</th>'
        + '<th scope="col">Requested</th>'
        + '<th scope="col" class="col-actions"><span class="sr-only">Actions</span></th>'
      + '</tr></thead><tbody id="deletion-body"></tbody>'
    + '</table></div>';
  panel.appendChild(deletions);

  const audit = document.createElement('div');
  audit.className = 'admin-panel';
  audit.style.marginTop = '20px';
  audit.innerHTML =
    '<div class="admin-panel-head">'
      + '<div>'
        + '<div class="admin-panel-title">Audit log</div>'
        + '<div class="admin-subtitle">Every write the console makes, oldest last</div>'
      + '</div>'
    + '</div>'
    + '<div class="table-scroll"><table class="admin-table">'
      + '<thead><tr>'
        + '<th scope="col">When</th><th scope="col">Who</th>'
        + '<th scope="col">Action</th><th scope="col">Target</th><th scope="col">Detail</th>'
      + '</tr></thead><tbody id="audit-body"></tbody>'
    + '</table></div>'
    + '<div class="pagination" id="audit-pager" style="padding:14px;"></div>';
  panel.appendChild(audit);

  return panel;
}

async function loadDeletions() {
  const tbody = $('#deletion-body');
  if (!tbody) return;

  const response = await Fetcher.get({ endpoint: '/v1/admin/deletion-requests', showError: false });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load the deletion queue" });
    return;
  }

  const items = response.data.requests || [];
  const rows = items.map((request) => {
    const tr = document.createElement('tr');
    tr.appendChild(cell(stacked(request.name || '—', request.email || '')));
    tr.appendChild(cell(stacked(request.reason || '—', request.details || '', 'cell-strong')));
    tr.appendChild(cell(fullDate(request.requested_at), 'nowrap'));

    const actions = document.createElement('div');
    actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
    // Both routes are keyed on the *user* id — there is no separate
    // request id on the wire.
    const id = request.user_id;

    actions.appendChild(rowButton('Dismiss', async () => {
      const response2 = await Fetcher.post({
        endpoint: '/v1/admin/deletion-requests/' + id + '/dismiss',
        body: {},
      });
      if (!response2.ok) return;
      toast.success('Request dismissed');
      loadDeletions();
    }, { needs: 'account_deletion' }));

    actions.appendChild(rowButton('Delete account', async () => {
      const confirmed = await confirmModal({
        title: 'Delete this account?',
        message: 'The customer record is removed. Their past orders stay, because '
          + 'an order keeps its own copy of everything it needs. This cannot be undone.',
        confirmLabel: 'Delete account',
      });
      if (!confirmed) return;

      const response2 = await Fetcher.post({
        endpoint: '/v1/admin/deletion-requests/' + id + '/delete',
        body: {},
      });
      if (!response2.ok) return;
      toast.success('Account deleted');
      loadDeletions();
    }, { danger: true, needs: 'account_deletion' }));

    tr.appendChild(cell(actions, 'col-actions'));
    return tr;
  });

  fillTable(tbody, rows, { empty: 'Nobody has asked to be deleted' });
}

async function loadAudit() {
  const tbody = $('#audit-body');
  if (!tbody) return;
  tbody.dataset.loaded = '1';

  const response = await Fetcher.get({
    endpoint: '/v1/admin/settings/audit-log/list',
    query: { page: auditPage, limit: 25 },
  });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load the audit log" });
    return;
  }

  const { items, total, limit, page } = response.data;
  const rows = (items || []).map((entry) => {
    const tr = document.createElement('tr');
    tr.appendChild(cell(dateTime(entry.created_at), 'nowrap'));
    tr.appendChild(cell(entry.admin_name || '—'));
    /* `action` is a dotted key — "order.status_changed". Spelled out
       here; the raw key stays in the title so it is still greppable. */
    const action = cell(spaced((entry.action || '').replace(/[._]/g, ' ')));
    action.title = entry.action || '';
    tr.appendChild(action);

    /* The model calls these `target_collection` / `target_id`. The id is
       a uuid, so only its leading block is shown — enough to match two
       entries against each other, with the whole thing on hover. */
    const target = cell(entry.target_collection
      ? entry.target_collection + (entry.target_id ? ' · ' + String(entry.target_id).slice(0, 8) : '')
      : '—');
    if (entry.target_id) target.title = entry.target_id;
    tr.appendChild(target);

    tr.appendChild(cell(entry.changes || ''));
    return tr;
  });

  fillTable(tbody, rows, { empty: 'Nothing logged yet' });
  paintPager($('#audit-pager'), { total, limit, page }, (next) => { auditPage = next; loadAudit(); });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function loadStore() {
  const response = await Fetcher.get({ endpoint: '/v1/admin/settings/store' });
  if (!response.ok) return;
  store = response.data;
  paintStore();
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  panels = $$('.admin-panel');
  wireTheme();

  /* The last panel — Notifications — has nothing behind it: no
     handler sends staff email, and nothing writes a notification
     row at all. It is replaced by the two panels that do have
     endpoints and had no screen. */
  const notifications = panels[4];
  const activity = buildActivityPanel();
  if (notifications) notifications.replaceWith(activity);
  /* The Shipping tab holds three panels now, so index 2 becomes a
     wrapper — `showPanel` only toggles `display` on whatever is in
     the array, so one container works as well as one panel. */
  const zonesPanel = panels[2];
  const delivery = buildPanel('Delivery methods', 'Add method', () => editMethod(null),
    'What checkout charges for shipping. A shopper picks one of these at the delivery step.');
  const packaging = buildPanel('Packaging options', 'Add option', () => editPackaging(null),
    'Optional extras at checkout — gift wrapping and the like. “No packaging” is always offered.');
  delivery.body.id = 'delivery-body';
  packaging.body.id = 'packaging-body';

  const shipping = document.createElement('div');
  zonesPanel.replaceWith(shipping);
  shipping.append(delivery.panel, packaging.panel, zonesPanel);
  // The zones panel ships hidden like every panel but the first; the
  // wrapper's display decides now, so the panel itself must not.
  zonesPanel.style.display = '';

  panels = [panels[0], panels[1], shipping, panels[3], activity];

  rebuildPayments(panels[1]);

  const tabs = $$('.admin-tabs .admin-tab');
  tabs.forEach((tab, index) => {
    tab.textContent = TABS[index] || tab.textContent;
    tab.addEventListener('click', () => showPanel(index));
    tab.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); showPanel(index); }
    });
  });

  const [discard, save] = $$('.admin-head-actions button');
  if (discard) discard.addEventListener('click', () => location.reload());
  if (save) {
    save.addEventListener('click', () => {
      // Only the Store panel has a form to save; everything else
      // writes as you go.
      showPanel(0);
      saveStore(save);
    });
  }

  const addZone = $$('.admin-panel-link').find((link) => link.textContent.trim() === 'Add zone');
  if (addZone) addZone.addEventListener('click', () => editZone(null));

  const inviteButton = $$('.admin-panel-link')
    .find((link) => /invite/i.test(link.textContent));
  if (inviteButton) inviteButton.addEventListener('click', inviteStaff);

  await loadStore();
  addStoreExtras(panels[0]);
  loadFulfilment();
  loadZones();
  loadStaff();
  loadDeletions();

  showPanel(0);
})();
