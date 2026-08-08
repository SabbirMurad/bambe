/* ═══════════════════════════════════════════════════════════════
   Bambe Web — product-form.js

   Drives the Add product page only. The product owns the title,
   description and classification; every priced variant lives in the
   SKU table, built a row at a time.
═══════════════════════════════════════════════════════════════ */
(function() {

  var body = document.getElementById('sku-body');
  if (!body) return;

  var empty = document.getElementById('sku-empty');
  var countEl = document.getElementById('sku-count');
  var titleEl = document.getElementById('p-title');

  /* Chip values, keyed by the data-chips name on each field. */
  var options = { tag: [] };

  /* Rough name → swatch guesses so typing a colour name into a row
     sets a sensible swatch. Anything unrecognised falls back to the
     brand tan and can be corrected with the colour picker. */
  var COLOR_HEX = {
    black: '#1A1A18', white: '#FFFFFF', ivory: '#F5F0E6', cream: '#DFD3C7',
    beige: '#D9C9B3', sand: '#E0D5C0', tan: '#C8A772', camel: '#C19A6B',
    brown: '#6B4A2F', chocolate: '#4A3025', rust: '#A85832', terracotta: '#B5623F',
    red: '#B3261E', burgundy: '#5C1F27', pink: '#E3A8B0', coral: '#E5645C',
    orange: '#FAAA39', mustard: '#C8A03A', gold: '#C8A772', yellow: '#E8C547',
    olive: '#6B6B45', green: '#4A6B45', sage: '#9CAF88', teal: '#3F7C7B',
    blue: '#439CDB', navy: '#1F3050', denim: '#4A6488', purple: '#5B4370',
    lilac: '#B9A7C9', grey: '#8A867F', gray: '#8A867F', charcoal: '#3A3835',
    silver: '#C4C0B9', stone: '#B8B2A7'
  };

  function hexFor(name) {
    return COLOR_HEX[String(name).trim().toLowerCase()] || '#C8A772';
  }

  /* ── Chip fields ────────────────────────────────────────── */
  document.querySelectorAll('[data-chips]').forEach(function(field) {
    var key = field.getAttribute('data-chips');
    var input = field.querySelector('input');
    if (!options[key]) options[key] = [];

    function render() {
      field.querySelectorAll('.chip').forEach(function(c) { c.remove(); });
      options[key].forEach(function(value, i) {
        var chip = document.createElement('span');
        chip.className = 'chip';
        chip.appendChild(document.createTextNode(value));

        var x = document.createElement('span');
        x.className = 'chip-x';
        x.textContent = '×';
        x.setAttribute('role', 'button');
        x.setAttribute('tabindex', '0');
        x.setAttribute('aria-label', 'Remove ' + value);
        x.onclick = function() {
          options[key].splice(i, 1);
          render();
        };
        chip.appendChild(x);

        field.insertBefore(chip, input);
      });
    }

    function add(raw) {
      raw.split(',').forEach(function(part) {
        var value = part.trim();
        if (!value) return;
        // Case-insensitive dedupe — "Navy" and "navy" are one option.
        var exists = options[key].some(function(v) {
          return v.toLowerCase() === value.toLowerCase();
        });
        if (!exists) options[key].push(value);
      });
      render();
    }

    input.addEventListener('keydown', function(e) {
      if (e.key === 'Enter' || e.key === ',') {
        e.preventDefault();
        add(input.value);
        input.value = '';
      } else if (e.key === 'Backspace' && input.value === '' && options[key].length) {
        options[key].pop();
        render();
      }
    });
    // Don't lose a half-typed value when focus moves on.
    input.addEventListener('blur', function() {
      if (input.value.trim()) { add(input.value); input.value = ''; }
    });
    // Clicking the padding of the field should focus the input.
    field.addEventListener('click', function(e) {
      if (e.target === field) input.focus();
    });
  });

  /* ── SKU rows ───────────────────────────────────────────── */
  function slug(text, len) {
    var s = String(text || '').toUpperCase().replace(/[^A-Z0-9]/g, '');
    return s.slice(0, len || 3);
  }

  function skuCode(size, color) {
    var base = slug(titleEl && titleEl.value, 3) || 'PRD';
    var parts = ['BM', base];
    if (color) parts.push(slug(color, 3));
    if (size) parts.push(slug(size, 3));
    return parts.join('-');
  }

  function addRow(size, color) {
    var tr = document.createElement('tr');

    /* Image — a hidden file input behind a clickable tile. The
       preview is local only; nothing is uploaded in the prototype. */
    var tdImg = document.createElement('td');
    var thumb = document.createElement('label');
    thumb.className = 'sku-thumb';
    thumb.title = 'Choose an image for this SKU';
    thumb.innerHTML = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M8 3.5v9M3.5 8h9" stroke="#94908A" stroke-width="1.6" stroke-linecap="round"/></svg>';
    var file = document.createElement('input');
    file.type = 'file';
    file.accept = 'image/*';
    file.hidden = true;
    file.setAttribute('aria-label', 'Image for ' + (color || '') + ' ' + (size || '') + ' SKU');
    file.onchange = function() {
      var f = file.files && file.files[0];
      if (!f) return;
      var url = URL.createObjectURL(f);
      thumb.innerHTML = '';
      var img = document.createElement('img');
      img.src = url;
      img.alt = '';
      // Release the blob once the browser has decoded it.
      img.onload = function() { URL.revokeObjectURL(url); };
      thumb.appendChild(img);
      thumb.appendChild(file);
      thumb.classList.add('filled');
    };
    thumb.appendChild(file);
    tdImg.appendChild(thumb);

    var tdSize = document.createElement('td');
    tdSize.innerHTML = '<input class="input-sm" type="text" placeholder="Size" aria-label="Size">';
    var sname = tdSize.querySelector('input');
    sname.value = size || '';

    var tdColor = document.createElement('td');
    var cell = document.createElement('div');
    cell.className = 'color-cell';
    var swatch = document.createElement('input');
    swatch.type = 'color';
    swatch.className = 'color-swatch';
    swatch.value = hexFor(color);
    swatch.setAttribute('aria-label', 'Colour swatch');
    var cname = document.createElement('input');
    cname.className = 'input-sm';
    cname.type = 'text';
    cname.placeholder = 'Colour';
    cname.value = color || '';
    cname.setAttribute('aria-label', 'Colour name');
    // Retyping the colour name re-guesses the swatch.
    cname.addEventListener('change', function() { swatch.value = hexFor(cname.value); });
    cell.appendChild(swatch);
    cell.appendChild(cname);
    tdColor.appendChild(cell);

    var tdPrice = document.createElement('td');
    tdPrice.innerHTML = '<div class="input-prefix"><input class="input-sm prefixed" type="number" min="0" step="0.01" inputmode="decimal" placeholder="0.00" aria-label="Price"></div>';

    var tdStock = document.createElement('td');
    tdStock.innerHTML = '<input class="input-sm" type="number" min="0" step="1" inputmode="numeric" placeholder="0" aria-label="Stock">';

    var tdCode = document.createElement('td');
    tdCode.innerHTML = '<input class="input-sm" type="text" placeholder="Auto" aria-label="SKU code">';
    var code = tdCode.querySelector('input');
    code.value = skuCode(size, color);

    /* The code tracks the title, size and colour until it is edited
       by hand, at which point it is left alone. */
    code.addEventListener('input', function() { code.dataset.touched = '1'; });
    function refreshCode() {
      if (!code.dataset.touched) code.value = skuCode(sname.value, cname.value);
    }
    sname.addEventListener('input', refreshCode);
    cname.addEventListener('input', refreshCode);
    if (titleEl) titleEl.addEventListener('input', refreshCode);

    var tdDel = document.createElement('td');
    tdDel.className = 'col-actions';
    var del = document.createElement('span');
    del.className = 'row-btn';
    del.tabIndex = 0;
    del.title = 'Remove this SKU';
    del.setAttribute('role', 'button');
    del.setAttribute('aria-label', 'Remove this SKU');
    del.innerHTML = '<svg width="15" height="15" viewBox="0 0 16 16" fill="none"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>';
    del.onclick = function() { tr.remove(); sync(); };
    del.onkeydown = function(e) {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); del.click(); }
    };
    tdDel.appendChild(del);

    [tdImg, tdSize, tdColor, tdPrice, tdStock, tdCode, tdDel].forEach(function(td) {
      tr.appendChild(td);
    });
    body.appendChild(tr);
  }

  function sync() {
    var n = body.children.length;
    if (countEl) countEl.textContent = n;
    if (empty) empty.style.display = n ? 'none' : 'block';
  }

  /* ── Actions ────────────────────────────────────────────── */
  var add = document.getElementById('add-sku');
  if (add) add.onclick = function() { addRow('', ''); sync(); };

  var clear = document.getElementById('clear-skus');
  if (clear) clear.onclick = function() { body.innerHTML = ''; sync(); };

  sync();

})();
