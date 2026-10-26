/* ═══════════════════════════════════════════════════════════════
   Bambee Web — brands.js

   The brand list, against `/v1/admin/brand/*`. A brand is a name
   and an optional logo; products pick one in their edit form, and
   the storefront shows it on the product page and filters by it.

   One list, not paged: the endpoint returns every brand, each with
   its live product count, which is also what decides whether Delete
   is offered — the server refuses a brand still on a product.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, fullDate, cell, stacked, fillTable, debounce, rowButton,
  mountAdminChrome, openModal, confirmModal, field, textInput, uploadImage,
} from './admin-core.js';
import { paintImage, clearImage } from './catalog.js';

let all = [];
let query = '';
let tbody = null;

const MAX_LOGO_BYTES = 8 * 1024 * 1024;

/* ── Logo ─────────────────────────────────────────────────────
   Uploaded the moment a file is picked (as a BrandLogo), saved onto
   the brand only with the form — the same split the category image
   uses. `value()` is the id to save, or null once removed.        */
function logoPicker(initialId) {
  let imageId = initialId || null;

  const el = document.createElement('div');
  el.className = 'cat-image-picker';

  const preview = document.createElement('div');
  preview.className = 'cat-image-preview brand-logo-preview';
  preview.setAttribute('role', 'button');
  preview.tabIndex = 0;
  preview.setAttribute('aria-label', 'Choose a logo');

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
    if (imageId) {
      preview.textContent = '';
      preview.classList.add('has-image');
      paintImage(preview, imageId, { property: '--img' });
    } else {
      clearImage(preview);
      preview.style.removeProperty('--img');
      preview.classList.remove('has-image');
      preview.textContent = 'No logo';
    }
    choose.textContent = imageId ? 'Replace' : 'Upload logo';
    remove.style.display = imageId ? '' : 'none';
  };

  const open = () => input.click();
  choose.addEventListener('click', open);
  preview.addEventListener('click', open);
  preview.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); }
  });
  remove.addEventListener('click', () => { imageId = null; paint(); });

  input.addEventListener('change', async () => {
    const file = input.files && input.files[0];
    input.value = '';
    if (!file) return;
    if (!file.type.startsWith('image/')) { toast.error('That file isn’t an image.'); return; }
    if (file.size > MAX_LOGO_BYTES) { toast.error('That image is over 8 MB — please pick a smaller one.'); return; }

    choose.disabled = true;
    choose.textContent = 'Uploading…';
    const id = await uploadImage(file, 'BrandLogo');
    choose.disabled = false;
    if (id) imageId = id;
    paint();
  });

  el.append(preview, actions, input);
  paint();
  return { el, value: () => imageId };
}

/* ── Add / edit ───────────────────────────────────────────────
   One form for both. `brand` is null for a new one. Resolves with
   the saved brand, or null if the dialog was dismissed — the
   product form's "+ Add brand" waits on it to select the new one. */
export function brandForm(brand) {
  return new Promise((resolve) => {
    let saved = null;
    const form = document.createElement('div');

    const name = textInput({ value: brand ? brand.name : '', placeholder: 'e.g. The Ordinary', maxLength: 80 });
    form.appendChild(field('Name', name, 'Shown on the product page, and in the shop’s brand filter.'));

    const logo = logoPicker(brand ? brand.logo_image_id : null);
    form.appendChild(field('Logo', logo.el, 'Optional.'));

    const error = document.createElement('div');
    error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
    form.appendChild(error);

    const modal = openModal({
      title: brand ? 'Edit ' + brand.name : 'Add a brand',
      body: form,
      width: '480px',
      onClose: () => resolve(saved),
      actions: [
        { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
        {
          label: brand ? 'Save' : 'Add brand',
          className: 'btn-primary',
          onClick: async (close, button) => {
            if (!name.value.trim()) {
              error.textContent = 'The brand needs a name.';
              error.style.display = '';
              return;
            }
            error.style.display = 'none';

            button.disabled = true;
            const response = brand
              ? await Fetcher.patch({
                endpoint: '/v1/admin/brand/' + brand.uuid,
                // '' removes the logo; an absent field leaves it.
                body: { name: name.value.trim(), logo_image_id: logo.value() || '' },
                showError: false,
              })
              : await Fetcher.post({
                endpoint: '/v1/admin/brand',
                body: { name: name.value.trim(), logo_image_id: logo.value() },
                showError: false,
              });
            button.disabled = false;

            if (!response.ok) {
              error.textContent = response.error || 'That brand couldn’t be saved.';
              error.style.display = '';
              return;
            }

            saved = response.data;
            toast.success(brand ? 'Brand updated' : 'Brand added');
            close();
          },
        },
      ],
    });

    setTimeout(() => name.focus(), 0);
    return modal;
  });
}

async function remove(brand) {
  const confirmed = await confirmModal({
    title: 'Delete ' + brand.name + '?',
    message: 'It isn’t on any product, so nothing in the shop changes.',
    confirmLabel: 'Delete brand',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/brand/' + brand.uuid });
  if (!response.ok) return;
  toast.success('Brand deleted');
  load();
}

/* ── Table ────────────────────────────────────────────────────  */
function brandRow(brand) {
  const tr = document.createElement('tr');

  const logo = document.createElement('div');
  logo.className = 'brand-logo';
  if (brand.logo_image_id) paintImage(logo, brand.logo_image_id, { property: '--img' });
  else logo.textContent = brand.name.charAt(0).toUpperCase();
  tr.appendChild(cell(logo));

  tr.appendChild(cell(stacked(brand.name, brand.slug)));

  const count = brand.product_count || 0;
  const products = document.createElement('a');
  products.textContent = count.toLocaleString();
  products.href = '/admin/products';
  products.className = count ? '' : 'cell-sub';
  tr.appendChild(cell(count ? products : '0', 'num'));

  tr.appendChild(cell(brand.created_at ? fullDate(brand.created_at) : '—'));

  const actions = document.createElement('td');
  actions.className = 'col-actions';
  actions.appendChild(rowButton('Edit', async () => { if (await brandForm(brand)) load(); }, { needs: 'catalogue' }));
  /* The server refuses a brand that is still on a product; saying so
     here beats a button that only ever answers with an error. */
  actions.appendChild(count
    ? rowButton('Delete', () => toast.error(
      count + (count === 1 ? ' product uses' : ' products use') + ' this brand. Change their brand first.'),
    { danger: true, needs: 'catalogue', title: 'In use by ' + count + (count === 1 ? ' product' : ' products') })
    : rowButton('Delete', () => remove(brand), { danger: true, needs: 'catalogue' }));
  tr.appendChild(actions);

  return tr;
}

function paint() {
  const needle = query.toLowerCase();
  const rows = all.filter((b) => !needle || b.name.toLowerCase().includes(needle) || b.slug.includes(needle));

  fillTable(tbody, rows.map(brandRow), {
    empty: needle ? 'No brands match “' + query + '”' : 'No brands yet. Add one to start tagging products with it.',
  });

  const subtitle = $('.admin-subtitle');
  if (subtitle) {
    const used = all.filter((b) => b.product_count).length;
    subtitle.textContent = all.length
      ? all.length + (all.length === 1 ? ' brand' : ' brands') + ' · ' + used + ' on products'
      : 'Brands appear on product pages and in the shop’s brand filter';
  }
}

async function load() {
  const response = await Fetcher.get({ endpoint: '/v1/admin/brand/list' });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load brands" });
    return;
  }
  all = response.data || [];
  paint();
}

/* ── Boot ─────────────────────────────────────────────────────
   Only on the Brands page: the product form imports brandForm()
   from here and must not have this run against its own markup. */
(async function init() {
  tbody = $('#brand-body');
  if (!tbody) return;

  const admin = await mountAdminChrome();
  if (!admin) return;

  const add = $('#brand-add');
  if (add) add.addEventListener('click', async () => { if (await brandForm(null)) load(); });

  const search = $('.admin-search input');
  if (search) {
    search.addEventListener('input', debounce(() => {
      query = search.value.trim();
      paint();
    }, 200));
  }

  load();
})();
