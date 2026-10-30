/* ═══════════════════════════════════════════════════════════════
   Bambee Web — product-form.js

   Create and edit a product with its variants. One page, two
   modes, told apart by `?id=` in the URL.

   The shapes here are the model's, not the wireframe's:

   - **A variant owns its photos**: a main one, `image_id`, which is
     required (the card thumbnail and every order line are built from
     it), and up to nine more in `extra_image_ids` for the product
     page's gallery. Each SKU row has its own photo manager, and a
     row without a photo cannot be saved.
   - **Create takes its variants inline**; editing writes them one
     at a time through `/product/{id}/variant`. That is the shape
     of the two endpoints, so it is the shape of the save.
   - **Brand is a picker, not a text box.** Brands live on Admin →
     Brands; "+ Add brand" opens that page's form (brands.js) so a
     new one can be made without leaving the product.
   - **"Track inventory" and "Continue selling at zero" are gone.**
     Nothing stores either. Stock is always tracked and zero always
     stops the sale; a switch implying otherwise would be a lie.

   Prices are typed in dollars and stored in cents, converted in
   one place at the boundary.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, mountAdminChrome, uploadImage, confirmModal, openModal,
} from './admin-core.js';
import { brandForm } from './brands.js';
import { paintImage } from './catalog.js';

let mode = 'create';       // or 'edit'
let productId = null;
let original = null;       // { product, variants } as loaded, in edit mode
let categories = [];
let brands = [];
let tags = [];
let rowSeq = 0;

/* Each SKU row on screen, in document order. `uuid` is set only for
   rows that already exist on the server. `values` holds the row's
   value for each option, in the same order as `options`. */
let skus = [];

/* What the SKUs differ by, named by the admin: { key, name, swatch }.
   At most MAX_OPTIONS, at most one shown as colour swatches. */
let options = [];
let optionSeq = 0;
const MAX_OPTIONS = 3;
const OPTION_EXAMPLES = ['Shade', 'Volume', 'Skin type'];
const VALUE_EXAMPLES = { 0: 'Ruby Woo', 1: '50 ml', 2: 'Oily' };

const cents = (dollars) => Math.round(Number(dollars || 0) * 100);
const dollars = (c) => (c === null || c === undefined ? '' : (c / 100).toFixed(2));

/* ── Tag chips ────────────────────────────────────────────────  */

function paintTags() {
  const field = $('[data-chips="tag"]');
  if (!field) return;
  const input = field.querySelector('input');

  $$('.chip', field).forEach((chip) => chip.remove());

  tags.forEach((value, index) => {
    const chip = document.createElement('span');
    chip.className = 'chip';
    chip.textContent = value;

    const remove = document.createElement('span');
    remove.className = 'chip-x';
    remove.setAttribute('role', 'button');
    remove.tabIndex = 0;
    remove.setAttribute('aria-label', 'Remove ' + value);
    remove.textContent = '×';
    const drop = () => { tags.splice(index, 1); paintTags(); };
    remove.addEventListener('click', drop);
    remove.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); drop(); }
    });

    chip.appendChild(remove);
    field.insertBefore(chip, input);
  });
}

function wireTags() {
  const input = $('#opt-tag');
  if (!input) return;

  const commit = () => {
    const value = input.value.trim().replace(/,$/, '');
    if (value && !tags.includes(value)) tags.push(value);
    input.value = '';
    paintTags();
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ',') { e.preventDefault(); commit(); }
    if (e.key === 'Backspace' && !input.value && tags.length) {
      tags.pop();
      paintTags();
    }
  });
  input.addEventListener('blur', commit);
}

/* ── SKU rows ─────────────────────────────────────────────────  */

/* ── SKU photos ───────────────────────────────────────────────
   `sku.images` is the row's photos in display order; the first is
   the main one (`image_id` on save), which stands for the SKU on its
   card, cart line and order. The cell shows that one with a count of
   the rest, and opens the photo manager below on click.          */
const MAX_PHOTOS = 10;

function imageCell(sku) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'position:relative;width:44px;height:44px;';

  const preview = document.createElement('div');
  preview.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
    + 'background:var(--input-bg);border:1px dashed var(--border);cursor:pointer;'
    + 'display:flex;align-items:center;justify-content:center;color:var(--faint);font-size:18px;';
  preview.setAttribute('role', 'button');
  preview.tabIndex = 0;

  const count = document.createElement('span');
  count.className = 'sku-photo-count';

  const paint = () => {
    const main = sku.images[0];
    if (main) {
      preview.textContent = '';
      preview.style.border = '1px solid var(--border)';
      paintImage(preview, main);
    } else {
      preview.textContent = '+';
      preview.style.border = '1px dashed var(--border)';
      preview.style.background = 'var(--input-bg)';
    }
    const extra = sku.images.length - 1;
    count.textContent = extra > 0 ? '+' + extra : '';
    count.style.display = extra > 0 ? '' : 'none';
    preview.title = sku.images.length
      ? sku.images.length + (sku.images.length === 1 ? ' photo' : ' photos') + ' — click to manage'
      : 'Add photos for this SKU';
  };
  paint();

  const open = () => managePhotos(sku, paint);
  preview.addEventListener('click', open);
  preview.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });

  wrap.append(preview, count);
  return wrap;
}

/* The photo manager for one SKU: every photo in order, the first
   marked Main. Add several at once; move one earlier or later, or
   make it the main photo; remove one. Changes are kept on the row
   and saved with the product, like every other field. */
function managePhotos(sku, onChange) {
  const body = document.createElement('div');
  const grid = document.createElement('div');
  grid.className = 'sku-photo-grid';
  body.appendChild(grid);

  const picker = document.createElement('input');
  picker.type = 'file';
  picker.accept = 'image/*';
  picker.multiple = true;
  picker.hidden = true;
  body.appendChild(picker);

  const note = document.createElement('div');
  note.className = 'form-hint';
  body.appendChild(note);

  const move = (from, to) => {
    if (to < 0 || to >= sku.images.length) return;
    const [id] = sku.images.splice(from, 1);
    sku.images.splice(to, 0, id);
    paint();
  };

  const button = (label, title, onClick) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'sku-photo-btn';
    b.textContent = label;
    b.title = title;
    b.setAttribute('aria-label', title);
    b.addEventListener('click', onClick);
    return b;
  };

  function paint() {
    grid.innerHTML = '';
    sku.images.forEach((id, index) => {
      const tile = document.createElement('div');
      tile.className = 'sku-photo';
      const img = document.createElement('div');
      img.className = 'sku-photo-img';
      paintImage(img, id);
      tile.appendChild(img);

      if (index === 0) {
        const main = document.createElement('span');
        main.className = 'sku-photo-main';
        main.textContent = 'Main';
        tile.appendChild(main);
      }

      const tools = document.createElement('div');
      tools.className = 'sku-photo-tools';
      if (index > 0) {
        tools.appendChild(button('★', 'Make this the main photo', () => move(index, 0)));
        tools.appendChild(button('←', 'Move earlier', () => move(index, index - 1)));
      }
      if (index < sku.images.length - 1) tools.appendChild(button('→', 'Move later', () => move(index, index + 1)));
      tools.appendChild(button('✕', 'Remove this photo', () => { sku.images.splice(index, 1); paint(); }));
      tile.appendChild(tools);
      grid.appendChild(tile);
    });

    if (sku.images.length < MAX_PHOTOS) {
      const add = document.createElement('div');
      add.className = 'sku-photo sku-photo-add';
      add.setAttribute('role', 'button');
      add.tabIndex = 0;
      add.textContent = '+ Add photos';
      add.addEventListener('click', () => picker.click());
      add.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); picker.click(); }
      });
      grid.appendChild(add);
    }

    note.textContent = 'The main photo stands for this SKU on its card, in the cart and on orders. '
      + 'The product page shows all of them, in this order, when the SKU is selected. Up to '
      + MAX_PHOTOS + '.';
    onChange();
  }

  picker.addEventListener('change', async () => {
    const files = [...(picker.files || [])];
    picker.value = '';
    const room = MAX_PHOTOS - sku.images.length;
    if (files.length > room) toast.error('Only ' + room + ' more ' + (room === 1 ? 'photo fits' : 'photos fit') + ' on this SKU.');
    for (const file of files.slice(0, room)) {
      if (!file.type.startsWith('image/')) { toast.error(file.name + ' isn’t an image.'); continue; }
      note.textContent = 'Uploading ' + file.name + '…';
      // One at a time, so they land in the order they were picked.
      const id = await uploadImage(file, 'ProductImage');
      if (id) sku.images.push(id);
      paint();
    }
  });

  openModal({
    title: 'Photos' + (sku.sku.trim() ? ' — ' + sku.sku.trim() : ''),
    body,
    width: '620px',
    actions: [{ label: 'Done', className: 'btn-primary', onClick: (close) => close() }],
  });
  paint();
}

function input(value, placeholder, width) {
  const el = document.createElement('input');
  el.className = 'form-input';
  el.type = 'text';
  el.value = value === null || value === undefined ? '' : String(value);
  el.placeholder = placeholder || '';
  el.style.cssText = 'height:34px;font-size:13px;padding:0 10px;'
    + (width ? 'width:' + width + ';' : '');
  return el;
}

/* ── Options ──────────────────────────────────────────────────
   The panel above the SKU table. Each option is a name the admin
   types and a "colour swatches" switch; adding, removing or
   renaming one reshapes the SKU table to match. Values follow their
   option by position, so removing an option drops that column's
   values from every row.                                         */
function paintOptions() {
  const host = $('#option-list');
  if (!host) return;
  host.innerHTML = '';

  if (!options.length) {
    const empty = document.createElement('div');
    empty.className = 'form-hint';
    empty.style.margin = '0';
    empty.textContent = 'No options: this product is sold in one form, with a single SKU.';
    host.appendChild(empty);
  }

  options.forEach((option, index) => {
    const row = document.createElement('div');
    row.className = 'option-row';

    const name = input(option.name, OPTION_EXAMPLES[index] || 'Option name');
    name.setAttribute('aria-label', 'Option ' + (index + 1) + ' name');
    name.maxLength = 60;
    // Only the header needs to follow the typing; repainting the rows
    // here would take focus away mid-word.
    name.addEventListener('input', () => { option.name = name.value; paintSkuHead(); });

    const swatch = document.createElement('label');
    swatch.className = 'option-swatch';
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = !!option.swatch;
    box.addEventListener('change', () => {
      // One swatch row per product: ticking this one unticks the rest.
      options.forEach((o) => { o.swatch = o === option ? box.checked : false; });
      paintOptions();
      paintSkus();
    });
    swatch.append(box, document.createTextNode('Show as colour swatches'));

    const remove = document.createElement('button');
    remove.type = 'button';
    remove.className = 'btn btn-ghost btn-sm';
    remove.textContent = 'Remove';
    remove.addEventListener('click', () => {
      options.splice(index, 1);
      skus.forEach((sku) => sku.values.splice(index, 1));
      paintOptions();
      paintSkus();
    });

    row.append(name, swatch, remove);
    host.appendChild(row);
  });

  const add = $('#add-option');
  if (add) {
    add.disabled = options.length >= MAX_OPTIONS;
    add.title = add.disabled ? 'A product can have up to ' + MAX_OPTIONS + ' options' : '';
  }
}

function addOption() {
  if (options.length >= MAX_OPTIONS) return;
  options.push({ key: 'opt-' + (++optionSeq), name: '', swatch: false });
  skus.forEach((sku) => sku.values.push(''));
  paintOptions();
  paintSkus();
  const inputs = $$('#option-list .option-row input.form-input');
  if (inputs.length) inputs[inputs.length - 1].focus();
}

function skuRow(sku) {
  const tr = document.createElement('tr');
  tr.dataset.row = sku.key;

  const imageTd = document.createElement('td');
  imageTd.appendChild(imageCell(sku));
  tr.appendChild(imageTd);

  const bind = (el, key, transform) => {
    el.addEventListener('input', () => {
      sku[key] = transform ? transform(el.value) : el.value;
    });
    const td = document.createElement('td');
    td.appendChild(el);
    tr.appendChild(td);
    return el;
  };

  /* One cell per option. The swatch option also takes the colour the
     product page draws its dot in (`color_hex`). */
  options.forEach((option, index) => {
    const td = document.createElement('td');
    const value = input(sku.values[index], VALUE_EXAMPLES[index] || '');
    value.setAttribute('aria-label', option.name || 'Option ' + (index + 1));
    value.addEventListener('input', () => { sku.values[index] = value.value; });
    if (option.swatch) {
      const wrap = document.createElement('div');
      wrap.style.cssText = 'display:flex;gap:6px;align-items:center;';
      value.style.flex = '1';
      const hex = document.createElement('input');
      hex.type = 'color';
      hex.className = 'color-swatch';
      hex.value = sku.color_hex || '#1A1A18';
      hex.setAttribute('aria-label', (option.name || 'Swatch') + ' colour');
      hex.addEventListener('input', () => { sku.color_hex = hex.value; });
      wrap.append(value, hex);
      td.appendChild(wrap);
    } else {
      td.appendChild(value);
    }
    tr.appendChild(td);
  });

  const price = input(dollars(sku.price), '0.00');
  price.addEventListener('input', () => { sku.price = cents(price.value); });
  const priceTd = document.createElement('td');
  priceTd.appendChild(price);
  tr.appendChild(priceTd);

  /* The "was" price. Filled in, the SKU is on sale: the storefront
     strikes it through beside the price and badges the discount.
     Empty is no sale, so clearing it ends one. */
  const was = input(dollars(sku.compare_at_price), '—');
  was.setAttribute('aria-label', 'Was price (leave empty for no sale)');
  was.addEventListener('input', () => {
    sku.compare_at_price = was.value.trim() ? cents(was.value) : null;
  });
  const wasTd = document.createElement('td');
  wasTd.appendChild(was);
  tr.appendChild(wasTd);

  const stock = input(sku.stock, '0');
  stock.addEventListener('input', () => { sku.stock = Number(stock.value) || 0; });
  const stockTd = document.createElement('td');
  stockTd.appendChild(stock);
  tr.appendChild(stockTd);

  bind(input(sku.sku, 'BM-APP-1043'), 'sku');

  const actionsTd = document.createElement('td');
  actionsTd.className = 'col-actions';
  const remove = document.createElement('span');
  remove.className = 'row-btn danger';
  remove.tabIndex = 0;
  remove.textContent = 'Remove';
  const drop = async () => {
    if (sku.uuid) {
      const confirmed = await confirmModal({
        title: 'Delete this SKU?',
        message: (sku.sku || 'This variant') + ' will be removed from the product. '
          + 'Orders that already include it keep their own copy.',
        confirmLabel: 'Delete SKU',
      });
      if (!confirmed) return;

      const response = await Fetcher.delete({ endpoint: '/v1/admin/product/variant/' + sku.uuid });
      if (!response.ok) return;
      toast.success('SKU deleted');
    }
    skus = skus.filter((s) => s.key !== sku.key);
    paintSkus();
  };
  remove.addEventListener('click', drop);
  remove.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); drop(); }
  });
  actionsTd.appendChild(remove);
  tr.appendChild(actionsTd);

  return tr;
}

/* The header follows the options: Image, one column per option, then
   the fixed price / was / stock / code columns. */
function paintSkuHead() {
  const head = $('#sku-head');
  if (!head) return;
  const row = document.createElement('tr');
  const th = (text, width, extra) => {
    const el = document.createElement('th');
    el.scope = 'col';
    if (width) el.style.width = width;
    if (extra) Object.assign(el, extra);
    el.textContent = text;
    row.appendChild(el);
    return el;
  };
  th('Image', '64px');
  options.forEach((option, index) => {
    th(option.name.trim() || 'Option ' + (index + 1), option.swatch ? '200px' : '150px');
  });
  th('Price', '120px');
  th('Was', '120px', { title: 'Fill in to put this SKU on sale' });
  th('Stock', '100px');
  th('SKU code');
  const actions = th('', null, { className: 'col-actions' });
  actions.innerHTML = '<span class="sr-only">Remove</span>';
  head.innerHTML = '';
  head.appendChild(row);
}

function paintSkus() {
  const tbody = $('#sku-body');
  if (!tbody) return;

  paintSkuHead();
  tbody.innerHTML = '';
  skus.forEach((sku) => tbody.appendChild(skuRow(sku)));

  const empty = $('#sku-empty');
  if (empty) empty.style.display = skus.length ? 'none' : '';

  const count = $('#sku-count');
  if (count) count.textContent = String(skus.length);
}

function blankSku() {
  return {
    key: 'row-' + (++rowSeq),
    uuid: null,
    sku: '',
    values: options.map(() => ''),
    color_hex: '#1A1A18',
    images: [],
    price: 0,
    compare_at_price: null,
    stock: 0,
  };
}

/* ── Validation ───────────────────────────────────────────────  */

function problems() {
  const out = [];
  if (!$('#p-title').value.trim()) out.push('The product needs a title.');
  if (!$('#p-desc').value.trim()) out.push('The product needs a description.');
  if (!$('#p-cat').value) out.push('Pick a category.');
  if (!skus.length) out.push('Add at least one SKU.');

  const names = options.map((o) => o.name.trim().toLowerCase());
  if (names.some((n) => !n)) out.push('Every option needs a name, like "Shade" or "Volume".');
  if (new Set(names).size !== names.length) out.push('Two options have the same name.');
  if (!options.length && skus.length > 1) {
    out.push('Add an option, like "Shade" or "Volume", to sell this product in more than one form.');
  }

  skus.forEach((sku, index) => {
    const label = 'SKU ' + (index + 1) + (sku.sku ? ' (' + sku.sku + ')' : '');
    if (!sku.sku.trim()) out.push(label + ' needs a code.');
    if (!sku.images.length) out.push(label + ' needs a photo.');
    if (!(sku.price > 0)) out.push(label + ' needs a price.');
    options.forEach((option, i) => {
      if (!String(sku.values[i] || '').trim()) out.push(label + ' needs a ' + (option.name.trim() || 'value') + '.');
    });
    if (sku.compare_at_price !== null && !(sku.compare_at_price > sku.price)) {
      out.push(label + ': the "was" price must be higher than the price, or empty for no sale.');
    }
  });

  const codes = skus.map((s) => s.sku.trim().toLowerCase()).filter(Boolean);
  if (new Set(codes).size !== codes.length) out.push('Two SKUs share the same code.');

  if (options.length) {
    const combos = skus.map((s) => s.values.map((v) => String(v || '').trim().toLowerCase()).join('\u001f'));
    if (new Set(combos).size !== combos.length) out.push('Two SKUs have the same option values.');
  }

  return out;
}

/* ── Save ─────────────────────────────────────────────────────  */

function payload(status) {
  return {
    title: $('#p-title').value.trim(),
    description: $('#p-desc').value.trim(),
    category_id: $('#p-cat').value,
    brand_id: $('#p-brand').value || null,
    tags,
    status,
    options: options.map((o) => ({ name: o.name.trim(), swatch: !!o.swatch })),
  };
}

function variantPayload(sku) {
  return {
    sku: sku.sku.trim(),
    options: options.map((_, i) => String(sku.values[i] || '').trim()),
    // Only the swatch option draws a colour; without one it means nothing.
    color_hex: options.some((o) => o.swatch) ? (sku.color_hex || null) : null,
    image_id: sku.images[0],
    extra_image_ids: sku.images.slice(1),
    price: sku.price,
    compare_at_price: sku.compare_at_price,
    stock: sku.stock,
  };
}

async function save(status, button) {
  const found = problems();
  if (found.length) {
    toast.error(found[0]);
    return;
  }

  button.disabled = true;

  if (mode === 'create') {
    const response = await Fetcher.post({
      endpoint: '/v1/admin/product',
      body: { ...payload(status), variants: skus.map(variantPayload) },
    });
    button.disabled = false;
    if (!response.ok) return;

    toast.success(status === 'Active' ? 'Product published' : 'Saved as a draft');
    setTimeout(() => { location.href = '/admin/products'; }, 600);
    return;
  }

  /* Edit: the product first, then each variant. A new row is a POST
     to /variant, an existing one a PATCH — there is no endpoint that
     takes the whole set, so this is the shape it has to be. */
  const updated = await Fetcher.patch({
    endpoint: '/v1/admin/product/' + productId,
    body: payload(status),
  });
  if (!updated.ok) { button.disabled = false; return; }

  let failed = 0;
  for (const sku of skus) {
    const response = sku.uuid
      ? await Fetcher.patch({
        endpoint: '/v1/admin/product/variant/' + sku.uuid,
        body: variantPayload(sku),
        showError: false,
      })
      : await Fetcher.post({
        endpoint: '/v1/admin/product/' + productId + '/variant',
        body: variantPayload(sku),
        showError: false,
      });
    if (!response.ok) failed++;
    else if (!sku.uuid && response.data && response.data.uuid) sku.uuid = response.data.uuid;
  }

  button.disabled = false;

  if (failed) {
    toast.error(failed + (failed === 1 ? ' SKU could not be saved' : ' SKUs could not be saved'));
    return;
  }

  toast.success('Product saved');
  setTimeout(() => { location.href = '/admin/products'; }, 600);
}

/* ── Load ─────────────────────────────────────────────────────  */

async function loadReferenceData() {
  const [categoryResponse, brandResponse] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/admin/category/list', showError: false }),
    Fetcher.get({ endpoint: '/v1/product/brand/list', showError: false }),
  ]);

  categories = categoryResponse.ok ? (categoryResponse.data || []) : [];
  brands = brandResponse.ok ? (brandResponse.data || []) : [];

  const categorySelect = $('#p-cat');
  if (categorySelect) {
    categorySelect.innerHTML = '';
    const blank = document.createElement('option');
    blank.value = '';
    blank.textContent = '— Pick a category —';
    categorySelect.appendChild(blank);

    /* Indented two levels, matching the tree, so "Shirts" under
       "Apparel" doesn't read as a top-level category. */
    categories.filter((c) => !c.parent_id).forEach((top) => {
      const option = document.createElement('option');
      option.value = top.uuid;
      option.textContent = top.name;
      categorySelect.appendChild(option);

      categories.filter((c) => c.parent_id === top.uuid).forEach((kid) => {
        const child = document.createElement('option');
        child.value = kid.uuid;
        child.textContent = '   ' + kid.name;
        categorySelect.appendChild(child);
      });
    });
  }

  /* Brands are managed on Admin → Brands; the dropdown lists them,
     and "+ Add brand" creates one without leaving this product. */
  const brandInput = $('#p-brand');
  if (brandInput) {
    const select = document.createElement('select');
    select.className = 'form-select';
    select.id = 'p-brand';

    const none = document.createElement('option');
    none.value = '';
    none.textContent = '— No brand —';
    select.appendChild(none);

    const addOption = (brand) => {
      const option = document.createElement('option');
      option.value = brand.uuid;
      option.textContent = brand.name;
      select.appendChild(option);
      return option;
    };
    brands.forEach(addOption);

    const row = document.createElement('div');
    row.style.cssText = 'display:flex;gap:8px;align-items:center;';
    select.style.flex = '1';
    const add = document.createElement('button');
    add.type = 'button';
    add.className = 'btn btn-outline btn-sm';
    add.textContent = '+ Add brand';
    add.addEventListener('click', async () => {
      const created = await brandForm(null);
      if (!created) return;
      brands.push(created);
      // Back into A–Z order, the order the list endpoint uses.
      const options = [...select.options].slice(1);
      const option = addOption(created);
      const after = options.find((o) => o.textContent.localeCompare(created.name) > 0);
      if (after) select.insertBefore(option, after);
      select.value = created.uuid;
    });

    brandInput.replaceWith(row);
    row.append(select, add);

    const hint = row.parentElement.querySelector('.form-hint');
    if (!hint) {
      const note = document.createElement('div');
      note.className = 'form-hint';
      note.textContent = 'Shown on the product page. Rename or remove brands on Admin → Brands.';
      row.after(note);
    }
  }
}

async function loadProduct() {
  const response = await Fetcher.get({ endpoint: '/v1/admin/product/' + productId });
  if (!response.ok) {
    toast.error("That product couldn't be loaded");
    return;
  }

  original = response.data;
  const { product, variants } = original;

  $('#p-title').value = product.title;
  $('#p-desc').value = product.description;
  $('#p-cat').value = product.category_id;
  const brandSelect = $('#p-brand');
  if (brandSelect) brandSelect.value = product.brand_id || '';

  tags = [...(product.tags || [])];
  paintTags();

  options = (product.options || []).map((o) => ({ key: 'opt-' + (++optionSeq), name: o.name, swatch: !!o.swatch }));
  paintOptions();

  skus = (variants || []).map((variant) => ({
    key: 'row-' + (++rowSeq),
    uuid: variant.uuid,
    sku: variant.sku || '',
    values: options.map((option) => {
      const pair = (variant.options || []).find((p) => p.name === option.name);
      return pair ? pair.value : '';
    }),
    color_hex: variant.color_hex || '#1A1A18',
    images: [variant.image_id, ...(variant.extra_image_ids || [])].filter(Boolean),
    price: variant.price,
    compare_at_price: variant.compare_at_price ?? null,
    stock: variant.stock,
  }));
  paintSkus();

  // Page furniture: this is an edit, and it should say so.
  document.title = 'Edit ' + product.title + ' — Bambee Admin';
  const heading = $('.admin-title');
  if (heading) heading.textContent = 'Edit product';
  const crumb = $('.breadcrumb span:last-child');
  if (crumb) crumb.textContent = product.title;

  const visible = $('.settings-row .toggle');
  if (visible) {
    visible.classList.toggle('on', product.status === 'Active');
    visible.setAttribute('aria-checked', String(product.status === 'Active'));
  }

  const publish = $$('.admin-head-actions button')
    .find((b) => b.classList.contains('btn-primary'));
  if (publish) publish.textContent = product.status === 'Active' ? 'Save changes' : 'Publish';
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  productId = new URLSearchParams(location.search).get('id');
  mode = productId ? 'edit' : 'create';

  wireTags();

  const addOptionButton = $('#add-option');
  if (addOptionButton) addOptionButton.addEventListener('click', addOption);
  paintOptions();
  paintSkuHead();

  const add = $('#add-sku');
  if (add) {
    add.addEventListener('click', () => {
      skus.push(blankSku());
      paintSkus();
      const rows = $$('#sku-body tr');
      const last = rows[rows.length - 1];
      if (last) last.querySelector('input.form-input').focus();
    });
  }

  const clear = $('#clear-skus');
  if (clear) {
    clear.addEventListener('click', async () => {
      if (!skus.length) return;
      const saved = skus.filter((s) => s.uuid);
      const confirmed = await confirmModal({
        title: 'Clear every SKU?',
        message: saved.length
          ? saved.length + (saved.length === 1 ? ' SKU has' : ' SKUs have')
            + ' already been saved and will be deleted from the product.'
          : 'Nothing has been saved yet, so this only clears the form.',
        confirmLabel: 'Clear all',
      });
      if (!confirmed) return;

      for (const sku of saved) {
        await Fetcher.delete({ endpoint: '/v1/admin/product/variant/' + sku.uuid, showError: false });
      }
      skus = [];
      paintSkus();
    });
  }

  const buttons = $$('.admin-head-actions button');
  const draft = buttons.find((b) => b.classList.contains('btn-outline'));
  const publish = buttons.find((b) => b.classList.contains('btn-primary'));
  if (draft) draft.addEventListener('click', () => save('Draft', draft));
  if (publish) publish.addEventListener('click', () => save('Active', publish));

  /* The Status panel had three switches and the model has one field.
     The two that store nothing are removed rather than left to
     imply behaviour that does not exist. */
  const statusRows = $$('.settings-row');
  const visible = statusRows[0] ? statusRows[0].querySelector('.toggle') : null;
  if (visible) {
    visible.dataset.readonly = 'true';
    visible.setAttribute('role', 'switch');
    visible.tabIndex = 0;
    const flip = () => {
      const on = !visible.classList.contains('on');
      visible.classList.toggle('on', on);
      visible.setAttribute('aria-checked', String(on));
      if (publish) publish.textContent = on ? 'Publish' : 'Save as draft';
    };
    visible.addEventListener('click', flip);
    visible.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
    });
  }
  statusRows.slice(1).forEach((row) => row.remove());

  await loadReferenceData();

  if (mode === 'edit') await loadProduct();
  else {
    skus.push(blankSku());
    paintSkus();
  }
})();
