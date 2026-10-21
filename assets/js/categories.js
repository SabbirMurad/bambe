/* ═══════════════════════════════════════════════════════════════
   Bambee Web — categories.js

   The category tree and the Add-category panel beneath it, against
   the four `/v1/admin/category/*` endpoints.

   The tree is built here rather than paged, because the endpoint
   deliberately returns every category in one array — there are
   tens of them, not thousands, and a parent whose child is on
   page two is not a tree.

   Two levels, matching the model: `parent_id` is a single optional
   id and the storefront menu renders exactly two levels. A
   category naming a grandchild is drawn at the top level with a
   note, rather than silently disappearing.

   The visibility switch in each row is the real thing — it PATCHes
   and rolls back if the write fails, rather than being the
   decorative `toggle.js` flip the wireframe had.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, fullDate, cell, fillTable, debounce, rowButton,
  mountAdminChrome, openModal, confirmModal, field, textInput, textArea, selectInput,
  uploadImage,
} from './admin-core.js';
import { paintImage, clearImage } from './catalog.js';

const TABS = [
  { value: 'all', label: 'All' },
  { value: 'visible', label: 'Visible' },
  { value: 'hidden', label: 'Hidden' },
  { value: 'empty', label: 'Empty' },
];

const LEVELS = [
  { value: '', label: 'All levels' },
  { value: 'top', label: 'Top level only' },
  { value: 'sub', label: 'Subcategories only' },
];

const SORTS = [
  { value: 'position', label: 'Menu order' },
  { value: 'name', label: 'Name A–Z' },
  { value: 'products', label: 'Most products' },
  { value: 'updated', label: 'Recently updated' },
];

const CHEVRON = '<svg width="12" height="12" viewBox="0 0 12 12" fill="none">'
  + '<path d="M4 2.5 8 6l-4 3.5" stroke="currentColor" stroke-width="1.6" '
  + 'stroke-linecap="round" stroke-linejoin="round"/></svg>';

let all = [];                 // every category, as returned
let collapsed = new Set();    // parent uuids whose children are folded away
let state = { tab: 'all', q: '', level: '', sort: 'position' };
let tbody = null;

const byId = (id) => all.find((c) => c.uuid === id);
const childrenOf = (id) => all.filter((c) => c.parent_id === id);

/* ── Rows ─────────────────────────────────────────────────────  */

function pathFor(category) {
  const parent = category.parent_id ? byId(category.parent_id) : null;
  return '/' + (parent ? parent.slug + '/' : '') + category.slug;
}

function visibilityToggle(category) {
  const el = document.createElement('div');
  el.className = 'toggle' + (category.visible ? ' on' : '');
  el.setAttribute('role', 'switch');
  el.tabIndex = 0;
  el.setAttribute('aria-checked', String(category.visible));
  el.setAttribute('aria-label', 'Visible in the storefront menu');

  const flip = async () => {
    const next = !el.classList.contains('on');
    el.classList.toggle('on', next);
    el.setAttribute('aria-checked', String(next));

    const response = await Fetcher.patch({
      endpoint: '/v1/admin/category/' + category.uuid,
      body: { visible: next },
    });

    if (!response.ok) {
      el.classList.toggle('on', !next);
      el.setAttribute('aria-checked', String(!next));
      return;
    }

    category.visible = next;
    toast.success(category.name + (next ? ' is now visible' : ' is now hidden'));
    // The Visible/Hidden tab counts are derived from this array, and the
    // row may not belong in the current tab any more.
    paint();
  };

  /* toggle.js flips any `.toggle` it finds through a delegated
     document listener, and it runs after this one — so reading the
     class here would give the state before its flip. The switch is
     driven from here instead, and marked read-only so the shared
     handler leaves it alone. */
  el.dataset.readonly = 'true';
  el.addEventListener('click', flip);
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
  });

  return el;
}

function categoryRow(category, depth) {
  const tr = document.createElement('tr');
  tr.dataset.id = category.uuid;
  tr.dataset.level = String(depth);
  if (!category.visible) tr.classList.add('cat-off');

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.className = 'admin-check';
  check.setAttribute('data-row-check', '');
  check.setAttribute('aria-label', 'Select ' + category.name);
  tr.appendChild(cell(check));

  const treeCell = document.createElement('div');
  treeCell.className = 'cat-cell';

  const kids = childrenOf(category.uuid);
  if (depth === 0 && kids.length) {
    const toggle = document.createElement('span');
    toggle.className = 'cat-toggle';
    toggle.tabIndex = 0;
    toggle.setAttribute('role', 'button');
    const open = !collapsed.has(category.uuid);
    toggle.setAttribute('aria-expanded', String(open));
    toggle.setAttribute('aria-label', (open ? 'Collapse ' : 'Expand ') + category.name);
    toggle.innerHTML = CHEVRON;

    const flip = () => {
      if (collapsed.has(category.uuid)) collapsed.delete(category.uuid);
      else collapsed.add(category.uuid);
      paint();
    };
    toggle.addEventListener('click', flip);
    toggle.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
    });
    treeCell.appendChild(toggle);
  } else {
    const spacer = document.createElement('span');
    spacer.className = 'cat-toggle-spacer';
    treeCell.appendChild(spacer);
  }

  const swatch = document.createElement('div');
  swatch.className = 'cat-swatch' + (depth ? ' sub' : '');
  swatch.style.background = category.swatch || 'var(--border)';
  treeCell.appendChild(swatch);

  const text = document.createElement('div');
  const name = document.createElement('div');
  name.className = 'cell-strong';
  name.textContent = category.name;
  text.appendChild(name);

  if (depth === 0) {
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = kids.length
      ? kids.length + (kids.length === 1 ? ' subcategory' : ' subcategories')
      : 'No subcategories';
    text.appendChild(sub);
  }
  treeCell.appendChild(text);
  tr.appendChild(cell(treeCell));

  const slug = document.createElement('span');
  slug.className = 'cat-slug';
  slug.textContent = pathFor(category);
  tr.appendChild(cell(slug));

  const count = cell(String(category.product_count || 0), 'num');
  if (!category.product_count) count.classList.add('cat-count-zero');
  tr.appendChild(count);

  tr.appendChild(cell(fullDate(category.modified_at)));
  tr.appendChild(cell(visibilityToggle(category)));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Edit', () => edit(category)));
  actions.appendChild(rowButton('Delete', () => remove(category), { danger: true }));
  tr.appendChild(cell(actions, 'col-actions'));

  return tr;
}

/* ── Filtering and ordering ───────────────────────────────────  */

function matches(category) {
  if (state.tab === 'visible' && !category.visible) return false;
  if (state.tab === 'hidden' && category.visible) return false;
  if (state.tab === 'empty' && category.product_count) return false;

  if (state.level === 'top' && category.parent_id) return false;
  if (state.level === 'sub' && !category.parent_id) return false;

  if (state.q) {
    const needle = state.q.toLowerCase();
    if (!category.name.toLowerCase().includes(needle)
      && !category.slug.toLowerCase().includes(needle)) return false;
  }
  return true;
}

function ordered(list) {
  const copy = [...list];
  switch (state.sort) {
    case 'name': return copy.sort((a, b) => a.name.localeCompare(b.name));
    case 'products': return copy.sort((a, b) => (b.product_count || 0) - (a.product_count || 0));
    case 'updated': return copy.sort((a, b) => (b.modified_at || 0) - (a.modified_at || 0));
    default: return copy.sort((a, b) => (a.position || 0) - (b.position || 0));
  }
}

/*
  Draws the tree. A filter that hits only a child still shows its
  parent — an indented row with nothing above it reads as a
  top-level category with a strange name.
*/
function paint() {
  const rows = [];
  const tops = ordered(all.filter((c) => !c.parent_id || !byId(c.parent_id)));

  tops.forEach((top) => {
    const kids = ordered(childrenOf(top.uuid));
    const visibleKids = kids.filter(matches);
    const topMatches = matches(top);

    if (!topMatches && !visibleKids.length) return;

    rows.push(categoryRow(top, 0));
    if (collapsed.has(top.uuid)) return;

    (topMatches ? kids.filter(matches) : visibleKids)
      .forEach((kid) => rows.push(categoryRow(kid, 1)));
  });

  const drew = fillTable(tbody, rows, {
    empty: state.q ? 'No categories match “' + state.q + '”' : 'Nothing in this tab',
  });

  const foot = $('#cat-foot');
  if (foot) {
    const shown = drew ? rows.length : 0;
    foot.textContent = 'Showing ' + shown + ' of ' + all.length
      + (all.length === 1 ? ' category' : ' categories');
  }

  const subtitle = $('.admin-subtitle');
  if (subtitle) {
    const tops2 = all.filter((c) => !c.parent_id).length;
    const subs = all.length - tops2;
    const classified = all.reduce((sum, c) => sum + (c.product_count || 0), 0);
    subtitle.textContent = tops2 + ' top-level · ' + subs
      + (subs === 1 ? ' subcategory · ' : ' subcategories · ')
      + classified + ' products classified';
  }

  paintTabs();

  const checkAll = $('[data-check-all]');
  if (checkAll) { checkAll.checked = false; checkAll.indeterminate = false; }
  const bulkBar = $('[data-bulk]');
  if (bulkBar) bulkBar.classList.remove('visible');
}

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;

  const counts = {
    all: all.length,
    visible: all.filter((c) => c.visible).length,
    hidden: all.filter((c) => !c.visible).length,
    empty: all.filter((c) => !c.product_count).length,
  };

  host.innerHTML = '';
  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.tab ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;
    const count = document.createElement('span');
    count.className = 'admin-tab-count';
    count.textContent = String(counts[value]);
    el.appendChild(count);

    const pick = () => { state.tab = value; paint(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

/* ── Create / edit / delete ───────────────────────────────────  */

function slugify(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
}

function parentOptions(exclude) {
  const options = [{ value: '', label: '— None (top level) —' }];
  all
    .filter((c) => !c.parent_id && c.uuid !== exclude)
    .sort((a, b) => a.name.localeCompare(b.name))
    .forEach((c) => options.push({ value: c.uuid, label: c.name }));
  return options;
}

/* ── Category image ───────────────────────────────────────────
   The picture on the category's tile on the home page. Uploaded the
   moment a file is picked (as a CategoryImage), but only attached to
   the category when the form is saved — the same split the product
   form uses for SKU pictures. `value()` is the image id to save, or
   null once it has been removed. */
const MAX_IMAGE_BYTES = 8 * 1024 * 1024;

function imagePicker(initialId) {
  let imageId = initialId || null;

  const el = document.createElement('div');
  el.className = 'cat-image-picker';

  const preview = document.createElement('div');
  preview.className = 'cat-image-preview';
  preview.setAttribute('role', 'button');
  preview.tabIndex = 0;
  preview.setAttribute('aria-label', 'Choose a category image');

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
      paintImage(preview, imageId);
    } else {
      clearImage(preview);
      preview.classList.remove('has-image');
      preview.textContent = 'No image';
    }
    choose.textContent = imageId ? 'Replace' : 'Upload image';
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
    if (file.size > MAX_IMAGE_BYTES) { toast.error('That image is over 8 MB — please pick a smaller one.'); return; }

    choose.disabled = true;
    choose.textContent = 'Uploading…';
    const id = await uploadImage(file, 'CategoryImage');
    choose.disabled = false;
    if (id) imageId = id;
    paint();
  });

  el.append(preview, actions, input);
  paint();

  return {
    el,
    value: () => imageId,
    reset: () => { imageId = null; paint(); },
  };
}

function edit(category) {
  const form = document.createElement('div');

  const name = textInput({ value: category.name });
  form.appendChild(field('Name', name));

  const slug = textInput({ value: category.slug });
  form.appendChild(field('Slug', slug));

  /* A category cannot be its own parent — `update.rs` refuses that —
     and moving a parent under one of its own children would make a
     cycle the two-level tree cannot draw, so children are not
     offered as parents either. */
  const parent = selectInput(parentOptions(category.uuid), category.parent_id || '');
  form.appendChild(field('Parent', parent));

  const description = textArea({ value: category.description || '', rows: 3 });
  form.appendChild(field('Description', description));

  const image = imagePicker(category.image_id);
  form.appendChild(field('Image', image.el, 'Shown on the category’s tile on the home page.'));

  const swatch = document.createElement('input');
  swatch.type = 'color';
  swatch.className = 'color-swatch';
  swatch.value = category.swatch || '#C8A772';
  form.appendChild(field('Swatch', swatch));

  const position = textInput({ value: String(category.position || 0) });
  form.appendChild(field('Menu position', position, 'Lower numbers come first.'));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  openModal({
    title: 'Edit ' + category.name,
    body: form,
    width: '520px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Save',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (!name.value.trim() || !slug.value.trim()) {
            error.textContent = 'A name and a slug are both required.';
            error.style.display = '';
            return;
          }

          button.disabled = true;
          const response = await Fetcher.patch({
            endpoint: '/v1/admin/category/' + category.uuid,
            body: {
              name: name.value.trim(),
              slug: slugify(slug.value.trim()),
              description: description.value.trim() || null,
              parent_id: parent.value || null,
              swatch: swatch.value,
              // '' removes the picture; update.rs reads an absent field as "unchanged".
              image_id: image.value() || '',
              position: Number(position.value) || 0,
            },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Category updated');
          load();
        },
      },
    ],
  });
}

async function remove(category) {
  const kids = childrenOf(category.uuid);
  const confirmed = await confirmModal({
    title: 'Delete ' + category.name + '?',
    message: kids.length
      ? 'It has ' + kids.length + (kids.length === 1 ? ' subcategory' : ' subcategories')
        + ' under it, and ' + (category.product_count || 0) + ' products classified into it.'
      : (category.product_count || 0) + ' products are classified into it.',
    confirmLabel: 'Delete category',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/category/' + category.uuid });
  if (!response.ok) return;
  toast.success('Category deleted');
  load();
}

function selectedIds() {
  return $$('[data-row-check]:checked').map((box) => box.closest('tr').dataset.id).filter(Boolean);
}

async function bulkVisibility(visible) {
  const ids = selectedIds();
  if (!ids.length) return;

  let failed = 0;
  for (const id of ids) {
    const response = await Fetcher.patch({
      endpoint: '/v1/admin/category/' + id,
      body: { visible },
      showError: false,
    });
    if (!response.ok) failed++;
  }

  if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be updated");
  else toast.success(ids.length + (visible ? ' now visible' : ' now hidden'));
  load();
}

async function bulkMove() {
  const ids = selectedIds();
  if (!ids.length) return;

  const parent = selectInput(parentOptions(ids.length === 1 ? ids[0] : null), '');
  const form = document.createElement('div');
  form.appendChild(field('Move under', parent,
    'Only top-level categories can be parents — the storefront menu is two levels deep.'));

  openModal({
    title: 'Move ' + ids.length + (ids.length === 1 ? ' category' : ' categories'),
    body: form,
    width: '440px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Move',
        className: 'btn-primary',
        onClick: async (close, button) => {
          button.disabled = true;
          let failed = 0;
          for (const id of ids) {
            if (id === parent.value) { failed++; continue; }
            const response = await Fetcher.patch({
              endpoint: '/v1/admin/category/' + id,
              body: { parent_id: parent.value || null },
              showError: false,
            });
            if (!response.ok) failed++;
          }
          button.disabled = false;
          close();

          if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be moved");
          else toast.success('Moved');
          load();
        },
      },
    ],
  });
}

async function bulkDelete() {
  const ids = selectedIds();
  if (!ids.length) return;

  const confirmed = await confirmModal({
    title: 'Delete ' + ids.length + (ids.length === 1 ? ' category?' : ' categories?'),
    message: 'One request per category — there is no bulk endpoint behind it. '
      + 'Products classified into them are not deleted.',
    confirmLabel: 'Delete',
  });
  if (!confirmed) return;

  let failed = 0;
  for (const id of ids) {
    const response = await Fetcher.delete({ endpoint: '/v1/admin/category/' + id, showError: false });
    if (!response.ok) failed++;
  }

  if (failed) toast.error(failed + ' of ' + ids.length + " couldn't be deleted");
  else toast.success('Deleted');
  load();
}

/* ── The Add panel ────────────────────────────────────────────  */

function wireCreatePanel() {
  const name = $('#c-name');
  const slug = $('#c-slug');
  const parent = $('#c-parent');
  const description = $('#c-desc');
  const swatch = $('#c-swatch');
  const visible = $('#c-visible');
  const create = $('#c-create');
  const message = $('#c-msg');
  if (!create) return;

  const imageHost = $('#c-image');
  const image = imageHost ? imagePicker(null) : null;
  if (image) imageHost.appendChild(image.el);

  // The switch in the panel is ours, not toggle.js's.
  if (visible) {
    visible.dataset.readonly = 'true';
    visible.setAttribute('role', 'switch');
    visible.setAttribute('aria-checked', 'true');
    const flip = () => {
      const on = !visible.classList.contains('on');
      visible.classList.toggle('on', on);
      visible.setAttribute('aria-checked', String(on));
    };
    visible.addEventListener('click', flip);
    visible.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
    });
  }

  let slugTouched = false;
  if (slug) slug.addEventListener('input', () => { slugTouched = true; });

  const refreshSlug = () => {
    if (slugTouched || !name || !slug) return;
    slug.value = slugify(name.value);
  };
  if (name) name.addEventListener('input', refreshSlug);
  if (parent) parent.addEventListener('change', refreshSlug);

  const say = (text, bad) => {
    if (!message) return;
    message.style.display = text ? '' : 'none';
    message.style.color = bad ? 'var(--error)' : 'var(--subtle)';
    message.textContent = text;
  };

  create.addEventListener('click', async () => {
    if (!name.value.trim()) return say('Give the category a name.', true);
    if (!slug.value.trim()) return say('It needs a slug.', true);

    create.disabled = true;
    const response = await Fetcher.post({
      endpoint: '/v1/admin/category',
      body: {
        name: name.value.trim(),
        slug: slugify(slug.value.trim()),
        description: description && description.value.trim() ? description.value.trim() : null,
        parent_id: parent && parent.value ? parent.value : null,
        swatch: swatch ? swatch.value : null,
        image_id: image ? image.value() : null,
        visible: visible ? visible.classList.contains('on') : true,
        position: all.filter((c) => !c.parent_id).length,
      },
    });
    create.disabled = false;
    if (!response.ok) return;

    name.value = '';
    slug.value = '';
    if (description) description.value = '';
    if (image) image.reset();
    slugTouched = false;
    say('Created.');
    toast.success('Category created');
    load();
  });
}

function refreshParentSelect() {
  const parent = $('#c-parent');
  if (!parent) return;
  const chosen = parent.value;
  parent.innerHTML = '';
  parentOptions(null).forEach(({ value, label }) => {
    const option = document.createElement('option');
    option.value = value;
    option.textContent = label;
    if (value === chosen) option.selected = true;
    parent.appendChild(option);
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({ endpoint: '/v1/admin/category/list' });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load categories" });
    return;
  }

  all = response.data || [];
  refreshParentSelect();
  paint();
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  tbody = $('#cat-body');
  if (!tbody) return;

  const [levelSelect, sortSelect] = $$('.admin-toolbar select.admin-filter');

  const fillSelect = (select, options, onChange) => {
    if (!select) return;
    select.innerHTML = '';
    options.forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      select.appendChild(option);
    });
    select.addEventListener('change', () => { onChange(select.value); paint(); });
  };

  fillSelect(levelSelect, LEVELS, (value) => { state.level = value; });
  fillSelect(sortSelect, SORTS, (value) => { state.sort = value; });

  const search = $('.admin-search input');
  if (search) {
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      paint();
    }, 200));
  }

  const expand = $('#cat-expand');
  if (expand) {
    expand.addEventListener('click', () => {
      const anyOpen = all.some((c) => !c.parent_id && childrenOf(c.uuid).length && !collapsed.has(c.uuid));
      collapsed = anyOpen
        ? new Set(all.filter((c) => !c.parent_id).map((c) => c.uuid))
        : new Set();
      expand.textContent = anyOpen ? 'Expand all' : 'Collapse all';
      paint();
    });
  }

  const jump = $('#cat-jump-new');
  if (jump) {
    jump.addEventListener('click', () => {
      const panel = $('#cat-new');
      if (panel) panel.scrollIntoView({ behavior: 'smooth', block: 'start' });
      const name = $('#c-name');
      if (name) setTimeout(() => name.focus(), 400);
    });
  }

  const [show, hide, move, del] = $$('[data-bulk] button');
  if (show) show.addEventListener('click', () => bulkVisibility(true));
  if (hide) hide.addEventListener('click', () => bulkVisibility(false));
  if (move) move.addEventListener('click', bulkMove);
  if (del) del.addEventListener('click', bulkDelete);

  wireCreatePanel();
  load();
})();
