/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-products.js

   The catalogue table: four tabs, search, category and sort
   filters, per-row actions and the bulk bar.

   `GET /v1/admin/product/list` returns the raw product documents,
   which know their `category_id` but not its name — so the
   category list is loaded once and indexed, rather than a lookup
   per row. It now also carries `stock_total`, `variant_count` and
   `low_stock`, added for this screen: `Product` only holds an
   `in_stock` boolean, and a Stock column that can only say yes or
   no is not a Stock column.

   What the wireframe drew and the API cannot do:

   - **Import CSV** — no import endpoint. Removed.
   - **Bulk "Edit category"** — there is no bulk endpoint at all;
     Set active and Delete are loops over the single-item ones,
     which is honest for a page of 20 but would not be for 300, so
     they are capped at the page you can see.
   - **Best selling** as a sort — `product/list` sorts on
     created_at, price_min and in_stock only.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, cell, fillTable, paintPager, paintShowing, debounce,
  rowButton, tag, mountAdminChrome, openModal, confirmModal, field, textInput,
} from './admin-core.js';
import { paintImage } from './catalog.js';

const TABS = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'draft', label: 'Draft' },
  { value: 'out_of_stock', label: 'Out of stock' },
];

const SORTS = [
  { value: 'newest', label: 'Newest first' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'price_desc', label: 'Price: high to low' },
  { value: 'price_asc', label: 'Price: low to high' },
  { value: 'stock_asc', label: 'Out of stock first' },
];

const STATUS_TAG = { Active: 'tag-green', Draft: 'tag-gold', Archived: 'tag-red' };

let state = { tab: 'all', q: '', category: '', sort: 'newest', page: 1, limit: 20 };
let categories = [];
let tbody = null;
let counts = {};
// The largest stock on the current page — the bar scale below.
let ceiling = 1;

const categoryName = (id) => (categories.find((c) => c.uuid === id) || {}).name || '—';

/* ── Rows ─────────────────────────────────────────────────────  */

function stockCell(product) {
  const wrap = document.createElement('div');

  const line = document.createElement('span');
  line.style.fontWeight = '700';

  if (!product.variant_count) {
    line.style.color = 'var(--subtle)';
    line.textContent = 'No variants';
    wrap.appendChild(line);
    return wrap;
  }

  const total = product.stock_total || 0;
  line.style.color = total === 0 ? 'var(--error)'
    : product.low_stock ? 'var(--warn)' : 'var(--dark)';
  line.textContent = total === 0 ? 'Out of stock'
    : total + (total === 1 ? ' left' : ' left');
  wrap.appendChild(line);

  const sub = document.createElement('div');
  sub.className = 'cell-sub';
  sub.textContent = product.variant_count + (product.variant_count === 1 ? ' variant' : ' variants');
  wrap.appendChild(sub);

  /* The bar's ceiling is the largest stock on the page, so the rows
     are comparable with each other. An absolute scale would make a
     20-unit line look empty next to a 2000-unit one. */
  const bar = document.createElement('div');
  bar.className = 'stock-bar';
  const fill = document.createElement('div');
  const share = Math.min(100, Math.round((total / Math.max(1, ceiling)) * 100));
  fill.className = 'stock-bar-fill ' + (total === 0 || product.low_stock ? 'out' : 'low');
  fill.style.width = Math.max(3, share) + '%';
  bar.appendChild(fill);
  wrap.appendChild(bar);

  return wrap;
}

function productRow(product) {
  const tr = document.createElement('tr');
  tr.dataset.id = product.uuid;

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.className = 'admin-check';
  check.setAttribute('data-row-check', '');
  check.setAttribute('aria-label', 'Select ' + product.title);
  tr.appendChild(cell(check));

  const media = document.createElement('div');
  media.className = 'cell-media';
  const thumb = document.createElement('div');
  thumb.className = 'cell-thumb';
  thumb.style.background = 'var(--input-bg)';
  paintImage(thumb, product.cover_image_id);
  const text = document.createElement('div');
  const title = document.createElement('div');
  title.className = 'cell-strong';
  title.textContent = product.title;
  const slug = document.createElement('div');
  slug.className = 'cell-sub';
  slug.textContent = '/' + product.slug;
  text.appendChild(title);
  text.appendChild(slug);
  media.appendChild(thumb);
  media.appendChild(text);
  tr.appendChild(cell(media));

  tr.appendChild(cell(categoryName(product.category_id)));

  const price = product.price_min === product.price_max
    ? money(product.price_min)
    : money(product.price_min) + ' – ' + money(product.price_max);
  tr.appendChild(cell(price, 'num'));

  tr.appendChild(cell(stockCell(product)));

  /* Two different things were both drawn as "Status": the product's
     own Draft/Active/Archived, and whether it is running out. The
     first is what the column is for; low stock rides along beside it
     because that is the row an operator is hunting for. */
  const statusCell = document.createElement('div');
  statusCell.style.cssText = 'display:flex;gap:6px;flex-wrap:wrap;';
  statusCell.appendChild(tag(product.status, STATUS_TAG[product.status] || 'tag-blue'));
  if (product.variant_count && product.stock_total === 0) statusCell.appendChild(tag('Out', 'tag-red'));
  else if (product.low_stock) statusCell.appendChild(tag('Low', 'tag-gold'));
  tr.appendChild(cell(statusCell));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Edit', () => {
    location.href = '/admin/products/new?id=' + encodeURIComponent(product.uuid);
  }));
  actions.appendChild(rowButton('View', () => {
    window.open('/product?slug=' + encodeURIComponent(product.slug), '_blank');
  }));
  actions.appendChild(rowButton('Delete', () => remove(product), { danger: true }));
  tr.appendChild(cell(actions, 'col-actions'));

  return tr;
}

/* ── Actions ──────────────────────────────────────────────────  */

async function remove(product) {
  const confirmed = await confirmModal({
    title: 'Delete this product?',
    message: '“' + product.title + '” will be archived and disappear from the storefront. '
      + 'Existing orders keep their copy of it.',
    confirmLabel: 'Delete product',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/product/' + product.uuid });
  if (!response.ok) return;
  toast.success('Product deleted');
  load();
  loadCounts();
}

function selectedIds() {
  return $$('[data-row-check]:checked').map((box) => box.closest('tr').dataset.id).filter(Boolean);
}

/*
  There is no bulk endpoint, so this is a loop. That is fine for the
  20 rows on screen and would not be for the whole catalogue, which
  is exactly why it only ever acts on the current selection.
*/
async function bulk(label, run) {
  const ids = selectedIds();
  if (!ids.length) return;

  const confirmed = await confirmModal({
    title: label + ' ' + ids.length + (ids.length === 1 ? ' product?' : ' products?'),
    message: 'This runs one request per product — there is no bulk endpoint behind it.',
    confirmLabel: label,
    danger: label === 'Delete',
  });
  if (!confirmed) return;

  let failed = 0;
  for (const id of ids) {
    const response = await run(id);
    if (!response.ok) failed++;
  }

  if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be updated");
  else toast.success(ids.length + (ids.length === 1 ? ' product updated' : ' products updated'));
  load();
  loadCounts();
}

/* ── Toolbar ──────────────────────────────────────────────────  */

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;
  host.innerHTML = '';

  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.tab ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;

    if (counts[value] !== undefined) {
      const count = document.createElement('span');
      count.className = 'admin-tab-count';
      count.textContent = counts[value].toLocaleString();
      el.appendChild(count);
    }

    const pick = () => { state.tab = value; state.page = 1; load(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

/*
  The four tab counts are four extra list calls with limit=1. Fired
  once on boot rather than on every filter change, because they do
  not depend on the search box — they are totals, not results.
*/
async function loadCounts() {
  const responses = await Promise.all(TABS.map(({ value }) =>
    Fetcher.get({
      endpoint: '/v1/admin/product/list',
      query: { tab: value, limit: 1 },
      showError: false,
    })));

  TABS.forEach(({ value }, index) => {
    counts[value] = responses[index].ok ? responses[index].data.total : 0;
  });
  paintTabs();
  paintSubtitle();
}

function paintSubtitle() {
  const subtitle = $('.admin-subtitle');
  if (!subtitle) return;
  const total = counts.all || 0;
  const out = counts.out_of_stock || 0;
  subtitle.textContent = total.toLocaleString() + (total === 1 ? ' product' : ' products')
    + (out ? ' · ' + out + ' out of stock' : '');
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/admin/product/list',
    query: {
      tab: state.tab,
      q: state.q,
      category: state.category,
      sort: state.sort,
      page: state.page,
      limit: state.limit,
    },
  });

  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load the catalogue" });
    { const showing = $('.admin-table-foot > span'); if (showing) showing.textContent = ''; }
    return;
  }

  const { items, total, limit, page } = response.data;
  ceiling = Math.max(1, ...items.map((item) => item.stock_total || 0));

  fillTable(tbody, items.map(productRow), {
    empty: state.q ? 'No products match “' + state.q + '”' : 'Nothing in this tab yet',
  });
  paintPager($('.pagination'), { total, limit, page }, (next) => { state.page = next; load(); });
  paintShowing($('.admin-table-foot > span'), { total, limit, page }, 'products');

  // admin.js wired the header checkbox against the rows that existed
  // at load; the table it was watching has been replaced since.
  const all = $('[data-check-all]');
  if (all) { all.checked = false; all.indeterminate = false; }
  const bulkBar = $('[data-bulk]');
  if (bulkBar) bulkBar.classList.remove('visible');
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  tbody = $('.admin-table tbody');

  const params = new URLSearchParams(location.search);
  if (params.get('q')) state.q = params.get('q');

  const categoryResponse = await Fetcher.get({ endpoint: '/v1/admin/category/list', showError: false });
  categories = categoryResponse.ok ? (categoryResponse.data || []) : [];

  // select.js copies the class onto its skin button, so ask for the
  // native element by tag or the two below are buttons.
  const [categorySelect, sortSelect] = $$('.admin-toolbar select.admin-filter');

  if (categorySelect) {
    categorySelect.innerHTML = '';
    const any = document.createElement('option');
    any.value = '';
    any.textContent = 'All categories';
    categorySelect.appendChild(any);
    categories.forEach((category) => {
      const option = document.createElement('option');
      option.value = category.uuid;
      option.textContent = category.name;
      categorySelect.appendChild(option);
    });
    categorySelect.addEventListener('change', () => {
      state.category = categorySelect.value;
      state.page = 1;
      load();
    });
  }

  if (sortSelect) {
    sortSelect.innerHTML = '';
    SORTS.forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      sortSelect.appendChild(option);
    });
    sortSelect.addEventListener('change', () => {
      state.sort = sortSelect.value;
      state.page = 1;
      load();
    });
  }

  const search = $('.admin-search input');
  if (search) {
    search.value = state.q;
    search.placeholder = 'Search products by name or slug…';
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      state.page = 1;
      load();
    }, 300));
  }

  /* No import endpoint exists. A button that cannot do the one thing
     its label promises is worse than a missing button. */
  const importButton = $$('.admin-head-actions .btn-outline')[0];
  if (importButton) importButton.remove();

  const [setActive, editCategory, deleteAll] = $$('[data-bulk] button');
  if (setActive) {
    setActive.addEventListener('click', () => bulk('Set active', (id) =>
      Fetcher.patch({ endpoint: '/v1/admin/product/' + id, body: { status: 'Active' }, showError: false })));
  }
  // Nothing behind a bulk category move, and moving them one by one
  // through this bar would hide how many requests it really is.
  if (editCategory) editCategory.remove();
  if (deleteAll) {
    deleteAll.addEventListener('click', () => bulk('Delete', (id) =>
      Fetcher.delete({ endpoint: '/v1/admin/product/' + id, showError: false })));
  }

  loadCounts();
  load();
})();
