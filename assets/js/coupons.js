/* ═══════════════════════════════════════════════════════════════
   Bambe Web — coupons.js

   Page behaviour for the coupon list. admin.js already handles row
   selection, the tabs and the switches; this adds what is specific
   to a discount code.

   Status is never stored. A coupon is disabled, scheduled, expired,
   used up or live purely as a consequence of its switch, its dates
   and its redemption count — so the badge, the tab counts, the
   subtitle and the sidebar badge are all recomputed from the table
   and can't drift apart from the rows they describe.
═══════════════════════════════════════════════════════════════ */
(function() {

  var body = document.getElementById('cp-body');
  if (!body) return;

  var TODAY = new Date();
  TODAY.setHours(0, 0, 0, 0);

  var MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
                'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

  function rows() {
    return Array.prototype.slice.call(body.querySelectorAll('[data-coupon]'));
  }

  function date(iso) {
    var parts = String(iso || '').split('-');
    // Built from parts rather than parsed: a bare "2026-08-01" is read
    // as UTC and can land a day out once the local offset is applied.
    return new Date(+parts[0], +parts[1] - 1, +parts[2]);
  }

  function pretty(iso) {
    var d = date(iso);
    return d.getDate() + ' ' + MONTHS[d.getMonth()] + ' ' + d.getFullYear();
  }

  function days(from, to) {
    return Math.round((to - from) / 86400000);
  }

  /* ── What state is this row in ──────────────────────────── */
  function stateOf(row) {
    var d = row.dataset;
    var on = row.querySelector('.toggle').classList.contains('on');
    var limit = parseInt(d.limit, 10) || 0;
    var used = parseInt(d.used, 10) || 0;
    var start = date(d.start);
    var end = date(d.end);

    if (!on) return { key: 'disabled', label: 'Disabled', tag: 'tag-gray', dim: true };
    if (limit && used >= limit) return { key: 'expired', label: 'Used up', tag: 'tag-red', dim: true };
    if (start > TODAY) {
      return { key: 'scheduled', label: 'Scheduled', tag: 'tag-gold', dim: false,
               note: 'Starts in ' + days(TODAY, start) + ' days' };
    }
    if (end < TODAY) return { key: 'expired', label: 'Expired', tag: 'tag-gray', dim: true };

    var left = days(TODAY, end);
    return { key: 'active', label: 'Active', tag: 'tag-green', dim: false,
             note: left === 0 ? 'Ends today' : 'Ends in ' + left + ' day' + (left === 1 ? '' : 's') };
  }

  /* ── Everything on the page, from the table ─────────────── */
  function sync() {
    var all = rows();
    var counts = { active: 0, scheduled: 0, expired: 0, disabled: 0 };
    var redeemed = 0;

    all.forEach(function(row) {
      var state = stateOf(row);
      counts[state.key]++;
      redeemed += parseInt(row.dataset.used, 10) || 0;

      var badge = row.querySelector('[data-field="status"]');
      badge.textContent = state.label;
      badge.className = 'tag ' + state.tag;

      row.querySelector('[data-field="status-note"]').textContent = state.note || '';
      row.classList.toggle('coupon-off', state.dim);
    });

    var tabs = document.querySelectorAll('.admin-tab-count');
    var order = [all.length, counts.active, counts.scheduled, counts.expired, counts.disabled];
    tabs.forEach(function(tab, i) { tab.textContent = order[i]; });

    document.getElementById('cp-subtitle').textContent =
      counts.active + ' live · ' + counts.scheduled + ' scheduled · ' +
      redeemed.toLocaleString() + ' redemptions all time';

    document.getElementById('cp-foot').textContent =
      'Showing ' + all.length + ' of ' + all.length + ' coupons';

    var badge = document.querySelector('.admin-nav-item.active .admin-nav-count');
    if (badge) badge.textContent = all.length;
  }

  /* A row's switch is what decides enabled/disabled. This listener is
     registered after toggle.js's, so the .on class already reflects
     the new state by the time it runs. */
  document.addEventListener('click', function(e) {
    var toggle = e.target.closest ? e.target.closest('.toggle') : null;
    if (!toggle || !body.contains(toggle)) return;
    sync();
  });

  /* ── Copy a code ─────────────────────────────────────────
     The code is the one thing on the row anyone needs to move
     elsewhere, so the chip hands it over. */
  body.addEventListener('click', function(e) {
    var chip = e.target.closest('[data-copy]');
    if (!chip) return;
    var code = chip.closest('[data-coupon]').dataset.code;
    var label = chip.lastChild;
    var was = label.textContent;

    /* Only claim the copy once it has actually happened. The clipboard
       rejects when the document isn't focused, and an unhandled
       rejection would surface as a page error, so the failure is
       caught and the chip simply stays as it was. */
    if (!navigator.clipboard || !navigator.clipboard.writeText) return;
    navigator.clipboard.writeText(code).then(function() {
      label.textContent = ' Copied ';
      setTimeout(function() { label.textContent = was; }, 1200);
    }, function() {});
  });

  /* ── Create ──────────────────────────────────────────────── */
  var codeEl = document.getElementById('cp-code');
  var typeEl = document.getElementById('cp-type');
  var valueEl = document.getElementById('cp-value');
  var valueHint = document.getElementById('cp-value-hint');
  var minEl = document.getElementById('cp-min');
  var limitEl = document.getElementById('cp-limit');
  var noteEl = document.getElementById('cp-note');
  var startEl = document.getElementById('cp-start');
  var endEl = document.getElementById('cp-end');
  var enabledEl = document.getElementById('cp-enabled');
  var msg = document.getElementById('cp-msg');

  function iso(d) {
    return d.getFullYear() + '-' +
           ('0' + (d.getMonth() + 1)).slice(-2) + '-' +
           ('0' + d.getDate()).slice(-2);
  }

  // A new coupon starting today and running a month is the common
  // case, so the dates open on it rather than on nothing.
  var inAMonth = new Date(TODAY);
  inAMonth.setMonth(inAMonth.getMonth() + 1);
  startEl.value = iso(TODAY);
  endEl.value = iso(inAMonth);

  codeEl.addEventListener('input', function() {
    var caret = codeEl.selectionStart;
    codeEl.value = codeEl.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
    codeEl.setSelectionRange(caret, caret);
  });

  /* Free shipping has no amount to enter, so the field says so
     rather than sitting there inviting a number that is ignored. */
  function refreshType() {
    var free = typeEl.value === 'ship';
    valueEl.disabled = free;
    valueEl.value = free ? '' : valueEl.value;
    valueEl.placeholder = free ? '—' : (typeEl.value === 'fixed' ? '20' : '15');
    valueHint.textContent = free
      ? 'Free shipping carries no amount.'
      : (typeEl.value === 'fixed'
          ? 'Flat amount off the order subtotal, in dollars.'
          : 'Percent off the order subtotal.');
  }
  typeEl.addEventListener('change', refreshType);
  refreshType();

  function say(text, ok) {
    msg.textContent = text;
    msg.style.display = text ? '' : 'none';
    msg.style.color = ok ? 'var(--success)' : 'var(--error)';
  }

  var TYPE_LABEL = { percent: 'Percentage', fixed: 'Fixed amount', ship: 'Free shipping' };
  var TYPE_TAG = { percent: 'tag-blue', fixed: 'tag-gold', ship: 'tag-green' };

  function valueText(kind, value) {
    if (kind === 'percent') return value + '% off';
    if (kind === 'fixed') return '$' + value + ' off';
    return 'Shipping';
  }

  function buildRow(c) {
    var tr = document.createElement('tr');
    tr.setAttribute('data-coupon', '');
    tr.dataset.code = c.code;
    tr.dataset.type = c.type;
    tr.dataset.value = c.value;
    tr.dataset.min = c.min;
    tr.dataset.used = '0';
    tr.dataset.limit = c.limit;
    tr.dataset.start = c.start;
    tr.dataset.end = c.end;

    tr.innerHTML =
      '<td><input type="checkbox" class="admin-check" data-row-check aria-label="Select ' + c.code + '"></td>' +
      '<td><span class="coupon-code" data-copy tabindex="0" role="button" title="Copy code">' +
        '<svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1.2" stroke="currentColor" stroke-width="1.2"/><path d="M8 3.5V2.6a1 1 0 0 0-1-1H2.6a1 1 0 0 0-1 1V7a1 1 0 0 0 1 1h.9" stroke="currentColor" stroke-width="1.2"/></svg>' +
        c.code + '</span><div class="cell-sub"></div></td>' +
      '<td><span class="tag ' + TYPE_TAG[c.type] + '">' + TYPE_LABEL[c.type] + '</span></td>' +
      '<td><span class="coupon-value">' + valueText(c.type, c.value) + '</span>' +
        '<div class="cell-sub">' + (c.min !== '0' ? 'Min spend $' + c.min : 'No minimum') + '</div></td>' +
      '<td class="coupon-usage"><span class="num">0</span> <span class="cell-sub" style="display:inline;">/ ' +
        (c.limit !== '0' ? Number(c.limit).toLocaleString() : '<span class="coupon-usage-none">Unlimited</span>') + '</span>' +
        (c.limit !== '0' ? '<div class="coupon-usage-bar"><div class="coupon-usage-fill" style="width:0%;"></div></div>' : '') +
      '</td>' +
      '<td>' + pretty(c.start) + ' – ' + pretty(c.end) + '<div class="cell-sub" data-field="status-note"></div></td>' +
      '<td><span class="tag" data-field="status">—</span></td>' +
      '<td><div class="toggle' + (c.enabled ? ' on' : '') + '" role="switch" tabindex="0" aria-checked="' + c.enabled + '"></div></td>' +
      '<td class="col-actions"><span class="row-btn" tabindex="0" title="More actions"><svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="3" r="1.3" fill="currentColor"/><circle cx="8" cy="8" r="1.3" fill="currentColor"/><circle cx="8" cy="13" r="1.3" fill="currentColor"/></svg></span></td>';

    tr.querySelector('.cell-sub').textContent = c.note;
    return tr;
  }

  document.getElementById('cp-create').onclick = function() {
    var code = codeEl.value.trim();
    if (!code) { say('Give the coupon a code first.', false); codeEl.focus(); return; }

    var clash = rows().some(function(row) {
      return row.dataset.code.toUpperCase() === code.toUpperCase();
    });
    if (clash) { say(code + ' is already in use — pick another code.', false); codeEl.focus(); return; }

    var type = typeEl.value;
    var value = valueEl.value.trim();
    if (type !== 'ship') {
      if (!value || isNaN(parseFloat(value))) {
        say('Enter the discount amount.', false); valueEl.focus(); return;
      }
      if (type === 'percent' && (parseFloat(value) <= 0 || parseFloat(value) > 100)) {
        say('A percentage has to be between 1 and 100.', false); valueEl.focus(); return;
      }
      value = String(parseFloat(value));
    }

    if (!startEl.value || !endEl.value) { say('Set both a start and an end date.', false); return; }
    if (date(endEl.value) < date(startEl.value)) {
      say('The end date falls before the start date.', false); endEl.focus(); return;
    }

    var tr = buildRow({
      code: code,
      type: type,
      value: value,
      min: String(parseFloat(minEl.value) || 0),
      limit: String(parseInt(limitEl.value, 10) || 0),
      start: startEl.value,
      end: endEl.value,
      note: noteEl.value.trim() || 'No description',
      enabled: enabledEl.classList.contains('on')
    });

    body.appendChild(tr);
    sync();
    tr.scrollIntoView({ behavior: 'smooth', block: 'center' });
    say('Created ' + code + '.', true);

    codeEl.value = '';
    valueEl.value = '';
    minEl.value = '';
    limitEl.value = '';
    noteEl.value = '';
    codeEl.focus();
  };

  document.getElementById('cp-jump-new').onclick = function() {
    document.getElementById('cp-new').scrollIntoView({ behavior: 'smooth', block: 'start' });
    codeEl.focus();
  };

  document.getElementById('cp-export').onclick = function() {
    say('Export queued — ' + rows().length + ' codes will be emailed as CSV.', true);
  };

  /* The copy chip is a span with a role, so Enter/Space is hand-wired
     the same way the other admin controls are. */
  document.addEventListener('keydown', function(e) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    var el = document.activeElement;
    if (el && el.matches && el.matches('.coupon-code')) {
      e.preventDefault();
      el.click();
    }
  });

  sync();

})();
