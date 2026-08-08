/* ═══════════════════════════════════════════════════════════════
   Bambe Web — select.js

   Replaces every native <select> on the page with a themed button
   and listbox, so dropdowns look like the rest of the design system
   instead of like the operating system.

   The native element is not removed. It stays in the DOM, hidden,
   and remains the one place the value lives: existing code that
   reads `.value`, sets `.value`, appends an <option> at runtime, or
   listens for `change` keeps working untouched, and a form that
   submits still submits. Everything here is a skin over that.

   Drop <script src="…/js/select.js"></script> on a page and every
   select on it is upgraded — there is no per-select markup.
═══════════════════════════════════════════════════════════════ */
(function() {

  var CARET = '<svg class="select-caret" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M3 4.5 6 7.5 9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  var CHECK = '<svg class="select-check" width="13" height="10" viewBox="0 0 13 10" fill="none" aria-hidden="true"><path d="M1 5.2 4.6 8.8 12 1.4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>';

  var uid = 0;
  var open = null;   // the one enhanced select currently showing its menu

  function esc(text) {
    return String(text).replace(/[&<>"]/g, function(c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }

  function enhance(native) {
    if (native.dataset.enhanced) return;
    native.dataset.enhanced = '1';

    var id = native.id || ('select-' + (++uid));
    var btn = document.createElement('button');
    var menu = null;
    var active = -1;              // keyboard cursor while the menu is open
    var typed = '';               // type-ahead buffer
    var typedAt = 0;

    /* The button wears the skin the select was wearing, so it lands
       in the layout at the same size in the same place — .form-select
       fills its field row, .admin-filter sits inline in a toolbar. */
    btn.type = 'button';
    btn.className = native.className + ' select-btn';
    btn.setAttribute('style', native.getAttribute('style') || '');
    btn.setAttribute('role', 'combobox');
    btn.setAttribute('aria-haspopup', 'listbox');
    btn.setAttribute('aria-expanded', 'false');
    btn.id = id + '-btn';
    if (native.getAttribute('aria-label')) btn.setAttribute('aria-label', native.getAttribute('aria-label'));
    if (native.disabled) btn.disabled = true;

    /* A <label for> pointing at the select would focus something the
       user can no longer see, so it is re-pointed at the button. */
    if (native.id) {
      var label = document.querySelector('label[for="' + native.id + '"]');
      if (label) label.setAttribute('for', btn.id);
    }

    btn.innerHTML = '<span class="select-label"></span>' + CARET;
    var labelEl = btn.querySelector('.select-label');

    native.classList.add('select-native');
    native.setAttribute('tabindex', '-1');
    native.setAttribute('aria-hidden', 'true');
    native.parentNode.insertBefore(btn, native);

    function options() {
      return Array.prototype.slice.call(native.options);
    }

    /* Repaint the button from whatever the native currently holds. */
    function paint() {
      var opt = native.options[native.selectedIndex];
      var text = opt ? opt.textContent.trim() : '';
      labelEl.textContent = text;
      /* An option with an empty value is a placeholder ("— None —",
         "All categories"), not a choice the user made. */
      btn.classList.toggle('is-empty', !opt || opt.value === '');
      if (menu) paintMenu();
    }

    function paintMenu() {
      var items = menu.querySelectorAll('.select-option');
      items.forEach(function(item, i) {
        item.setAttribute('aria-selected', i === native.selectedIndex ? 'true' : 'false');
        item.classList.toggle('active', i === active);
      });
    }

    function buildMenu() {
      menu = document.createElement('div');
      menu.className = 'select-menu';
      menu.setAttribute('role', 'listbox');
      menu.id = id + '-menu';
      if (btn.getAttribute('aria-label')) menu.setAttribute('aria-label', btn.getAttribute('aria-label'));

      var opts = options();
      if (!opts.length) {
        menu.innerHTML = '<div class="select-empty">No options</div>';
      } else {
        menu.innerHTML = opts.map(function(opt, i) {
          return '<div class="select-option" role="option" data-i="' + i + '"' +
                 (opt.disabled ? ' aria-disabled="true"' : '') + '>' +
                 '<span class="select-option-text">' + esc(opt.textContent.trim()) + '</span>' +
                 CHECK + '</div>';
        }).join('');
      }

      menu.addEventListener('mousedown', function(e) { e.preventDefault(); });  // keep focus on the button
      menu.addEventListener('click', function(e) {
        var item = e.target.closest('.select-option');
        if (!item || item.getAttribute('aria-disabled') === 'true') return;
        choose(parseInt(item.getAttribute('data-i'), 10));
        close();
      });
      menu.addEventListener('mousemove', function(e) {
        var item = e.target.closest('.select-option');
        if (!item) return;
        active = parseInt(item.getAttribute('data-i'), 10);
        paintMenu();
      });

      document.body.appendChild(menu);
    }

    /* Viewport coordinates, because the menu lives on <body>. Flips
       above the button when there isn't room below it. */
    function place() {
      if (!menu) return;
      var r = btn.getBoundingClientRect();
      var gap = 6;
      /* At least as wide as the button, but free to grow past it —
         a narrow toolbar filter shouldn't wrap "Subcategories only"
         onto two lines. The cap lives in the stylesheet. */
      menu.style.minWidth = Math.max(r.width, 180) + 'px';
      menu.style.left = r.left + 'px';

      var h = menu.offsetHeight;
      var below = window.innerHeight - r.bottom - gap;
      var up = below < h && r.top - gap > below;
      menu.classList.toggle('drop-up', up);
      menu.style.top = (up ? Math.max(gap, r.top - gap - h) : r.bottom + gap) + 'px';
      menu.style.maxHeight = Math.max(120, (up ? r.top : window.innerHeight - r.bottom) - gap * 2) + 'px';

      // Keep it on screen when the button sits near the right edge.
      var over = menu.getBoundingClientRect().right - (window.innerWidth - gap);
      if (over > 0) menu.style.left = Math.max(gap, r.left - over) + 'px';
    }

    function choose(i) {
      if (i < 0 || i >= native.options.length) return;
      if (native.selectedIndex === i) return;
      native.selectedIndex = i;
      paint();
      // Fire the event a real select would, so page code sees the pick.
      native.dispatchEvent(new Event('input', { bubbles: true }));
      native.dispatchEvent(new Event('change', { bubbles: true }));
    }

    function openMenu() {
      if (open === api || btn.disabled) return;
      if (open) open.close();
      buildMenu();
      active = native.selectedIndex;
      paintMenu();
      place();
      // One frame with the menu measured but not yet .open, so the
      // fade/slide has a starting state to animate from.
      requestAnimationFrame(function() {
        if (!menu) return;
        menu.classList.add('open');
        var sel = menu.querySelector('[aria-selected="true"]');
        if (sel) sel.scrollIntoView({ block: 'nearest' });
      });
      btn.setAttribute('aria-expanded', 'true');
      btn.setAttribute('aria-controls', menu.id);
      open = api;
    }

    function close() {
      if (!menu) return;
      var dying = menu;
      menu = null;
      active = -1;
      dying.classList.remove('open');
      setTimeout(function() { dying.remove(); }, 160);
      btn.setAttribute('aria-expanded', 'false');
      btn.removeAttribute('aria-controls');
      if (open === api) open = null;
    }

    function move(delta) {
      var opts = options();
      if (!opts.length) return;
      var i = active < 0 ? native.selectedIndex : active;
      // Skip past disabled options rather than parking the cursor on one.
      for (var step = 0; step < opts.length; step++) {
        i = Math.min(opts.length - 1, Math.max(0, i + delta));
        if (!opts[i].disabled) break;
        if (i === 0 || i === opts.length - 1) break;
      }
      active = i;
      if (menu) {
        paintMenu();
        var el = menu.querySelector('[data-i="' + i + '"]');
        if (el) el.scrollIntoView({ block: 'nearest' });
      } else {
        choose(i);   // closed: arrows change the value in place, like a native select
      }
    }

    function typeAhead(ch) {
      var now = Date.now();
      typed = (now - typedAt < 700 ? typed : '') + ch.toLowerCase();
      typedAt = now;
      var opts = options();
      for (var i = 0; i < opts.length; i++) {
        if (opts[i].textContent.trim().toLowerCase().indexOf(typed) === 0) {
          if (menu) { active = i; paintMenu();
            var el = menu.querySelector('[data-i="' + i + '"]');
            if (el) el.scrollIntoView({ block: 'nearest' });
          } else { choose(i); }
          return;
        }
      }
    }

    btn.addEventListener('click', function() { menu ? close() : openMenu(); });

    btn.addEventListener('keydown', function(e) {
      switch (e.key) {
        case 'ArrowDown': e.preventDefault(); menu ? move(1) : openMenu(); break;
        case 'ArrowUp':   e.preventDefault(); menu ? move(-1) : openMenu(); break;
        case 'Home':      if (menu) { e.preventDefault(); active = 0; paintMenu(); } break;
        case 'End':       if (menu) { e.preventDefault(); active = native.options.length - 1; paintMenu(); } break;
        case 'Enter':
        case ' ':
          e.preventDefault();
          if (menu) { choose(active); close(); } else { openMenu(); }
          break;
        case 'Escape':    if (menu) { e.preventDefault(); close(); } break;
        case 'Tab':       close(); break;
        default:
          if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) {
            e.preventDefault();
            typeAhead(e.key);
          }
      }
    });

    btn.addEventListener('blur', close);

    /* Page code that appends or removes <option> elements after load
       — the admin category form does — should be reflected without
       having to tell us about it. */
    new MutationObserver(function() {
      if (menu) { close(); }
      paint();
    }).observe(native, { childList: true, subtree: true, characterData: true });

    /* Assigning `.value` or `.selectedIndex` fires no event, so a
       programmatic reset would otherwise leave a stale label on the
       button. Patch the two accessors on this instance to repaint. */
    ['value', 'selectedIndex'].forEach(function(prop) {
      var desc = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, prop);
      if (!desc || !desc.set) return;
      Object.defineProperty(native, prop, {
        configurable: true,
        get: function() { return desc.get.call(this); },
        set: function(v) { desc.set.call(this, v); paint(); }
      });
    });

    // A native `change` can still arrive from page code dispatching one.
    native.addEventListener('change', paint);

    var api = { close: close, place: place };
    paint();
  }

  function scan(root) {
    (root || document).querySelectorAll('select:not([data-enhanced])').forEach(enhance);
  }

  /* Close on an outside click, and follow the button if the page
     scrolls or resizes underneath an open menu. */
  document.addEventListener('mousedown', function(e) {
    if (open && !e.target.closest('.select-menu') && !e.target.closest('.select-btn')) open.close();
  });
  window.addEventListener('scroll', function() { if (open) open.place(); }, true);
  window.addEventListener('resize', function() { if (open) open.place(); });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function() { scan(); });
  } else {
    scan();
  }

  // Exposed for anything that renders a <select> after load.
  window.BambeSelect = { scan: scan };

})();
