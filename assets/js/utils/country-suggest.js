/* ═══════════════════════════════════════════════════════════════
   Bambee Web — utils/country-suggest.js

   Country suggestions for a plain text input: type a few letters and
   matching countries appear underneath, with their flag and calling
   code. Arrow keys move through them, Enter or a click picks one,
   Escape closes the list.

   The field keeps storing the country's name ("Bangladesh"), which is
   what every address display and the payment gateway already read.
   Leaving the field with an exact name or a two-letter code typed in
   ("bd", "bangladesh") tidies it to the canonical spelling.

   Pass the phone input too and picking a country starts an empty
   phone number with that country's calling code.

   The country list is dial-codes.js, the same one the sign-up phone
   field uses.
═══════════════════════════════════════════════════════════════ */
import { DIAL_CODES } from '../dial-codes.js';

const MAX_SHOWN = 8;
let uid = 0;

/** The country an exact name or ISO code refers to, or null. */
export function findCountry(value) {
  const q = String(value || '').trim().toLowerCase();
  if (!q) return null;
  return DIAL_CODES.find((c) => c.name.toLowerCase() === q || c.iso.toLowerCase() === q) || null;
}

function matches(query) {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const starts = [];
  const contains = [];
  DIAL_CODES.forEach((c) => {
    const name = c.name.toLowerCase();
    if (name.startsWith(q) || c.iso.toLowerCase() === q) starts.push(c);
    else if (name.includes(q)) contains.push(c);
  });
  return starts.concat(contains).slice(0, MAX_SHOWN);
}

export function attachCountrySuggest(input, { phoneInput } = {}) {
  if (!input || input.dataset.countrySuggest) return;
  input.dataset.countrySuggest = 'on';

  // A positioned wrapper so the list can sit directly under the field.
  const wrap = document.createElement('div');
  wrap.className = 'country-suggest';
  input.parentNode.insertBefore(wrap, input);
  wrap.appendChild(input);

  const listId = 'country-suggest-' + (++uid);
  const list = document.createElement('ul');
  list.className = 'country-suggest-list';
  list.id = listId;
  list.setAttribute('role', 'listbox');
  list.hidden = true;
  wrap.appendChild(list);

  input.setAttribute('autocomplete', 'off');
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-controls', listId);
  input.setAttribute('aria-expanded', 'false');

  let shown = [];
  let active = -1;
  // The prefix this helper last wrote into the phone field, so changing
  // the country can swap it — but never overwrite a number someone typed.
  let autoPrefix = null;

  const close = () => {
    list.hidden = true;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    active = -1;
  };

  const highlight = (index) => {
    active = index;
    [...list.children].forEach((li, i) => {
      const on = i === index;
      li.classList.toggle('active', on);
      li.setAttribute('aria-selected', String(on));
      if (on) {
        input.setAttribute('aria-activedescendant', li.id);
        li.scrollIntoView({ block: 'nearest' });
      }
    });
  };

  const pick = (country) => {
    input.value = country.name;
    input.dispatchEvent(new Event('change', { bubbles: true }));
    if (phoneInput) {
      const phone = phoneInput.value.trim();
      if (!phone || phone === autoPrefix) {
        phoneInput.value = country.dial + ' ';
        autoPrefix = country.dial;
      }
    }
    close();
  };

  const render = () => {
    shown = matches(input.value);
    list.innerHTML = '';
    if (!shown.length) { close(); return; }
    shown.forEach((country, i) => {
      const li = document.createElement('li');
      li.id = listId + '-' + i;
      li.className = 'country-suggest-option';
      li.setAttribute('role', 'option');
      li.setAttribute('aria-selected', 'false');

      const flag = document.createElement('span');
      flag.className = 'country-suggest-flag';
      flag.textContent = country.flag;
      flag.setAttribute('aria-hidden', 'true');
      const name = document.createElement('span');
      name.className = 'country-suggest-name';
      name.textContent = country.name;
      const dial = document.createElement('span');
      dial.className = 'country-suggest-dial';
      dial.textContent = country.dial;
      li.append(flag, name, dial);

      // mousedown, not click: the input's blur would close the list first.
      li.addEventListener('mousedown', (e) => { e.preventDefault(); pick(country); });
      list.appendChild(li);
    });
    list.hidden = false;
    input.setAttribute('aria-expanded', 'true');
    highlight(0);
  };

  input.addEventListener('input', render);
  input.addEventListener('focus', () => { if (input.value.trim()) render(); });
  input.addEventListener('keydown', (e) => {
    if (list.hidden) {
      if (e.key === 'ArrowDown' && input.value.trim()) { e.preventDefault(); render(); }
      return;
    }
    if (e.key === 'ArrowDown') { e.preventDefault(); highlight((active + 1) % shown.length); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); highlight((active - 1 + shown.length) % shown.length); }
    else if (e.key === 'Enter') { e.preventDefault(); if (shown[active]) pick(shown[active]); }
    else if (e.key === 'Escape') { e.preventDefault(); close(); }
    else if (e.key === 'Tab') { if (shown[active] && input.value.trim()) pick(shown[active]); }
  });
  input.addEventListener('blur', () => {
    const exact = findCountry(input.value);
    if (exact && input.value !== exact.name) input.value = exact.name;
    close();
  });
}
