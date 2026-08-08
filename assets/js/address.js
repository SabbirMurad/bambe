/* ═══════════════════════════════════════════════════════════════
   Bambe Web — address.js

   Behaviour for the checkout address step. The page was drawn with
   every state already painted in — one card marked selected, one
   delivery option marked selected — and nothing wired, so none of it
   could be changed. This makes the three groups real:

     · saved addresses          single-select, add and edit
     · delivery method          single-select, priced
     · packaging                single-select, priced

   Each card carries its own values in data-* attributes, so the form
   can read a card back out to edit it and write it back afterwards
   without a separate model to keep in step.

   The order summary is derived from the selections rather than
   stored, so shipping, packaging and the total can never disagree
   with the options above them.
═══════════════════════════════════════════════════════════════ */
(function() {

  var list = document.getElementById('addr-list');
  if (!list) return;

  var form = document.getElementById('addr-form');
  var formTitle = document.getElementById('addr-form-title');
  var msg = document.getElementById('addr-msg');
  var defaultCheck = document.getElementById('a-default');

  var FIELDS = ['label', 'name', 'street', 'city', 'state', 'zip', 'country', 'phone'];
  var editing = null;   // the card being edited, or null when adding

  function input(field) { return document.getElementById('a-' + field); }

  /* ── Selection ──────────────────────────────────────────────
     One helper for all three groups: they are the same radio
     behaviour wearing different components. */
  function selectIn(group, chosen) {
    group.forEach(function(el) {
      var on = el === chosen;
      el.classList.toggle('selected', on);
      el.setAttribute('aria-checked', on ? 'true' : 'false');
    });
  }

  function cards() {
    return Array.prototype.slice.call(list.querySelectorAll('[data-addr]'));
  }

  list.addEventListener('click', function(e) {
    var edit = e.target.closest('[data-edit]');
    if (edit) {
      // Editing a card shouldn't also select it — those are separate
      // intents, and the click starts on the Edit control.
      e.stopPropagation();
      open(edit.closest('[data-addr]'));
      return;
    }
    var card = e.target.closest('[data-addr]');
    if (card) selectIn(cards(), card);
  });

  /* ── The card's rendered face ───────────────────────────────
     Written from the card's own data-*, so add and edit share one
     path and a card is always consistent with its values. */
  function paint(card) {
    var d = card.dataset;
    card.querySelector('[data-field="label"]').textContent = d.label || 'Address';
    card.querySelector('[data-field="name"]').textContent = d.name;
    card.querySelector('[data-field="phone"]').textContent = d.phone;
    card.querySelector('[data-field="text"]').innerHTML =
      esc(d.street) + '<br>' +
      esc(d.city) + ', ' + esc(d.state) + ' ' + esc(d.zip) + '<br>' +
      esc(d.country);
    card.querySelector('[data-field="badge"]').hidden = !d.default;
  }

  function esc(text) {
    return String(text == null ? '' : text).replace(/[&<>"]/g, function(c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }

  /* Only one address can be the default, so setting one clears the
     rest rather than leaving two badges on screen. */
  function makeDefault(card) {
    cards().forEach(function(c) {
      var on = c === card;
      c.dataset.default = on ? '1' : '';
      c.querySelector('[data-field="badge"]').hidden = !on;
    });
  }

  function blankCard() {
    var card = cards()[0].cloneNode(true);
    card.classList.remove('selected');
    card.setAttribute('aria-checked', 'false');
    return card;
  }

  /* ── The form ───────────────────────────────────────────── */
  function say(text, ok) {
    msg.textContent = text || '';
    msg.classList.toggle('ok', !!ok);
  }

  function setChecked(on) {
    defaultCheck.querySelector('.form-check-input').classList.toggle('checked', on);
    defaultCheck.setAttribute('aria-checked', on ? 'true' : 'false');
  }
  function isChecked() {
    return defaultCheck.querySelector('.form-check-input').classList.contains('checked');
  }

  function open(card) {
    editing = card || null;
    formTitle.textContent = card ? 'Edit address' : 'Add a new address';
    FIELDS.forEach(function(f) {
      var el = input(f);
      var value = card ? (card.dataset[f] || '') : '';
      el.value = value;
      /* A <select> has no blank state: assigning '' leaves it on
         selectedIndex -1, reading back as empty and failing the
         required check. A new address starts on the first option,
         and an unknown saved value falls back to it too. */
      if (el.tagName === 'SELECT' && el.selectedIndex < 0) el.selectedIndex = 0;
    });
    // A default address can't be un-defaulted from here — something
    // has to be the default — so the box is locked while it is.
    var isDefault = !!(card && card.dataset.default);
    setChecked(isDefault);
    defaultCheck.classList.toggle('is-locked', isDefault);
    say('');
    form.hidden = false;
    form.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    input('label').focus();
  }

  function close() {
    form.hidden = true;
    editing = null;
    say('');
  }

  document.getElementById('addr-add').onclick = function() { open(null); };
  document.getElementById('addr-cancel').onclick = close;
  document.getElementById('addr-discard').onclick = close;

  defaultCheck.onclick = function() {
    if (defaultCheck.classList.contains('is-locked')) return;
    setChecked(!isChecked());
  };

  document.getElementById('addr-save').onclick = function() {
    var values = {};
    var missing = null;
    // Label and the second address line are the only optional parts.
    ['name', 'street', 'city', 'state', 'zip', 'country', 'phone'].forEach(function(f) {
      values[f] = input(f).value.trim();
      if (!values[f] && !missing) missing = f;
    });
    if (missing) {
      say('Fill in the ' + missing.replace('zip', 'ZIP') + ' before saving.');
      input(missing).focus();
      return;
    }
    values.label = input('label').value.trim() || 'Address';

    var card = editing || blankCard();
    FIELDS.forEach(function(f) { card.dataset[f] = values[f]; });
    if (!editing) {
      card.dataset.default = '';
      // New cards go in ahead of the add tile, which stays last.
      list.insertBefore(card, document.getElementById('addr-add'));
    }
    paint(card);
    if (isChecked()) makeDefault(card);
    // Whatever you just saved is what you meant to ship to.
    selectIn(cards(), card);

    close();
    card.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  };

  /* ── Delivery and packaging ─────────────────────────────────
     Same radio behaviour as the cards; the difference is that these
     carry a price, so choosing one moves the summary. */
  function priceGroup(id, onPick) {
    var wrap = document.getElementById(id);
    var options = Array.prototype.slice.call(wrap.querySelectorAll('.delivery-option'));
    wrap.addEventListener('click', function(e) {
      var option = e.target.closest('.delivery-option');
      if (!option) return;
      selectIn(options, option);
      onPick();
    });
    return function chosen() {
      return wrap.querySelector('.delivery-option.selected') || options[0];
    };
  }

  var shipEl = document.getElementById('sum-shipping');
  var packRow = document.getElementById('sum-pack-row');
  var packLabel = document.getElementById('sum-pack-label');
  var packEl = document.getElementById('sum-pack');
  var totalEl = document.getElementById('sum-total');

  // Merchandise figures are fixed for the wireframe; only the two
  // choices above move, so the rest is read once from the markup.
  var MERCHANDISE = 213.97 - 21.40 + 15.40;

  var shipping = priceGroup('delivery-options', refresh);
  var packaging = priceGroup('packaging-options', refresh);

  function money(n) { return '$' + n.toFixed(2); }

  function refresh() {
    var ship = parseFloat(shipping().dataset.price) || 0;
    var packOption = packaging();
    var pack = parseFloat(packOption.dataset.price) || 0;

    shipEl.textContent = ship ? money(ship) : 'Free';
    shipEl.style.color = ship ? '' : 'var(--success)';

    packRow.hidden = !pack;
    packLabel.textContent = 'Packaging (' + packOption.dataset.pack + ')';
    packEl.textContent = money(pack);

    totalEl.textContent = money(MERCHANDISE + ship + pack);
  }

  /* Divs standing in for radios and buttons need Enter/Space. */
  document.addEventListener('keydown', function(e) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    var el = document.activeElement;
    if (!el || !el.matches) return;
    if (el.matches('[data-addr], .add-address-card, .delivery-option, [data-edit], .addr-form-close, #a-default')) {
      e.preventDefault();
      el.click();
    }
  });

  refresh();

})();
