/* ═══════════════════════════════════════════════════════════════
   Bambe Web — home.js

   The landing page has three data-backed strips: the category
   grid and the two product rows. Everything else on the page —
   hero, promos, app band — is static copy and stays as drawn.

   Both product rows are the same endpoint under a different sort,
   because there is no merchandising model: "Trending" is
   best-selling and "New Arrivals" is newest. Neither is curated,
   and pretending otherwise would mean inventing a slot the admin
   console has no way to fill.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { renderGrid, showSkeleton, hideSkeleton, emptyState } from './catalog.js';

/* ── Category strip ───────────────────────────────────────────
   The wireframe drew a different line-art icon per category. The
   API has no icon field — only an optional image — so the icons
   are kept as a pool and dealt out in order. They are decorative;
   the label under them is the truth.                           */
const CATEGORY_ICONS = [
  '<path d="M8 6h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2Z" stroke="#FAAA39" stroke-width="1.4"/><path d="M10 11h8M10 14h5" stroke="#FAAA39" stroke-width="1.4" stroke-linecap="round"/>',
  '<path d="M14 4C10 4 7 8 7 14s3 10 7 10 7-4 7-10S18 4 14 4Z" stroke="#FAAA39" stroke-width="1.4"/><path d="M10 10h8M10 18h8" stroke="#FAAA39" stroke-width="1.4" stroke-linecap="round"/>',
  '<rect x="6" y="8" width="16" height="12" rx="3" stroke="#FAAA39" stroke-width="1.4"/><path d="M10 8V6a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2" stroke="#FAAA39" stroke-width="1.4"/><circle cx="14" cy="14" r="2" stroke="#FAAA39" stroke-width="1.3"/>',
  '<path d="M6 10h16l-1.5 12h-13L6 10Z" stroke="#FAAA39" stroke-width="1.4" stroke-linejoin="round"/><path d="M11 10V7a3 3 0 0 1 6 0v3" stroke="#FAAA39" stroke-width="1.4"/>',
  '<circle cx="14" cy="14" r="8" stroke="#FAAA39" stroke-width="1.4"/><path d="M14 9v5l3 2" stroke="#FAAA39" stroke-width="1.4" stroke-linecap="round"/>',
  '<path d="M7 21V9l7-4 7 4v12" stroke="#FAAA39" stroke-width="1.4" stroke-linejoin="round"/><path d="M11 21v-6h6v6" stroke="#FAAA39" stroke-width="1.4"/>',
];

function categoryCard(node, index) {
  const card = document.createElement('a');
  card.className = 'category-card';
  card.href = '/listing?category=' + encodeURIComponent(node.slug);
  card.style.textDecoration = 'none';
  card.style.color = 'inherit';

  card.innerHTML =
    '<div class="category-icon">'
      + '<svg width="28" height="28" viewBox="0 0 28 28" fill="none">'
      + CATEGORY_ICONS[index % CATEGORY_ICONS.length]
      + '</svg>'
    + '</div>'
    + '<span class="category-label"></span>'
    + '<span class="category-count"></span>';

  // textContent, not innerHTML — a category name is merchant input.
  card.querySelector('.category-label').textContent = node.name;

  const count = node.product_count || 0;
  card.querySelector('.category-count').textContent =
    count === 1 ? '1 item' : count.toLocaleString() + ' items';

  return card;
}

async function loadCategories() {
  const strip = document.querySelector('.categories-strip .category-grid');
  if (!strip) return;

  const response = await Fetcher.get({ endpoint: '/v1/category/list', showError: false });
  if (!response.ok) return; // the strip keeps whatever it was drawn with

  /* Top level only. The children belong in the listing sidebar, where
     there is room to indent them; flattening them into the strip would
     put "T-Shirts" beside "Men's Fashion" as though they were peers. */
  const categories = response.data || [];
  strip.innerHTML = '';

  if (!categories.length) {
    emptyState(strip, 'No categories yet', 'They appear here once the catalogue has some.');
    return;
  }

  const frag = document.createDocumentFragment();
  categories.forEach((node, index) => frag.appendChild(categoryCard(node, index)));
  strip.appendChild(frag);
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
*/
function productRows() {
  return [...document.querySelectorAll('.products-section .product-grid')];
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

(function init() {
  const [trending, arrivals] = productRows();

  pointViewAllLinks();
  loadCategories();
  loadRow(trending, 'best_selling', 8, 'Nothing trending yet');
  loadRow(arrivals, 'newest', 5, 'No new arrivals yet');
})();
