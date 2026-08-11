/* ═══════════════════════════════════════════════════════════════
   Bambe Web — listing.js

   Behaviour and data for the product listing pages (listing.html,
   the filtered/search-result view, and search.html). The sidebar
   controls were drawn as static states; this makes them real:
   check, radio, size and colour filters all pick up and drop,
   cards collapse, the chip bar reflects what is selected, and the
   grid/list toggle switches the layout.

   The filters now drive an actual query. Every facet maps to a
   parameter of GET /v1/product/list via the `data-facet` attribute
   on its card, the selection is mirrored into the URL so a
   filtered view is linkable and the back button works, and the
   result count is the server's `total` rather than a guess.

   Category and brand options are rendered from the API — the
   wireframe's hardcoded lists would otherwise claim a catalogue
   the shop does not have.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { renderGrid, showSkeleton, hideSkeleton, emptyState } from './catalog.js';

(function() {

  var sidebar = document.querySelector('.filter-sidebar');
  var grid = document.querySelector('.product-grid');

  /* Where the price slider publishes itself, so the query builder
     can read it without reaching into that closure. */
  var priceState = null;

  /* ── Filter cards: collapse ─────────────────────────────────
     The head already had a chevron and a pointer cursor promising
     this, so it only ever needed wiring.                        */
  document.querySelectorAll('.filter-card-head').forEach(function(head) {
    var card = head.closest('.filter-card');
    head.setAttribute('role', 'button');
    head.setAttribute('tabindex', '0');
    head.setAttribute('aria-expanded', 'true');
    head.onclick = function() {
      var collapsed = card.classList.toggle('collapsed');
      head.setAttribute('aria-expanded', collapsed ? 'false' : 'true');
    };
  });

  /* ── Selectable facets ──────────────────────────────────────
     Checkboxes and chips are multi-select; ratings and category
     are single, because the API scopes to one category tree and
     one minimum rating.                                        */
  if (sidebar) {
    sidebar.addEventListener('click', function(e) {
      var option = e.target.closest('.filter-option');
      var chip = e.target.closest('.size-chip');
      var swatch = e.target.closest('.color-swatch');
      var rating = e.target.closest('.rating-filter-item');

      if (option) {
        var box = option.querySelector('.form-check-input');
        if (box) {
          var card = option.closest('.filter-card');
          var wasChecked = box.classList.contains('checked');
          // A single-select card clears its siblings first, so the
          // ticked row is always the only one.
          if (card && card.hasAttribute('data-single')) {
            card.querySelectorAll('.filter-option').forEach(function(o) {
              var b = o.querySelector('.form-check-input');
              if (b) b.classList.remove('checked');
              o.setAttribute('aria-checked', 'false');
            });
          }
          box.classList.toggle('checked', !wasChecked);
          // role="checkbox" sits on the row, so the state belongs
          // there too — the box inside it is only the paint.
          option.setAttribute('aria-checked', box.classList.contains('checked') ? 'true' : 'false');
        }
      } else if (chip) {
        chip.classList.toggle('selected');
        chip.setAttribute('aria-pressed', chip.classList.contains('selected') ? 'true' : 'false');
      } else if (swatch) {
        swatch.classList.toggle('selected');
        swatch.setAttribute('aria-pressed', swatch.classList.contains('selected') ? 'true' : 'false');
      } else if (rating) {
        var group = rating.closest('.filter-card-body');
        var dot = rating.querySelector('.form-radio-input');
        var already = dot && dot.classList.contains('checked');
        group.querySelectorAll('.rating-filter-item').forEach(function(r) {
          var d = r.querySelector('.form-radio-input');
          if (d) d.classList.remove('checked');
          r.setAttribute('aria-checked', 'false');
        });
        // Clicking the checked rating again drops the constraint —
        // otherwise a minimum rating could never be removed.
        if (dot && !already) {
          dot.classList.add('checked');
          rating.setAttribute('aria-checked', 'true');
        }
      } else {
        return;
      }
      refresh();
    });

    /* These are divs, so Enter/Space have to be wired by hand. */
    sidebar.addEventListener('keydown', function(e) {
      if (e.key !== 'Enter' && e.key !== ' ') return;
      var el = e.target;
      if (el.matches && el.matches('.filter-option, .size-chip, .color-swatch, .rating-filter-item, .filter-card-head')) {
        e.preventDefault();
        el.click();
      }
    });
  }

  /* a11y roles, stamped from whatever options exist at the time —
     called again after the API fills the category and brand cards.

     The boxes only carry a tick where the page was drawn already
     checked — .form-check-input hides the mark with opacity rather
     than removing it, so an empty box has nothing to reveal when it
     is ticked. Give every box the mark up front and let the
     stylesheet do the showing. */
  function stampRoles() {
    if (!sidebar) return;
    sidebar.querySelectorAll('.filter-option').forEach(function(o) {
      var box = o.querySelector('.form-check-input');
      if (box && !box.querySelector('svg')) {
        box.innerHTML = '<svg width="10" height="8" viewBox="0 0 10 8" fill="none" aria-hidden="true">' +
                        '<path d="M1 4l3 3 5-6" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
      }
      o.setAttribute('role', 'checkbox');
      o.setAttribute('tabindex', '0');
      o.setAttribute('aria-checked', box && box.classList.contains('checked') ? 'true' : 'false');
    });
    sidebar.querySelectorAll('.rating-filter-item').forEach(function(r) {
      var dot = r.querySelector('.form-radio-input');
      r.setAttribute('role', 'radio');
      r.setAttribute('tabindex', '0');
      r.setAttribute('aria-checked', dot && dot.classList.contains('checked') ? 'true' : 'false');
    });
    sidebar.querySelectorAll('.size-chip, .color-swatch').forEach(function(c) {
      c.setAttribute('role', 'button');
      c.setAttribute('tabindex', '0');
      c.setAttribute('aria-pressed', c.classList.contains('selected') ? 'true' : 'false');
      if (c.classList.contains('color-swatch') && c.dataset.name) c.setAttribute('aria-label', c.dataset.name);
    });
  }

  /* ── Price range ────────────────────────────────────────────
     The track, the fill and the two thumbs were drawn at fixed
     percentages with the pair of inputs holding matching text. The
     numbers are the source of truth here: the domain comes off the
     track, positions are derived from the values, and the two stay
     locked together whichever end the user drives.              */
  document.querySelectorAll('.price-range-track').forEach(function(track) {
    var fill = track.querySelector('.price-range-fill');
    var thumbs = Array.prototype.slice.call(track.querySelectorAll('.price-range-thumb'));
    var inputs = Array.prototype.slice.call(
      track.closest('.filter-card-body').querySelectorAll('.price-input'));
    if (thumbs.length !== 2 || inputs.length !== 2) return;

    function num(text, fallback) {
      var n = parseFloat(String(text).replace(/[^0-9.\-]/g, ''));
      return isNaN(n) ? fallback : n;
    }

    var min = num(track.dataset.min, 0);
    var max = num(track.dataset.max, 500);
    var step = num(track.dataset.step, 5);
    // Opens at the full span, not the wireframe's $20–$150: an
    // untouched slider must not silently exclude anything.
    var value = [min, max];

    function clamp(v) { return Math.min(max, Math.max(min, Math.round(v / step) * step)); }
    function pct(v) { return ((v - min) / (max - min)) * 100; }

    function paint() {
      thumbs[0].style.left = pct(value[0]) + '%';
      thumbs[1].style.left = pct(value[1]) + '%';
      fill.style.left = pct(value[0]) + '%';
      fill.style.right = (100 - pct(value[1])) + '%';
      inputs[0].value = '$' + value[0];
      inputs[1].value = '$' + value[1];
      thumbs.forEach(function(t, i) {
        t.setAttribute('aria-valuenow', value[i]);
        t.setAttribute('aria-valuetext', '$' + value[i]);
        // Neither handle may cross the other, so each one's own
        // range stops where its partner is standing.
        t.setAttribute('aria-valuemin', i === 0 ? min : value[0]);
        t.setAttribute('aria-valuemax', i === 0 ? value[1] : max);
      });
    }

    function set(i, v, quiet) {
      v = clamp(v);
      value[i] = i === 0 ? Math.min(v, value[1]) : Math.max(v, value[0]);
      paint();
      if (!quiet) refresh();
    }

    function valueAt(clientX) {
      var r = track.getBoundingClientRect();
      return min + ((clientX - r.left) / r.width) * (max - min);
    }

    // Published so the query builder can read the range, and so the
    // URL can drive it back on load.
    priceState = {
      min: min,
      max: max,
      get: function() { return { from: value[0], to: value[1] }; },
      // "Whole span" means the user has not constrained anything, so
      // no price parameter is sent at all.
      isFullRange: function() { return value[0] === min && value[1] === max; },
      set: function(from, to) {
        set(0, from, true);
        set(1, to, true);
      },
      reset: function() { set(0, min, true); set(1, max, true); },
    };

    thumbs.forEach(function(thumb, i) {
      thumb.setAttribute('role', 'slider');
      thumb.setAttribute('tabindex', '0');
      thumb.setAttribute('aria-label', i === 0 ? 'Minimum price' : 'Maximum price');

      thumb.addEventListener('pointerdown', function(e) {
        e.preventDefault();
        // Pointer capture keeps the drag alive when the cursor
        // leaves the 16px thumb, which it does immediately.
        thumb.setPointerCapture(e.pointerId);
        thumb.classList.add('dragging');
      });
      thumb.addEventListener('pointermove', function(e) {
        if (!thumb.hasPointerCapture(e.pointerId)) return;
        // Quiet while dragging: one query per drag, on release,
        // rather than one per pixel.
        set(i, valueAt(e.clientX), true);
      });
      thumb.addEventListener('pointerup', function() {
        thumb.classList.remove('dragging');
        refresh();
      });
      thumb.addEventListener('pointercancel', function() { thumb.classList.remove('dragging'); });

      thumb.addEventListener('keydown', function(e) {
        var jump = e.key === 'PageUp' || e.key === 'PageDown' ? step * 10 : step;
        if (e.key === 'ArrowLeft' || e.key === 'ArrowDown' || e.key === 'PageDown') {
          e.preventDefault(); set(i, value[i] - jump);
        } else if (e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'PageUp') {
          e.preventDefault(); set(i, value[i] + jump);
        } else if (e.key === 'Home') {
          e.preventDefault(); set(i, min);
        } else if (e.key === 'End') {
          e.preventDefault(); set(i, max);
        }
      });
    });

    // Clicking the bare track moves whichever handle is nearer.
    track.addEventListener('pointerdown', function(e) {
      if (e.target.closest('.price-range-thumb')) return;
      var v = valueAt(e.clientX);
      set(Math.abs(v - value[0]) <= Math.abs(v - value[1]) ? 0 : 1, v);
    });

    // Typing a number is the other way to drive it. Committing on
    // change rather than input lets a half-typed "1" alone.
    inputs.forEach(function(input, i) {
      input.addEventListener('change', function() { set(i, num(input.value, value[i])); });
      input.addEventListener('keydown', function(e) { if (e.key === 'Enter') input.blur(); });
    });

    paint();
  });

  /* ── Reading the current selection ──────────────────────────
     Each filter card reports the labels it has picked, so the chip
     bar and the clear buttons can both work off one shape.      */
  function pickedIn(card) {
    var picks = [];
    card.querySelectorAll('.filter-option').forEach(function(o) {
      var box = o.querySelector('.form-check-input');
      if (!box || !box.classList.contains('checked')) return;
      var label = o.querySelector('.filter-option-label');
      picks.push({ el: o, text: label.textContent.trim(), value: o.dataset.value });
    });
    card.querySelectorAll('.size-chip.selected').forEach(function(c) {
      picks.push({ el: c, text: c.textContent.trim(), value: c.textContent.trim() });
    });
    card.querySelectorAll('.color-swatch.selected').forEach(function(c) {
      picks.push({ el: c, text: c.dataset.name || 'Colour', value: c.dataset.name });
    });
    card.querySelectorAll('.rating-filter-item').forEach(function(r) {
      var dot = r.querySelector('.form-radio-input');
      if (dot && dot.classList.contains('checked')) {
        // "4★ & up" → 4
        var n = (r.dataset.label || '').match(/\d/);
        picks.push({ el: r, text: r.dataset.label || 'Rated', value: n ? n[0] : null });
      }
    });
    return picks;
  }

  function clearPick(pick) {
    var box = pick.el.querySelector ? pick.el.querySelector('.form-check-input, .form-radio-input') : null;
    if (box) {
      box.classList.remove('checked');
      pick.el.setAttribute('aria-checked', 'false');
    } else {
      pick.el.classList.remove('selected');
      pick.el.setAttribute('aria-pressed', 'false');
    }
  }

  /* ── Active filter chips ────────────────────────────────────
     Rebuilt from the sidebar every time, so the bar can never
     drift out of step with the controls. One chip per card, since
     "S, M, L" reads better than three separate size chips.     */
  var chipBar = document.querySelector('.active-filters');
  var chipLead = chipBar ? chipBar.querySelector('.active-filters-label') : null;

  function renderChips() {
    if (!chipBar) return;
    chipBar.querySelectorAll('.active-filter-chip').forEach(function(c) { c.remove(); });

    var any = false;
    document.querySelectorAll('.filter-card').forEach(function(card) {
      var picks = pickedIn(card);
      // The price card holds no pickable rows; it reports itself.
      if (card.dataset.facet === 'price' && priceState && !priceState.isFullRange()) {
        var range = priceState.get();
        any = true;
        appendChip(card, '$' + range.from + ' – $' + range.to, function() { priceState.reset(); refresh(); });
        return;
      }
      if (!picks.length) return;
      any = true;
      appendChip(card, picks.map(function(p) { return p.text; }).join(', '), function() {
        picks.forEach(clearPick);
        refresh();
      });
    });

    function appendChip(card, text, onRemove) {
      var chip = document.createElement('div');
      chip.className = 'active-filter-chip';
      chip.setAttribute('role', 'button');
      chip.setAttribute('tabindex', '0');
      var title = card.querySelector('.filter-card-title');
      chip.setAttribute('aria-label', 'Remove ' + (title ? title.textContent.trim() + ' filter' : 'filter'));
      chip.innerHTML = '<span></span><svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>';
      chip.firstChild.textContent = text;
      chip.onclick = onRemove;
      chip.onkeydown = function(e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); chip.click(); }
      };
      chipBar.appendChild(chip);
    }

    // With nothing selected the "Active:" lead-in has nothing to
    // introduce, so the whole bar steps out of the layout.
    chipBar.style.display = any ? '' : 'none';
    if (chipLead) chipLead.style.display = any ? '' : 'none';
  }

  /* ── Clear all ──────────────────────────────────────────────── */
  document.querySelectorAll('.clear-filters-btn').forEach(function(btn) {
    btn.setAttribute('role', 'button');
    btn.setAttribute('tabindex', '0');
    btn.onclick = function() {
      document.querySelectorAll('.filter-card').forEach(function(card) {
        pickedIn(card).forEach(clearPick);
      });
      if (priceState) priceState.reset();
      refresh();
    };
    btn.onkeydown = function(e) {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); btn.click(); }
    };
  });

  /* ── Grid / list view ───────────────────────────────────────
     The two buttons were painted with one marked active and no
     handler. The first is grid, the second is list.            */
  var toggle = document.querySelector('.listing-view-toggle');
  if (toggle && grid) {
    var buttons = Array.prototype.slice.call(toggle.querySelectorAll('.view-btn'));
    buttons.forEach(function(btn, i) {
      btn.setAttribute('role', 'button');
      btn.setAttribute('tabindex', '0');
      btn.setAttribute('aria-label', i === 0 ? 'Grid view' : 'List view');
      btn.setAttribute('aria-pressed', btn.classList.contains('active') ? 'true' : 'false');
      btn.onclick = function() {
        buttons.forEach(function(b) {
          b.classList.toggle('active', b === btn);
          b.setAttribute('aria-pressed', b === btn ? 'true' : 'false');
        });
        grid.classList.toggle('is-list', i === 1);
      };
      btn.onkeydown = function(e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); btn.click(); }
      };
    });
  }

  /* ── Sort ───────────────────────────────────────────────────
     The <select> was drawn with labels only. select.js skins the
     native element but leaves it as the value holder, so reading
     `.value` and listening for `change` still works.

     Matched on the tag, not on `.form-select`: select.js copies that
     class onto the <button> it builds, so the class alone can return
     a button whose `.options` is undefined — which order of script
     execution decides, and that is not something to depend on.     */
  var SORTS = ['relevance', 'price_asc', 'price_desc', 'newest', 'rating', 'best_selling'];
  var sortSelect = document.querySelector('.listing-sort select');
  if (sortSelect) {
    Array.prototype.slice.call(sortSelect.options).forEach(function(opt, i) {
      if (!opt.value || opt.value === opt.textContent) opt.value = SORTS[i] || 'relevance';
    });
    sortSelect.addEventListener('change', function() { state.page = 1; sync(); load(); });
  }

  /* ── Query state ────────────────────────────────────────────
     Everything the server needs, read off the controls. Multi-
     select facets go over as "a,b,c"; product/list splits them
     and matches any.                                           */
  var state = { page: 1 };

  function cardValues(facet) {
    var card = document.querySelector('.filter-card[data-facet="' + facet + '"]');
    if (!card) return [];
    return pickedIn(card).map(function(p) { return p.value; }).filter(Boolean);
  }

  function buildQuery() {
    var query = {};

    var category = cardValues('category');
    if (category.length) query.category = category[0];

    var brand = cardValues('brand');
    if (brand.length) query.brand = brand.join(',');

    var size = cardValues('size');
    if (size.length) query.size = size.join(',');

    var color = cardValues('color');
    if (color.length) query.color = color.join(',');

    var rating = cardValues('rating');
    if (rating.length) query.rating = rating[0];

    if (priceState && !priceState.isFullRange()) {
      var range = priceState.get();
      // Cents on the wire — the slider is in whole currency units.
      query.price_min = range.from * 100;
      query.price_max = range.to * 100;
    }

    var term = searchTerm();
    if (term) query.q = term;

    if (sortSelect && sortSelect.value && sortSelect.value !== 'relevance') query.sort = sortSelect.value;
    if (state.page > 1) query.page = state.page;

    return query;
  }

  /* The navbar search box is the same control on both pages. */
  function searchTerm() {
    var input = document.querySelector('.nav-search input');
    return input ? input.value.trim() : '';
  }

  /* ── URL ────────────────────────────────────────────────────
     The query string is the page's memory: reload, share or press
     back and the same grid comes up.                           */
  function sync() {
    var query = buildQuery();
    var params = new URLSearchParams();
    Object.keys(query).forEach(function(k) { params.set(k, query[k]); });
    var url = location.pathname + (params.toString() ? '?' + params.toString() : '');
    history.replaceState(null, '', url);
  }

  function applyUrl() {
    var params = new URLSearchParams(location.search);

    function tick(facet, values) {
      var card = document.querySelector('.filter-card[data-facet="' + facet + '"]');
      if (!card || !values.length) return;
      card.querySelectorAll('.filter-option').forEach(function(o) {
        if (values.indexOf(o.dataset.value) === -1) return;
        var box = o.querySelector('.form-check-input');
        if (box) box.classList.add('checked');
        o.setAttribute('aria-checked', 'true');
      });
      card.querySelectorAll('.size-chip').forEach(function(c) {
        if (values.indexOf(c.textContent.trim()) === -1) return;
        c.classList.add('selected');
        c.setAttribute('aria-pressed', 'true');
      });
      card.querySelectorAll('.color-swatch').forEach(function(c) {
        if (values.indexOf(c.dataset.name) === -1) return;
        c.classList.add('selected');
        c.setAttribute('aria-pressed', 'true');
      });
    }

    ['category', 'brand', 'size', 'color'].forEach(function(facet) {
      var raw = params.get(facet);
      if (raw) tick(facet, raw.split(','));
    });

    var rating = params.get('rating');
    if (rating) {
      document.querySelectorAll('.rating-filter-item').forEach(function(r) {
        var n = (r.dataset.label || '').match(/\d/);
        var dot = r.querySelector('.form-radio-input');
        var on = n && n[0] === rating;
        if (dot) dot.classList.toggle('checked', !!on);
        r.setAttribute('aria-checked', on ? 'true' : 'false');
      });
    }

    if (priceState && (params.get('price_min') || params.get('price_max'))) {
      priceState.set(
        Math.round(parseInt(params.get('price_min') || '0', 10) / 100),
        Math.round(parseInt(params.get('price_max') || String(priceState.max * 100), 10) / 100)
      );
    }

    var sort = params.get('sort');
    if (sort && sortSelect) sortSelect.value = sort;

    var q = params.get('q');
    var input = document.querySelector('.nav-search input');
    if (input) input.value = q || '';

    state.page = parseInt(params.get('page') || '1', 10) || 1;
  }

  /* ── Facet options from the API ─────────────────────────────
     The wireframe listed categories and brands it invented. These
     come from the catalogue, so an empty shop shows an empty card
     rather than six filters that match nothing.                */
  function optionRow(value, label, count) {
    var row = document.createElement('div');
    row.className = 'filter-option';
    row.dataset.value = value;
    row.innerHTML = '<div class="filter-option-label"><div class="form-check-input"></div><span></span></div>'
                  + '<span class="filter-option-count"></span>';
    row.querySelector('.filter-option-label span').textContent = label;
    row.querySelector('.filter-option-count').textContent = count === null || count === undefined ? '' : count;
    return row;
  }

  var categoryNames = {};

  async function loadFacets() {
    var categoryBody = document.querySelector('.filter-card[data-facet="category"] .filter-card-body');
    var brandBody = document.querySelector('.filter-card[data-facet="brand"] .filter-card-body');

    var results = await Promise.all([
      categoryBody ? Fetcher.get({ endpoint: '/v1/category/list', showError: false }) : null,
      brandBody ? Fetcher.get({ endpoint: '/v1/product/brand/list', showError: false }) : null,
    ]);

    if (categoryBody && results[0] && results[0].ok) {
      // The tree comes back nested; the sidebar is one flat list, so
      // children are folded in under their parent.
      var flat = [];
      (function walk(nodes, depth) {
        (nodes || []).forEach(function(node) {
          flat.push({ node: node, depth: depth });
          walk(node.children, depth + 1);
        });
      })(results[0].data, 0);

      flat.forEach(function(entry) {
        var row = optionRow(entry.node.slug, entry.node.name, entry.node.product_count);
        if (entry.depth) row.style.paddingLeft = (entry.depth * 14) + 'px';
        categoryBody.appendChild(row);
        // Kept so the crumb and the tab title can name the selected
        // category, which the sidebar only knows by slug.
        categoryNames[entry.node.slug] = entry.node.name;
      });
      if (!flat.length) categoryBody.innerHTML = '<div class="filter-option-count" style="padding:4px 0;">No categories yet</div>';
    }

    if (brandBody && results[1] && results[1].ok) {
      var brands = results[1].data || [];
      brands.forEach(function(b) { brandBody.appendChild(optionRow(b.uuid, b.name, null)); });
      if (!brands.length) brandBody.innerHTML = '<div class="filter-option-count" style="padding:4px 0;">No brands yet</div>';
    }

    stampRoles();
  }

  /* ── Pagination ─────────────────────────────────────────────
     Rebuilt from total/limit/page. The wireframe drew 1–3 … 18;
     this keeps that shape but only as far as the data goes.    */
  function renderPagination(total, page, limit) {
    var bar = document.querySelector('.pagination');
    if (!bar) return;
    var pages = Math.max(1, Math.ceil(total / limit));
    bar.innerHTML = '';
    bar.style.display = pages <= 1 ? 'none' : '';
    if (pages <= 1) return;

    function arrow(dir, disabled) {
      var el = document.createElement('div');
      el.className = 'page-btn' + (disabled ? ' disabled' : '');
      var stroke = disabled ? '#C4C0B9' : '#6A665F';
      el.innerHTML = dir === 'prev'
        ? '<svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M9 2L4 7l5 5" stroke="' + stroke + '" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>'
        : '<svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M5 2l5 5-5 5" stroke="' + stroke + '" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>';
      if (!disabled) el.onclick = function() { goTo(dir === 'prev' ? page - 1 : page + 1); };
      return el;
    }

    function numbered(n) {
      var el = document.createElement('div');
      el.className = 'page-btn' + (n === page ? ' active' : '');
      el.textContent = n;
      if (n !== page) el.onclick = function() { goTo(n); };
      return el;
    }

    function gap() {
      var el = document.createElement('div');
      el.className = 'page-btn';
      el.style.cssText = 'padding:0 12px;width:auto;';
      el.textContent = '…';
      return el;
    }

    bar.appendChild(arrow('prev', page <= 1));
    // First, last, and a window around the current page — enough to
    // navigate without printing eighteen buttons.
    var shown = [];
    for (var n = 1; n <= pages; n++) {
      if (n === 1 || n === pages || Math.abs(n - page) <= 1) shown.push(n);
    }
    shown.forEach(function(n, i) {
      if (i && n - shown[i - 1] > 1) bar.appendChild(gap());
      bar.appendChild(numbered(n));
    });
    bar.appendChild(arrow('next', page >= pages));
  }

  function goTo(page) {
    state.page = page;
    sync();
    load();
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  /* ── Load ───────────────────────────────────────────────────  */
  var countEl = document.querySelector('.listing-result-count');
  var inFlight = 0;

  async function load() {
    if (!grid) return;
    showSkeleton(grid);

    var mine = ++inFlight;
    var res = await Fetcher.get({ endpoint: '/v1/product/list', query: buildQuery() });
    // A slower earlier request must not overwrite a newer result —
    // easy to trigger by clicking three facets quickly.
    if (mine !== inFlight) return;

    hideSkeleton(grid);

    if (!res.ok) {
      emptyState(grid, "Couldn't load products", 'Please try again in a moment.');
      renderPagination(0, 1, 24);
      if (countEl) countEl.textContent = '';
      return;
    }

    var data = res.data;
    renderGrid(grid, data.items, {
      emptyTitle: 'No products match these filters',
      emptyDetail: 'Try widening the price range or clearing a filter.',
    });
    renderPagination(data.total, data.page, data.limit);

    if (countEl) {
      countEl.innerHTML = '<strong></strong> <span></span>';
      countEl.querySelector('strong').textContent = data.total.toLocaleString();
      countEl.querySelector('span').textContent = data.total === 1 ? 'product found' : 'products found';
    }

    renderSearchHeading(data.total);
    renderListingHeading(data.total);
  }

  /*
    The listing page shipped with "Men's Fashion — 843 results" baked into
    the title and the crumb. Both now follow whatever is actually selected,
    so a bookmarked filtered view is named correctly in the tab strip and
    the crumb never contradicts the grid under it.

    The search page owns its own heading (above), so it opts out.
  */
  function renderListingHeading(total) {
    if (document.getElementById('search-term')) return;

    var picked = cardValues('category');
    var name = picked.length ? (categoryNames[picked[0]] || picked[0]) : 'All products';

    var crumb = document.querySelector('.breadcrumb-item.current');
    if (crumb) crumb.textContent = name;

    document.title = name + ' — '
      + (total === 1 ? '1 result' : total.toLocaleString() + ' results')
      + ' — Bambe';
  }

  /* The search page names the term in three places — crumb, headline
     and subtitle. All three come from the term actually queried, so
     the page cannot claim results for something else. */
  function renderSearchHeading(total) {
    var term = searchTerm();
    var crumb = document.getElementById('search-crumb');
    var headline = document.getElementById('search-term');
    var subtitle = document.getElementById('search-subtitle');
    if (!headline) return;

    headline.textContent = term ? '"' + term + '"' : 'everything';
    if (crumb) crumb.textContent = term ? 'Search results for "' + term + '"' : 'Search results';
    if (subtitle) {
      subtitle.textContent = total === 1 ? 'Showing 1 result' : 'Showing ' + total.toLocaleString() + ' results';
    }
    document.title = term ? 'Search: "' + term + '" — Bambe' : 'Search — Bambe';
  }

  /* Any facet change is a new result set, so it goes back to
     page one — staying on page 7 of a narrower query would land
     on an empty grid. */
  function refresh() {
    state.page = 1;
    renderChips();
    sync();
    load();
  }

  /* ── Search box ─────────────────────────────────────────────  */
  var searchInput = document.querySelector('.nav-search input');
  var searchBtn = document.querySelector('.nav-search-btn');
  if (searchInput) {
    searchInput.addEventListener('keydown', function(e) { if (e.key === 'Enter') refresh(); });
  }
  if (searchBtn) searchBtn.onclick = refresh;

  // The back button restores a previous filter set, so re-read and
  // re-run rather than leaving the grid on the old query.
  window.addEventListener('popstate', function() {
    location.reload();
  });

  /* ── Boot ───────────────────────────────────────────────────  */
  stampRoles();
  loadFacets().then(function() {
    applyUrl();
    renderChips();
    load();
  });

})();
