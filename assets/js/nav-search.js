/*
  The navbar search, on every page that carries the navbar.

  On a phone the bar has no room for a field, so the search is drawn as
  an icon like the wishlist, bell and cart beside it (common.css), and a
  tap opens the field as a full-width row under the bar. The open state
  is `search-open` on the .navbar. The icon is marked aria-expanded while
  open, which also keeps nav-autohide.js from sliding the bar away.

  It also submits: Enter or the search button goes to /search?q=… .
  The listing and search pages filter in place (listing.js) and the
  admin console searches its own tables (admin-core.js), so those pages
  keep their own handlers and get only the phone toggle from here.

  Imported for its side effect, from navbar.js and admin-core.js.
*/
const box = document.querySelector('.navbar .nav-search');
const navbar = box && box.closest('.navbar');
const input = box && box.querySelector('input');
const button = box && box.querySelector('.nav-search-btn');
const phone = window.matchMedia('(max-width: 640px)');

const ownsSearch = /^\/(listing|search|admin)(\/|$)/.test(location.pathname);

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

function submit() {
  const q = input.value.trim();
  location.href = q ? '/search?q=' + encodeURIComponent(q) : '/search';
}

function start() {
  if (!box || !input) return;

  syncRole();
  phone.addEventListener('change', syncRole);

  // The search page is about its field, so there it starts open.
  if (phone.matches && location.pathname === '/search') setOpen(true);

  box.addEventListener('click', (event) => {
    if (!phone.matches) return;
    // Taps inside the open field are typing, not toggling.
    if (input.contains(event.target) || (button && button.contains(event.target))) return;
    isOpen() ? close() : open();
  });
  box.addEventListener('keydown', (event) => {
    if (event.target !== box || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    isOpen() ? close() : open();
  });
  document.addEventListener('click', (event) => {
    if (phone.matches && isOpen() && !box.contains(event.target)) close();
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && isOpen()) { close(); box.focus(); }
  });

  if (!ownsSearch) {
    input.addEventListener('keydown', (event) => { if (event.key === 'Enter') submit(); });
    if (button) button.onclick = submit;
  }
}

start();
