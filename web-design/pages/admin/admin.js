/* ═══════════════════════════════════════════════════════════════
   Bambe Web — admin.js

   The storefront pages each carry a small inline <script>, but the
   six admin pages share the same table / tab / toggle behaviour, so
   it lives in one file rather than being pasted six times.
═══════════════════════════════════════════════════════════════ */
(function() {

  /* ── Storefront nav ─────────────────────────────────────── */
  var searchBtn = document.querySelector('.nav-search-btn');
  if (searchBtn) searchBtn.onclick = function() { location.href = '../shopping/search.html'; };

  var logout = document.querySelector('.admin-nav-logout');
  if (logout) logout.onclick = function() { location.href = '../auth/login.html'; };

  /* ── Filter tabs ────────────────────────────────────────── */
  document.querySelectorAll('.admin-tabs').forEach(function(group) {
    var tabs = group.querySelectorAll('.admin-tab');
    tabs.forEach(function(tab) {
      tab.onclick = function() {
        tabs.forEach(function(t) { t.classList.remove('active'); });
        tab.classList.add('active');
      };
    });
  });

  /* ── Table row selection ────────────────────────────────────
     Header checkbox drives every row; row checkboxes drive the
     header's indeterminate state and the bulk action bar.

     Rows are re-queried on every pass and changes are caught by
     delegation, so a table that grows after load — the category tree
     builds new rows — stays in sync without rewiring.            */
  document.querySelectorAll('[data-selectable]').forEach(function(table) {
    var all = table.querySelector('[data-check-all]');
    // The bulk bar is a sibling of the table's scroll wrapper, so
    // scope the lookup to the surrounding panel.
    var panel = table.closest('.admin-panel');
    var bulk = panel ? panel.querySelector('[data-bulk]') : null;
    var count = bulk ? bulk.querySelector('[data-bulk-count]') : null;

    function rows() { return table.querySelectorAll('[data-row-check]'); }

    function sync() {
      var boxes = rows();
      var checked = 0;
      boxes.forEach(function(box) {
        var tr = box.closest('tr');
        if (box.checked) {
          checked++;
          if (tr) tr.classList.add('selected');
        } else if (tr) {
          tr.classList.remove('selected');
        }
      });

      if (all) {
        all.checked = checked > 0 && checked === boxes.length;
        all.indeterminate = checked > 0 && checked < boxes.length;
      }
      if (bulk) bulk.classList.toggle('visible', checked > 0);
      if (count) count.textContent = checked + ' selected';
    }

    if (all) {
      all.onchange = function() {
        rows().forEach(function(box) { box.checked = all.checked; });
        sync();
      };
    }
    table.addEventListener('change', function(e) {
      if (e.target.matches('[data-row-check]')) sync();
    });
    sync();
  });

  /* ── Support ticket selection ───────────────────────────── */
  var tickets = document.querySelectorAll('.ticket-item');
  tickets.forEach(function(item) {
    item.onclick = function() {
      tickets.forEach(function(t) { t.classList.remove('active'); });
      item.classList.add('active');
    };
  });

  /* Switches are the shared component in js/toggle.js — including
     their keyboard handling — so .toggle is deliberately absent from
     both this file and the parity list below. Two owners would flip
     the class twice and leave it where it started. */

  /* ── Keyboard parity ────────────────────────────────────────
     Several controls above are divs with tabindex rather than
     real buttons, so Enter/Space have to be wired by hand.     */
  document.addEventListener('keydown', function(e) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    var el = document.activeElement;
    if (!el) return;
    if (el.matches('.admin-tab, .ticket-item, .cat-toggle, .admin-nav-logout, .row-btn, .admin-panel-link')) {
      e.preventDefault();
      el.click();
    }
  });

})();
