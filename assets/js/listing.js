/* ═══════════════════════════════════════════════════════════════
   Bambe Web — listing.js

   Behaviour for the product listing pages (listing.html, the
   filtered/search-result view, and search.html). The sidebar
   controls were drawn as static states; this makes them real:
   check, radio, size and colour filters all pick up and drop,
   cards collapse, the chip bar reflects what is selected, and the
   grid/list toggle switches the layout.

   Nothing here filters actual data — the grid is fixed wireframe
   markup — but every control now has state, and the result count
   is recomputed from the selected facets so the page stays
   internally consistent.
═══════════════════════════════════════════════════════════════ */
(function() {

  var sidebar = document.querySelector('.filter-sidebar');

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
     Checkboxes and chips are multi-select; ratings are a radio
     group, so picking one clears its siblings.                 */
  if (sidebar) {
    sidebar.addEventListener('click', function(e) {
      var option = e.target.closest('.filter-option');
      var chip = e.target.closest('.size-chip');
      var swatch = e.target.closest('.color-swatch');
      var rating = e.target.closest('.rating-filter-item');

      if (option) {
        var box = option.querySelector('.form-check-input');
        if (box) {
          box.classList.toggle('checked');
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

    /* a11y roles, stamped from the markup that is already there.

       The boxes only carry a tick where the page was drawn already
       checked — .form-check-input hides the mark with opacity rather
       than removing it, so an empty box has nothing to reveal when it
       is ticked. Give every box the mark up front and let the
       stylesheet do the showing. */
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

    var min = num(track.dataset.min, 0);
    var max = num(track.dataset.max, 500);
    var step = num(track.dataset.step, 5);
    var value = [num(inputs[0].value, min), num(inputs[1].value, max)];

    function num(text, fallback) {
      var n = parseFloat(String(text).replace(/[^0-9.\-]/g, ''));
      return isNaN(n) ? fallback : n;
    }
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

    function set(i, v) {
      v = clamp(v);
      value[i] = i === 0 ? Math.min(v, value[1]) : Math.max(v, value[0]);
      paint();
    }

    function valueAt(clientX) {
      var r = track.getBoundingClientRect();
      return min + ((clientX - r.left) / r.width) * (max - min);
    }

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
        set(i, valueAt(e.clientX));
      });
      thumb.addEventListener('pointerup', function() { thumb.classList.remove('dragging'); });
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
      picks.push({ el: o, text: label.textContent.trim() });
    });
    card.querySelectorAll('.size-chip.selected').forEach(function(c) {
      picks.push({ el: c, text: c.textContent.trim() });
    });
    card.querySelectorAll('.color-swatch.selected').forEach(function(c) {
      picks.push({ el: c, text: c.dataset.name || 'Colour' });
    });
    card.querySelectorAll('.rating-filter-item').forEach(function(r) {
      var dot = r.querySelector('.form-radio-input');
      if (dot && dot.classList.contains('checked')) picks.push({ el: r, text: r.dataset.label || 'Rated' });
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
      if (!picks.length) return;
      any = true;

      var chip = document.createElement('div');
      chip.className = 'active-filter-chip';
      chip.setAttribute('role', 'button');
      chip.setAttribute('tabindex', '0');
      var title = card.querySelector('.filter-card-title');
      chip.setAttribute('aria-label', 'Remove ' + (title ? title.textContent.trim() + ' filter' : 'filter'));
      chip.innerHTML = '<span></span><svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>';
      chip.firstChild.textContent = picks.map(function(p) { return p.text; }).join(', ');
      chip.onclick = function() { picks.forEach(clearPick); refresh(); };
      chip.onkeydown = function(e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); chip.click(); }
      };
      chipBar.appendChild(chip);
    });

    // With nothing selected the "Active:" lead-in has nothing to
    // introduce, so the whole bar steps out of the layout.
    chipBar.style.display = any ? '' : 'none';
    if (chipLead) chipLead.style.display = any ? '' : 'none';
  }

  /* ── Result count ───────────────────────────────────────────
     Illustrative, not a real query: the narrowest facet the user
     picked sets the count, which keeps the number moving in the
     direction the filters imply. The unfiltered total is taken
     from the markup on first run so each page keeps its own.   */
  var countEl = document.querySelector('.listing-result-count strong');
  var baseCount = countEl ? parseInt(countEl.textContent.replace(/\D/g, ''), 10) : 0;

  function renderCount() {
    if (!countEl) return;
    var narrowest = baseCount;
    document.querySelectorAll('.filter-card').forEach(function(card) {
      var sum = 0, has = false;
      card.querySelectorAll('.filter-option').forEach(function(o) {
        var box = o.querySelector('.form-check-input');
        var n = o.querySelector('.filter-option-count');
        if (box && box.classList.contains('checked') && n) {
          sum += parseInt(n.textContent.replace(/\D/g, ''), 10) || 0;
          has = true;
        }
      });
      if (has) narrowest = Math.min(narrowest, sum);
    });
    countEl.textContent = narrowest.toLocaleString();
  }

  function refresh() {
    renderChips();
    renderCount();
  }

  /* ── Clear all ──────────────────────────────────────────────── */
  document.querySelectorAll('.clear-filters-btn').forEach(function(btn) {
    btn.setAttribute('role', 'button');
    btn.setAttribute('tabindex', '0');
    btn.onclick = function() {
      document.querySelectorAll('.filter-card').forEach(function(card) {
        pickedIn(card).forEach(clearPick);
      });
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
  var grid = document.querySelector('.product-grid');
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

  refresh();

})();
