/*
  The navbar search, on every page that carries the navbar.

  On a phone the bar has no room for a field, so the search is drawn as
  an icon like the wishlist, bell and cart beside it (common.css), and a
  tap opens the field as a full-width row under the bar. The open state
  is `search-open` on the .navbar. The icon is marked aria-expanded while
  open, which also keeps nav-autohide.js from sliding the bar away.

  In the shop it is also a live search. A moment after typing stops, a
  dropdown under the field lists the products that match, with a link to
  every result; empty, it lists the shopper's recent searches instead
  (kept in this browser). Enter or the button goes to /search?q=… .

  The listing and search pages filter their own grid as you type
  (listing.js), so there the dropdown only offers recent searches; the
  admin console searches its own tables (admin-core.js) and gets only
  the phone toggle from here.

  Imported for its side effect, from navbar.js and admin-core.js.
*/
import { Fetcher } from './fetcher.js';

const box = document.querySelector('.navbar .nav-search');
const navbar = box && box.closest('.navbar');
const input = box && box.querySelector('input');
const button = box && box.querySelector('.nav-search-btn');
const phone = window.matchMedia('(max-width: 640px)');

const isAdmin = /^\/admin(\/|$)/.test(location.pathname);
const filtersInPlace = /^\/(listing|search)(\/|$)/.test(location.pathname);

// Long enough not to fire on every key, short enough to feel live.
const TYPING_PAUSE_MS = 300;
const MIN_CHARS = 2;
const SUGGESTIONS = 6;

const isOpen = () => navbar.classList.contains('search-open');

function setOpen(open) {
  navbar.classList.toggle('search-open', open);
  if (phone.matches) box.setAttribute('aria-expanded', String(open));
}

function open() {
  setOpen(true);
  // Focused inside the tap itself, or iOS will not raise the keyboard.
  input.focus();
}

function close() {
  setOpen(false);
  hideDropdown();
  if (box.contains(document.activeElement)) document.activeElement.blur();
}

// Only a phone sees the search as a button, so only a phone is told so.
function syncRole() {
  if (phone.matches) {
    box.setAttribute('role', 'button');
    box.setAttribute('aria-label', 'Search');
    box.setAttribute('aria-expanded', String(isOpen()));
    box.tabIndex = 0;
  } else {
    ['role', 'aria-label', 'aria-expanded', 'tabindex'].forEach((a) => box.removeAttribute(a));
    navbar.classList.remove('search-open');
  }
}

/* ── Recent searches ──────────────────────────────────────────
   Kept in this browser only — a convenience, not a record. Newest
   first, no repeats (ignoring case), at most HISTORY_MAX.        */
const HISTORY_KEY = 'bambe.search_history';
const HISTORY_MAX = 8;

function readHistory() {
  try {
    const list = JSON.parse(localStorage.getItem(HISTORY_KEY) || '[]');
    return Array.isArray(list) ? list.filter((t) => typeof t === 'string' && t.trim()) : [];
  } catch (_) {
    return [];
  }
}

function writeHistory(list) {
  try { localStorage.setItem(HISTORY_KEY, JSON.stringify(list.slice(0, HISTORY_MAX))); } catch (_) { /* private mode */ }
}

function remember(term) {
  const t = term.trim();
  if (!t) return;
  writeHistory([t, ...readHistory().filter((h) => h.toLowerCase() !== t.toLowerCase())]);
}

function forget(term) {
  writeHistory(readHistory().filter((h) => h !== term));
}

function submit(term = input.value) {
  const q = term.trim();
  remember(q);
  location.href = q ? '/search?q=' + encodeURIComponent(q) : '/search';
}

/* ── Dropdown ─────────────────────────────────────────────────  */
let dropdown = null;
let items = [];          // the selectable rows, in order
let active = -1;         // index into items, or -1
let requestSeq = 0;
let typingTimer = null;
let catalog = null;      // money / paintImage, loaded on first use

function ensureDropdown() {
  if (dropdown) return dropdown;
  dropdown = document.createElement('div');
  dropdown.className = 'nav-search-drop';
  dropdown.id = 'nav-search-drop';
  dropdown.setAttribute('role', 'listbox');
  dropdown.hidden = true;
  // Mousedown, not click: a click would blur the field first and close it.
  dropdown.addEventListener('mousedown', (e) => e.preventDefault());
  box.appendChild(dropdown);
  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-controls', dropdown.id);
  input.setAttribute('aria-expanded', 'false');
  return dropdown;
}

function hideDropdown() {
  if (!dropdown) return;
  dropdown.hidden = true;
  input.setAttribute('aria-expanded', 'false');
  input.removeAttribute('aria-activedescendant');
  items = [];
  active = -1;
}

function showDropdown() {
  ensureDropdown();
  dropdown.hidden = !dropdown.childElementCount;
  input.setAttribute('aria-expanded', String(!dropdown.hidden));
}

function setActive(index) {
  items.forEach((el, i) => el.classList.toggle('active', i === index));
  active = index;
  if (index >= 0) {
    input.setAttribute('aria-activedescendant', items[index].id);
    items[index].scrollIntoView({ block: 'nearest' });
  } else {
    input.removeAttribute('aria-activedescendant');
  }
}

function heading(text, action) {
  const row = document.createElement('div');
  row.className = 'nav-search-drop-head';
  const label = document.createElement('span');
  label.textContent = text;
  row.appendChild(label);
  if (action) row.appendChild(action);
  return row;
}

function option(className, onPick) {
  const el = document.createElement('a');
  el.className = 'nav-search-drop-item ' + className;
  el.id = 'nav-search-opt-' + items.length;
  el.setAttribute('role', 'option');
  el.addEventListener('click', (e) => { e.preventDefault(); onPick(); });
  items.push(el);
  return el;
}

const CLOCK = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="6.2" stroke="currentColor" stroke-width="1.4"/><path d="M8 4.8V8l2.2 1.4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>';

function historyRow(term) {
  const el = option('is-history', () => submit(term));
  el.href = '/search?q=' + encodeURIComponent(term);
  el.innerHTML = CLOCK;
  const text = document.createElement('span');
  text.className = 'nav-search-drop-text';
  text.textContent = term;
  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'nav-search-drop-remove';
  remove.setAttribute('aria-label', 'Remove “' + term + '” from recent searches');
  remove.textContent = '×';
  remove.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    forget(term);
    renderHistory();
    input.focus();
  });
  el.append(text, remove);
  return el;
}

function renderHistory() {
  ensureDropdown();
  dropdown.innerHTML = '';
  items = [];
  active = -1;
  const history = readHistory();
  if (history.length) {
    const clear = document.createElement('button');
    clear.type = 'button';
    clear.className = 'nav-search-drop-clear';
    clear.textContent = 'Clear';
    clear.addEventListener('click', () => { writeHistory([]); renderHistory(); input.focus(); });
    dropdown.appendChild(heading('Recent searches', clear));
    history.forEach((term) => dropdown.appendChild(historyRow(term)));
  }
  showDropdown();
}

async function renderResults(term) {
  const seq = ++requestSeq;
  ensureDropdown();
  if (!catalog) catalog = await import('./catalog.js');

  const response = await Fetcher.get({
    endpoint: '/v1/product/list',
    query: { q: term, limit: SUGGESTIONS },
    showError: false,
  });
  // A slower answer to an older query must not replace a newer one.
  if (seq !== requestSeq || input.value.trim() !== term) return;

  dropdown.innerHTML = '';
  items = [];
  active = -1;

  // Earlier searches that contain what's being typed, above the products.
  const lower = term.toLowerCase();
  const past = readHistory().filter((h) => h.toLowerCase().includes(lower) && h.toLowerCase() !== lower).slice(0, 3);
  past.forEach((h) => dropdown.appendChild(historyRow(h)));

  const products = response.ok ? (response.data.items || []) : [];
  if (products.length) {
    dropdown.appendChild(heading('Products'));
    products.forEach((p) => {
      const url = '/product/' + encodeURIComponent(p.slug);
      const el = option('is-product', () => { remember(term); location.href = url; });
      el.href = url;
      const thumb = document.createElement('span');
      thumb.className = 'nav-search-drop-thumb';
      if (p.image) catalog.paintImage(thumb, p.image);
      const text = document.createElement('span');
      text.className = 'nav-search-drop-text';
      const title = document.createElement('span');
      title.className = 'nav-search-drop-title';
      title.textContent = p.title;
      if (p.status === 'Draft') {
        const tag = document.createElement('span');
        tag.className = 'nav-search-drop-draft';
        tag.textContent = 'Draft';
        title.appendChild(tag);
      }
      text.appendChild(title);
      if (p.brand && p.brand.name) {
        const brand = document.createElement('span');
        brand.className = 'nav-search-drop-sub';
        brand.textContent = p.brand.name;
        text.appendChild(brand);
      }
      const price = document.createElement('span');
      price.className = 'nav-search-drop-price';
      price.textContent = catalog.money(p.price_min);
      el.append(thumb, text, price);
      dropdown.appendChild(el);
    });
  } else {
    const none = document.createElement('div');
    none.className = 'nav-search-drop-empty';
    none.textContent = response.ok ? 'No products match “' + term + '”.' : 'Search isn’t answering right now.';
    dropdown.appendChild(none);
  }

  const all = option('is-all', () => submit(term));
  all.href = '/search?q=' + encodeURIComponent(term);
  all.textContent = products.length
    ? 'See all results for “' + term + '”'
    : 'Search for “' + term + '”';
  dropdown.appendChild(all);
  showDropdown();
}

function onTyping() {
  clearTimeout(typingTimer);
  const term = input.value.trim();
  if (!term) { renderHistory(); return; }
  if (filtersInPlace) { hideDropdown(); return; }   // listing.js filters the grid
  if (term.length < MIN_CHARS) { hideDropdown(); return; }
  typingTimer = setTimeout(() => renderResults(term), TYPING_PAUSE_MS);
}

function onKeys(e) {
  const showing = dropdown && !dropdown.hidden && items.length;
  if (e.key === 'ArrowDown' && showing) {
    e.preventDefault();
    setActive((active + 1) % items.length);
  } else if (e.key === 'ArrowUp' && showing) {
    e.preventDefault();
    setActive(active <= 0 ? items.length - 1 : active - 1);
  } else if (e.key === 'Enter') {
    if (showing && active >= 0) {
      e.preventDefault();
      items[active].click();
    } else if (filtersInPlace) {
      // listing.js runs the search; only the history is ours.
      remember(input.value);
      hideDropdown();
    } else {
      submit();
    }
  } else if (e.key === 'Escape' && dropdown && !dropdown.hidden) {
    e.stopPropagation();   // close the list first, the phone row on a second press
    hideDropdown();
  }
}

function start() {
  if (!box || !input) return;

  syncRole();
  phone.addEventListener('change', syncRole);

  // The search page is about its field, so there it starts open.
  if (phone.matches && location.pathname === '/search') setOpen(true);

  box.addEventListener('click', (event) => {
    if (!phone.matches) return;
    // Taps inside the open field, its button or its dropdown are not toggles.
    if (input.contains(event.target) || (button && button.contains(event.target))
      || (dropdown && dropdown.contains(event.target))) return;
    isOpen() ? close() : open();
  });
  box.addEventListener('keydown', (event) => {
    if (event.target !== box || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    isOpen() ? close() : open();
  });
  document.addEventListener('click', (event) => {
    if (box.contains(event.target)) return;
    hideDropdown();
    if (phone.matches && isOpen()) close();
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && isOpen()) { close(); box.focus(); }
  });

  if (isAdmin) return;

  input.setAttribute('autocomplete', 'off');
  input.addEventListener('input', onTyping);
  input.addEventListener('focus', () => { if (!input.value.trim()) renderHistory(); });
  input.addEventListener('keydown', onKeys);
  if (button) {
    // listing.js owns the button where the grid filters in place;
    // adding a listener rather than replacing its handler keeps both.
    if (filtersInPlace) button.addEventListener('click', () => remember(input.value));
    else button.onclick = () => submit();
  }
}

start();
