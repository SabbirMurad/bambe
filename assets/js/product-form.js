/* ═══════════════════════════════════════════════════════════════
   Bambe Web — product-form.js

   Create and edit a product with its variants. One page, two
   modes, told apart by `?id=` in the URL.

   The shapes here are the model's, not the wireframe's:

   - **A variant owns its image**, and `image_id` is required, not
     optional (model/product.rs says so at length: the gallery, the
     card thumbnail and every order line are all built from it). So
     each SKU row has its own uploader and a row without a picture
     cannot be saved.
   - **Create takes its variants inline**; editing writes them one
     at a time through `/product/{id}/variant`. That is the shape
     of the two endpoints, so it is the shape of the save.
   - **Brand is a picker, not a text box.** `/v1/product/brand/list`
     reads brands and no endpoint creates one, so typing a new
     brand name here could only ever be discarded.
   - **"Track inventory" and "Continue selling at zero" are gone.**
     Nothing stores either. Stock is always tracked and zero always
     stops the sale; a switch implying otherwise would be a lie.

   Prices are typed in dollars and stored in cents, converted in
   one place at the boundary.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, mountAdminChrome, uploadImage, confirmModal,
} from './admin-core.js';

let mode = 'create';       // or 'edit'
let productId = null;
let original = null;       // { product, variants } as loaded, in edit mode
let categories = [];
let brands = [];
let tags = [];
let rowSeq = 0;

/* Each SKU row on screen, in document order. `uuid` is set only for
   rows that already exist on the server. */
let skus = [];

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

function imageCell(sku) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'position:relative;width:44px;height:44px;';

  const preview = document.createElement('div');
  preview.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
    + 'background:var(--input-bg);border:1px dashed var(--border);cursor:pointer;'
    + 'display:flex;align-items:center;justify-content:center;color:var(--faint);font-size:18px;';
  preview.setAttribute('role', 'button');
  preview.tabIndex = 0;
  preview.title = 'Upload an image for this SKU';

  const paint = () => {
    if (sku.image_id) {
      preview.textContent = '';
      preview.style.border = '1px solid var(--border)';
      preview.style.background = "url('/image/webp/" + sku.image_id + "') center / cover no-repeat";
    } else {
      preview.textContent = '+';
      preview.style.border = '1px dashed var(--border)';
      preview.style.background = 'var(--input-bg)';
    }
  };
  paint();

  const picker = document.createElement('input');
  picker.type = 'file';
  picker.accept = 'image/*';
  picker.style.display = 'none';

  picker.addEventListener('change', async () => {
    const file = picker.files && picker.files[0];
    if (!file) return;

    preview.textContent = '…';
    preview.style.background = 'var(--input-bg)';

    const id = await uploadImage(file, 'ProductImage');
    if (!id) { paint(); return; }

    sku.image_id = id;
    paint();

    /* In edit mode an existing SKU's picture is a field like any
       other — saved with the row, not on upload — so nothing is
       written here. A new row has nowhere to write it to yet. */
  });

  const open = () => picker.click();
  preview.addEventListener('click', open);
  preview.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });

  wrap.appendChild(preview);
  wrap.appendChild(picker);
  return wrap;
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

  bind(input(sku.size, 'M'), 'size');

  /* Colour is a name plus a swatch — the storefront's variant picker
     draws the swatch from `color_hex` and labels it with `color`. */
  const colourTd = document.createElement('td');
  const colourWrap = document.createElement('div');
  colourWrap.style.cssText = 'display:flex;gap:6px;align-items:center;';
  const colourName = input(sku.color, 'Navy');
  colourName.style.flex = '1';
  colourName.addEventListener('input', () => { sku.color = colourName.value; });
  const colourHex = document.createElement('input');
  colourHex.type = 'color';
  colourHex.className = 'color-swatch';
  colourHex.value = sku.color_hex || '#1A1A18';
  colourHex.setAttribute('aria-label', 'Colour swatch');
  colourHex.addEventListener('input', () => { sku.color_hex = colourHex.value; });
  colourWrap.appendChild(colourName);
  colourWrap.appendChild(colourHex);
  colourTd.appendChild(colourWrap);
  tr.appendChild(colourTd);

  const price = input(dollars(sku.price), '0.00');
  price.addEventListener('input', () => { sku.price = cents(price.value); });
  const priceTd = document.createElement('td');
  priceTd.appendChild(price);
  tr.appendChild(priceTd);

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

function paintSkus() {
  const tbody = $('#sku-body');
  if (!tbody) return;

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
    size: '',
    color: '',
    color_hex: '#1A1A18',
    image_id: null,
    price: 0,
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

  skus.forEach((sku, index) => {
    const label = 'SKU ' + (index + 1) + (sku.sku ? ' (' + sku.sku + ')' : '');
    if (!sku.sku.trim()) out.push(label + ' needs a code.');
    if (!sku.image_id) out.push(label + ' needs an image.');
    if (!(sku.price > 0)) out.push(label + ' needs a price.');
  });

  const codes = skus.map((s) => s.sku.trim().toLowerCase()).filter(Boolean);
  if (new Set(codes).size !== codes.length) out.push('Two SKUs share the same code.');

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
  };
}

function variantPayload(sku) {
  return {
    sku: sku.sku.trim(),
    size: sku.size.trim() || null,
    color: sku.color.trim() || null,
    color_hex: sku.color_hex || null,
    image_id: sku.image_id,
    price: sku.price,
    compare_at_price: null,
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

  /* The wireframe had a free-text brand box. Nothing creates a brand
     — `/v1/product/brand/list` is read-only and there is no admin
     endpoint — so anything typed would have been dropped on save. */
  const brandInput = $('#p-brand');
  if (brandInput) {
    const select = document.createElement('select');
    select.className = 'form-select';
    select.id = 'p-brand';

    const none = document.createElement('option');
    none.value = '';
    none.textContent = brands.length ? '— No brand —' : '— No brands set up —';
    select.appendChild(none);

    brands.forEach((brand) => {
      const option = document.createElement('option');
      option.value = brand.uuid;
      option.textContent = brand.name;
      select.appendChild(option);
    });

    brandInput.replaceWith(select);

    const hint = select.parentElement.querySelector('.form-hint');
    if (!hint) {
      const note = document.createElement('div');
      note.className = 'form-hint';
      note.textContent = brands.length
        ? 'Brands are shared across the catalogue.'
        : 'No brands exist yet, and there is no endpoint to create one from here.';
      select.after(note);
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

  skus = (variants || []).map((variant) => ({
    key: 'row-' + (++rowSeq),
    uuid: variant.uuid,
    sku: variant.sku || '',
    size: variant.size || '',
    color: variant.color || '',
    color_hex: variant.color_hex || '#1A1A18',
    image_id: variant.image_id,
    price: variant.price,
    stock: variant.stock,
  }));
  paintSkus();

  // Page furniture: this is an edit, and it should say so.
  document.title = 'Edit ' + product.title + ' — Bambe Admin';
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
