/* ═══════════════════════════════════════════════════════════════
   Bambe Web — categories.js

   Drives the Categories page only: the two-level tree in the table
   and the Add category panel beneath it. Loaded after admin.js, which
   already handles row selection, tabs and the visibility switches.
═══════════════════════════════════════════════════════════════ */
(function() {

  var body = document.getElementById('cat-body');
  if (!body) return;

  var CHEVRON = '<svg width="12" height="12" viewBox="0 0 12 12" fill="none">' +
    '<path d="M4 2.5 8 6l-4 3.5" stroke="currentColor" stroke-width="1.6" ' +
    'stroke-linecap="round" stroke-linejoin="round"/></svg>';
  var KEBAB = '<svg width="15" height="15" viewBox="0 0 16 16" fill="none">' +
    '<circle cx="8" cy="3" r="1.3" fill="currentColor"/>' +
    '<circle cx="8" cy="8" r="1.3" fill="currentColor"/>' +
    '<circle cx="8" cy="13" r="1.3" fill="currentColor"/></svg>';

  function rows(sel) { return body.querySelectorAll(sel); }
  function parentRow(key) { return body.querySelector('tr[data-cat="' + key + '"]'); }
  function childRows(key) { return body.querySelectorAll('tr[data-parent="' + key + '"]'); }

  /* ── Tree expand / collapse ─────────────────────────────── */
  function setOpen(toggle, open) {
    var row = toggle.closest('tr');
    var name = row.querySelector('.cell-strong').textContent.trim();
    toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    toggle.setAttribute('aria-label', (open ? 'Collapse ' : 'Expand ') + name);
    childRows(row.getAttribute('data-cat')).forEach(function(child) {
      child.classList.toggle('cat-hidden', !open);
    });
  }

  body.addEventListener('click', function(e) {
    var toggle = e.target.closest('[data-cat-toggle]');
    if (!toggle) return;
    setOpen(toggle, toggle.getAttribute('aria-expanded') !== 'true');
    sync();
  });

  var allBtn = document.getElementById('cat-expand');
  if (allBtn) {
    allBtn.onclick = function() {
      var collapsing = allBtn.textContent.trim() === 'Collapse all';
      rows('[data-cat-toggle]').forEach(function(t) { setOpen(t, !collapsing); });
      allBtn.textContent = collapsing ? 'Expand all' : 'Collapse all';
      sync();
    };
  }

  /* A row's switch says whether the category is in the storefront menu;
     the dimming follows it. This listener is registered after admin.js's,
     so by the time it runs the .on class already reflects the new state. */
  document.addEventListener('click', function(e) {
    var toggle = e.target.closest ? e.target.closest('.toggle') : null;
    if (!toggle) return;
    var row = toggle.closest('tr');
    if (!row || !body.contains(row)) return;
    row.classList.toggle('cat-off', !toggle.classList.contains('on'));
    sync();
  });

  /* ── Counts ─────────────────────────────────────────────────
     Every readout on the page is derived from the table so the tree
     and the chrome around it can't drift apart.                  */
  var subtitleEl = document.querySelector('.admin-head .admin-subtitle');
  var footEl = document.getElementById('cat-foot');
  var navCountEl = document.querySelector('.admin-nav-item.active .admin-nav-count');
  var tabCounts = document.querySelectorAll('.admin-tabs .admin-tab-count');

  function plural(n, one, many) { return n + ' ' + (n === 1 ? one : many); }

  function sync() {
    var all = rows('tr').length;
    var tops = rows('tr[data-level="0"]').length;
    var subs = rows('tr[data-level="1"]').length;
    var hidden = rows('tr.cat-off').length;
    var shown = all - rows('tr.cat-hidden').length;

    var empty = 0, classified = 0;
    rows('td.num').forEach(function(cell) {
      var n = parseInt(cell.textContent, 10) || 0;
      if (n === 0) empty++;
      // Only leaf-level figures are summed — a parent's count already
      // includes everything filed under its children.
      if (cell.closest('tr').getAttribute('data-level') === '1') classified += n;
    });
    rows('tr[data-level="0"]').forEach(function(row) {
      if (!childRows(row.getAttribute('data-cat')).length) {
        classified += parseInt(row.querySelector('td.num').textContent, 10) || 0;
      }
    });

    if (subtitleEl) {
      subtitleEl.textContent = tops + ' top-level · ' + subs + ' subcategories · ' +
        classified + ' products classified';
    }
    if (footEl) {
      footEl.textContent = shown === all
        ? 'Showing ' + plural(all, 'category', 'categories')
        : 'Showing ' + shown + ' of ' + all + ' categories';
    }
    if (navCountEl) navCountEl.textContent = all;
    if (tabCounts[0]) tabCounts[0].textContent = all;
    if (tabCounts[1]) tabCounts[1].textContent = all - hidden;
    if (tabCounts[2]) tabCounts[2].textContent = hidden;
    if (tabCounts[3]) tabCounts[3].textContent = empty;
  }

  /* ── Add category ───────────────────────────────────────── */
  var nameEl = document.getElementById('c-name');
  var slugEl = document.getElementById('c-slug');
  var parentEl = document.getElementById('c-parent');
  var descEl = document.getElementById('c-desc');
  var swatchEl = document.getElementById('c-swatch');
  var visibleEl = document.getElementById('c-visible');
  var createBtn = document.getElementById('c-create');
  var msgEl = document.getElementById('c-msg');

  var jumpBtn = document.getElementById('cat-jump-new');
  var panel = document.getElementById('cat-new');
  if (jumpBtn && panel) {
    jumpBtn.onclick = function() {
      panel.scrollIntoView({ behavior: 'smooth', block: 'center' });
      if (nameEl) nameEl.focus({ preventScroll: true });
    };
  }

  /* "Shirts & Tops" → "shirts-tops": the ampersand and the spaces
     around it collapse into a single separator. */
  function slugify(text) {
    return String(text || '').toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');
  }

  function refreshSlug() {
    if (slugEl.dataset.touched) return;
    var leaf = slugify(nameEl.value);
    slugEl.value = leaf ? (parentEl.value ? '/' + parentEl.value + '/' + leaf : '/' + leaf) : '';
  }
  slugEl.addEventListener('input', function() { slugEl.dataset.touched = '1'; });
  nameEl.addEventListener('input', refreshSlug);
  parentEl.addEventListener('change', refreshSlug);

  function say(text, ok) {
    msgEl.textContent = text;
    msgEl.style.display = '';
    msgEl.style.color = ok ? 'var(--success)' : 'var(--error)';
  }

  function today() {
    var months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
    var d = new Date();
    return ('0' + d.getDate()).slice(-2) + ' ' + months[d.getMonth()] + ' ' + d.getFullYear();
  }

  function cell(tag, cls, text) {
    var el = document.createElement(tag);
    if (cls) el.className = cls;
    if (text !== undefined) el.textContent = text;
    return el;
  }

  function buildRow(spec) {
    var tr = document.createElement('tr');
    tr.setAttribute('data-level', spec.parent ? '1' : '0');
    if (spec.parent) tr.setAttribute('data-parent', spec.parent);
    else tr.setAttribute('data-cat', spec.key);
    if (!spec.visible) tr.className = 'cat-off';

    var tdCheck = cell('td');
    var box = document.createElement('input');
    box.type = 'checkbox';
    box.className = 'admin-check';
    box.setAttribute('data-row-check', '');
    box.setAttribute('aria-label', 'Select ' + spec.name);
    tdCheck.appendChild(box);

    var tdName = cell('td');
    var group = cell('div', 'cat-cell');
    // A new category has no children yet, so it takes the spacer rather
    // than a chevron; adding a child later swaps one in.
    group.appendChild(cell('span', 'cat-toggle-spacer'));
    var swatch = cell('div', 'cat-swatch' + (spec.parent ? ' sub' : ''));
    swatch.style.background = spec.color;
    group.appendChild(swatch);
    var text = cell('div');
    text.appendChild(cell('div', 'cell-strong', spec.name));
    if (!spec.parent) text.appendChild(cell('div', 'cell-sub', 'No subcategories'));
    group.appendChild(text);
    tdName.appendChild(group);

    var tdSlug = cell('td');
    tdSlug.appendChild(cell('span', 'cat-slug', spec.slug));

    var tdVis = cell('td');
    var toggle = cell('div', 'toggle' + (spec.visible ? ' on' : ''));
    toggle.setAttribute('role', 'switch');
    toggle.setAttribute('tabindex', '0');
    toggle.setAttribute('aria-checked', spec.visible ? 'true' : 'false');
    toggle.setAttribute('aria-label', 'Visible in storefront menu');
    tdVis.appendChild(toggle);

    var tdActions = cell('td', 'col-actions');
    var kebab = cell('span', 'row-btn');
    kebab.tabIndex = 0;
    kebab.title = 'More actions';
    kebab.innerHTML = KEBAB;
    tdActions.appendChild(kebab);

    [tdCheck, tdName, tdSlug, cell('td', 'num cat-count-zero', '0'),
     cell('td', null, today()), tdVis, tdActions].forEach(function(td) {
      tr.appendChild(td);
    });
    return tr;
  }

  /* A parent that just gained its first child needs a real chevron in
     place of the spacer that was standing in for one. */
  function ensureToggle(row) {
    var group = row.querySelector('.cat-cell');
    var toggle = group.querySelector('[data-cat-toggle]');
    if (toggle) return toggle;

    toggle = cell('span', 'cat-toggle');
    toggle.setAttribute('data-cat-toggle', '');
    toggle.setAttribute('role', 'button');
    toggle.setAttribute('tabindex', '0');
    toggle.setAttribute('aria-expanded', 'true');
    toggle.innerHTML = CHEVRON;

    var spacer = group.querySelector('.cat-toggle-spacer');
    if (spacer) group.replaceChild(toggle, spacer);
    else group.insertBefore(toggle, group.firstChild);
    return toggle;
  }

  function refreshSubCount(row) {
    var n = childRows(row.getAttribute('data-cat')).length;
    var sub = row.querySelector('.cell-sub');
    if (sub) sub.textContent = n ? plural(n, 'subcategory', 'subcategories') : 'No subcategories';
  }

  function insert(tr, parentKey) {
    if (!parentKey) { body.appendChild(tr); return; }

    var parent = parentRow(parentKey);
    if (!parent) { body.appendChild(tr); return; }

    var siblings = childRows(parentKey);
    var after = siblings.length ? siblings[siblings.length - 1] : parent;
    after.parentNode.insertBefore(tr, after.nextSibling);

    var toggle = ensureToggle(parent);
    // Dropping a child into a collapsed parent must not force it open.
    if (toggle.getAttribute('aria-expanded') !== 'true') tr.classList.add('cat-hidden');
    refreshSubCount(parent);
  }

  createBtn.onclick = function() {
    var name = nameEl.value.trim();
    if (!name) {
      say('Give the category a name first.', false);
      nameEl.focus();
      return;
    }

    refreshSlug();
    var slug = slugEl.value.trim();
    if (!slug) { say('That name does not produce a usable slug.', false); return; }
    if (slug.charAt(0) !== '/') slug = '/' + slug;

    var clash = false;
    rows('.cat-slug').forEach(function(el) {
      if (el.textContent.trim().toLowerCase() === slug.toLowerCase()) clash = true;
    });
    if (clash) {
      say(slug + ' is already in use — edit the slug and try again.', false);
      slugEl.focus();
      return;
    }

    var parentKey = parentEl.value;
    var key = slugify(name);
    var visible = visibleEl.classList.contains('on');

    var tr = buildRow({
      name: name,
      slug: slug,
      parent: parentKey,
      key: key,
      color: swatchEl.value,
      visible: visible
    });
    insert(tr, parentKey);

    // A new top level becomes a possible parent for the next one.
    if (!parentKey) {
      var option = document.createElement('option');
      option.value = key;
      option.textContent = name;
      parentEl.appendChild(option);
    }

    sync();
    tr.scrollIntoView({ behavior: 'smooth', block: 'center' });
    say('Added ' + name + ' at ' + slug + '.', true);

    nameEl.value = '';
    descEl.value = '';
    slugEl.value = '';
    delete slugEl.dataset.touched;
    nameEl.focus();
  };

  sync();

})();
