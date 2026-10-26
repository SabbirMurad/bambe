/* ═══════════════════════════════════════════════════════════════
   Bambee Web — home.js

   The landing page has three data-backed strips: the category
   doors and the two product rows. Everything else on the page —
   hero, collage, app band — is static copy and stays as drawn.

   Both product rows are the same endpoint under a different sort,
   because there is no merchandising model: "Trending" is
   best-selling and "New Arrivals" is newest. Neither is curated,
   and pretending otherwise would mean inventing a slot the admin
   console has no way to fill.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { renderGrid, showSkeleton, hideSkeleton, emptyState, loadWishlist, paintImage } from './catalog.js';
import { session } from './session.js';
import { toast } from './toast.js';

/* ── Hero parallax ────────────────────────────────────────────
   The drifting light leans a little towards the cursor. The
   photograph deliberately does not — the subject is aligned to the
   logo above her, and moving the picture with the pointer walked her
   off that mark.

   The lean is written to a CSS custom property rather than to
   `transform`, because the orbs are already running their drift
   animations on transform and setting that inline would cancel them.
   The stylesheet composes the two through a translate() that reads
   these values, so they cannot fight.

   Pointer-position work is done on the animation frame — mousemove
   fires far faster than the screen redraws, and doing layout maths
   on every event is how this kind of effect turns into jank.     */
function initHeroParallax() {
  const hero = document.querySelector('.hero-frame');
  if (!hero) return;

  // Whoever asked not to be moved is not moved by this either.
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  // A finger is not a cursor: on touch there is no hover to follow.
  if (!window.matchMedia('(hover: hover) and (pointer: fine)').matches) return;

  let targetX = 0, targetY = 0;   // where the pointer wants it
  let currentX = 0, currentY = 0; // where it actually is
  let frame = null;

  const step = () => {
    // Ease toward the target so the movement trails the cursor
    // instead of snapping to it.
    currentX += (targetX - currentX) * 0.06;
    currentY += (targetY - currentY) * 0.06;
    hero.style.setProperty('--px', currentX.toFixed(2) + 'px');
    hero.style.setProperty('--py', currentY.toFixed(2) + 'px');

    // Stop the loop once it has effectively arrived, so an idle hero
    // is not holding a rAF open for nothing.
    if (Math.abs(targetX - currentX) < 0.1 && Math.abs(targetY - currentY) < 0.1) {
      frame = null;
      return;
    }
    frame = requestAnimationFrame(step);
  };

  const wake = () => { if (frame === null) frame = requestAnimationFrame(step); };

  hero.addEventListener('pointermove', (event) => {
    const rect = hero.getBoundingClientRect();
    // -1 … 1 from the centre of the hero.
    const nx = (event.clientX - rect.left) / rect.width * 2 - 1;
    const ny = (event.clientY - rect.top) / rect.height * 2 - 1;
    targetX = nx * 18;
    targetY = ny * 12;
    wake();
  });

  hero.addEventListener('pointerleave', () => {
    targetX = 0;
    targetY = 0;
    wake();
  });
}

/* ── Newsletter signup ────────────────────────────────────────
   Posts to /v1/newsletter, which takes an address without an account.

   The endpoint answers the same way whether the address is new or
   already on the list, so there is nothing to distinguish here — and
   nothing that would let someone use the form to find out who shops
   here.                                                            */
function initSignup() {
  const form = document.getElementById('signup-form');
  if (!form) return;

  const input = document.getElementById('signup-email');
  const button = document.getElementById('signup-submit');
  const note = document.getElementById('signup-note');
  const resting = note.textContent;

  const say = (message, state) => {
    note.textContent = message;
    note.classList.toggle('is-error', state === 'error');
    note.classList.toggle('is-done', state === 'done');
  };

  form.addEventListener('submit', async (event) => {
    event.preventDefault();

    const email = input.value.trim();
    // Cheap shape check before spending a request on it; the server
    // validates properly either way.
    if (!email || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
      input.classList.add('error');
      say('Enter a valid email address.', 'error');
      input.focus();
      return;
    }
    input.classList.remove('error');

    button.disabled = true;
    const original = button.innerHTML;
    button.textContent = 'Signing up…';

    const response = await Fetcher.post({
      endpoint: '/v1/newsletter',
      body: { email_address: email, source: 'home-footer' },
      showError: false,
    });

    button.disabled = false;
    button.innerHTML = original;

    if (response.ok) {
      form.reset();
      say("Thanks — you're on the list.", 'done');
      toast.success("Thanks — you're on the list");
      // Back to the resting line after a while, so the section does
      // not sit there permanently congratulating the visitor.
      setTimeout(() => say(resting, null), 6000);
    } else {
      /* Show what the server actually said — a generic line hid a real
         validation failure here for several attempts. */
      const reason = (response.data && response.data.message) || response.error;
      say(reason || 'That did not go through. Try again in a moment.', 'error');
    }
  });
}

/* ── Category doors ───────────────────────────────────────────
   Three full-bleed photographic panels in the reference, but the
   count here is whatever the shop actually has — the grid lays out
   however many arrive. Each door takes a photograph through the
   --img custom property; until the merchant sets one, .door-photo
   falls back to its own tint rather than a broken box.

   The wireframe's line-art icons are gone with the card layout they
   belonged to: at this size a photograph is the affordance.     */

function categoryDoor(node) {
  const door = document.createElement('a');
  door.className = 'door';
  door.href = '/listing?category=' + encodeURIComponent(node.slug);

  const photo = document.createElement('div');
  photo.className = 'door-photo';
  /* The category tree returns the banner as `image_id`, a bare id —
     not an `image` object the way the product list does. paintImage()
     accepts either, but it has to be handed the right field. */
  paintImage(photo, node.image_id || node.image, { property: '--img' });
  door.appendChild(photo);

  const body = document.createElement('div');
  body.className = 'door-body';

  const name = document.createElement('div');
  name.className = 'door-name';
  // textContent, not innerHTML — a category name is merchant input.
  name.textContent = node.name;
  body.appendChild(name);

  const count = node.product_count || 0;
  const sub = document.createElement('div');
  sub.className = 'door-sub';
  sub.textContent = count
    ? `Shop ${count === 1 ? '1 piece' : count.toLocaleString() + ' pieces'}`
    : 'Explore the range';
  body.appendChild(sub);

  const cta = document.createElement('span');
  cta.className = 'door-cta';
  cta.textContent = 'Shop ' + node.name;
  body.appendChild(cta);

  door.appendChild(body);
  return door;
}

async function loadCategories() {
  const doors = document.getElementById('category-doors');
  if (!doors) return;

  const response = await Fetcher.get({ endpoint: '/v1/category/list', showError: false });
  if (!response.ok) return; // the section keeps the empty state it was drawn with

  /* Top level only. The children belong in the listing sidebar, where
     there is room to indent them; flattening them into the row would
     put "Shampoo" beside "Hair Care" as though they were peers. */
  const categories = response.data || [];
  if (!categories.length) return;   // the drawn-in empty state already says so

  /* At most six to a row. Past six they wrap, split evenly so the
     last row is never one lone door: 7 is 4 + 3, 8 is 4 + 4, 13 is
     5 + 4 + 4. The grid reads the column count from --door-cols. */
  const rows = Math.ceil(categories.length / 6);
  doors.style.setProperty('--door-cols', String(Math.ceil(categories.length / rows)));

  doors.innerHTML = '';
  const frag = document.createDocumentFragment();
  categories.forEach((node) => frag.appendChild(categoryDoor(node)));
  doors.appendChild(frag);
}

/* ── Product rows ─────────────────────────────────────────────  */
async function loadRow(grid, sort, limit, emptyTitle) {
  if (!grid) return;
  showSkeleton(grid);

  const response = await Fetcher.get({
    endpoint: '/v1/product/list',
    query: { sort, limit },
    showError: false,
  });

  hideSkeleton(grid);

  if (!response.ok) {
    emptyState(grid, "Couldn't load products", 'Please try again in a moment.');
    return;
  }

  renderGrid(grid, response.data.items, {
    emptyTitle,
    emptyDetail: 'Check back once the catalogue fills up.',
  });
}

/*
  Both rows are `.products-section .product-grid`, in document order:
  Trending first, New Arrivals second. Read positionally rather than
  by a new id so the wireframe markup stays untouched.

  Placeholders are excluded: skeleton.js puts a copy of each grid, with
  the same classes, right after it before this runs, and counting it
  made "the second grid" Trending's placeholder — so New Arrivals was
  never filled and kept its shimmer for good.
*/
function productRows() {
  return [...document.querySelectorAll('.products-section .product-grid:not(.skeleton-clone)')];
}

/* The "View all" link beside each row should land on the same
   ordering the row was built from, or it contradicts what it is
   claiming to show more of. */
function pointViewAllLinks() {
  const sections = [...document.querySelectorAll('.products-section')];
  const sorts = ['best_selling', 'newest'];

  sections.forEach((section, index) => {
    const link = section.querySelector('.section-head-link');
    if (link && sorts[index]) link.href = '/listing?sort=' + sorts[index];
  });
}


/* ── Recently viewed ──────────────────────────────────────────
   Every product page has been posting to `/v1/recently-viewed`
   since the detail page was wired, and nothing ever read the
   list back — so the trail was being written into a drawer no
   one opened.

   The section is built here rather than drawn in the markup
   because it has no signed-out and no empty state: a visitor
   with nothing behind them should see the page they saw before,
   not a heading over a blank strip. Appended after the last
   product row so the two positional lookups above are unaffected. */
function recentlySection() {
  const section = document.createElement('section');
  section.className = 'products-section';
  section.innerHTML =
    '<div class="section-head">'
      + '<span class="section-head-title">Recently Viewed</span>'
      + '<a href="/listing" class="section-head-link">Browse all '
      + '<svg width="14" height="14" viewBox="0 0 14 14" fill="none">'
      + '<path d="M3 7h8M7 3l4 4-4 4" stroke="var(--blue)" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>'
      + '</svg></a>'
    + '</div>'
    + '<div class="product-grid product-grid-5"></div>';
  return section;
}

async function loadRecentlyViewed() {
  // The endpoint is `require_access` — there is no guest history to read.
  if (!session.isSignedIn()) return;

  const response = await Fetcher.get({
    endpoint: '/v1/recently-viewed',
    query: { limit: 5 },
    showError: false,
  });
  if (!response.ok || !(response.data || []).length) return;

  // The row is keyed on `product_id`; the card (and the wishlist heart
  // it carries) expects `uuid`.
  const items = response.data.map((item) => ({
    ...item,
    uuid: item.product_id,
    rating_count: item.rating_count || 0,
    in_stock: true,
  }));

  const sections = [...document.querySelectorAll('.products-section')];
  const last = sections[sections.length - 1];
  if (!last) return;

  const section = recentlySection();
  last.after(section);

  await loadWishlist();
  renderGrid(section.querySelector('.product-grid'), items);
}

(function init() {
  const [trending, arrivals] = productRows();

  initHeroParallax();
  initSignup();

  pointViewAllLinks();
  loadCategories();
  loadRow(trending, 'best_selling', 8, 'Nothing trending yet');
  loadRow(arrivals, 'newest', 5, 'No new arrivals yet');
  loadRecentlyViewed();
})();
