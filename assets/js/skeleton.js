/* ═══════════════════════════════════════════════════════════════
   Bambee Web — skeleton.js

   Loading placeholders for anything the real product would fetch:
   product grids, admin tables, order and notification lists, the
   dashboard tiles and charts.

   Mark a container with data-skeleton and it gets one. The
   placeholder is not written by hand — it is derived from the
   container itself: the real subtree is cloned, then every piece of
   text, image, icon and control inside the clone is measured and
   swapped for a shimmer bar of the same size. The card chrome —
   borders, radii, padding, column tracks — is left alone, because
   that outline is what makes a skeleton read as the thing it is
   standing in for.

   Deriving it this way means the placeholder cannot drift out of
   step with the markup, and no page has to carry a second copy of
   its own layout.

       <div class="product-grid" data-skeleton>…</div>
       <tbody data-skeleton>…</tbody>

   The markup inside a target is wireframe sample content — it exists
   to give the placeholder its shape, never to be read. So it is
   never shown: common.css keeps every target invisible until it is
   loaded, the placeholder takes its place as soon as this script
   runs, and the target only comes back when the page module that
   owns it has real data in it and calls Skeleton.clear(target).
   There is no timer. A timer used to end the shimmer after ~1.4s,
   which on any slower request revealed the sample content — fake
   names, fake totals — until the real data arrived.
═══════════════════════════════════════════════════════════════ */
(function() {

  /* Live placeholders, keyed by the element each one stands in for,
     so a page module can clear its own without touching the others. */
  var mounted = new Map();

  /* Elements with no text to strip — they are replaced outright by a
     box of the same size rather than walked into. */
  var REPLACE = {
    svg: 1, img: 1, canvas: 1, video: 1, picture: 1,
    input: 1, select: 1, textarea: 1, button: 1
  };

  /* The neutral surfaces a placeholder is allowed to keep. Anything
     painted in something else is carrying brand colour — a status
     pill, a selected chip, an avatar — and gets flattened. Resolved
     to rgb() once, since that is the form computed styles come in. */
  var neutrals = null;
  function isNeutral(colour) {
    if (!neutrals) {
      neutrals = {};
      var probe = document.createElement('div');
      probe.style.cssText = 'position:absolute;visibility:hidden';
      document.body.appendChild(probe);
      ['--white', '--surface', '--input-bg', '--border'].forEach(function(token) {
        probe.style.backgroundColor = 'var(' + token + ')';
        neutrals[getComputedStyle(probe).backgroundColor] = 1;
      });
      probe.remove();
    }
    // Fully transparent means it inherits whatever is behind it.
    return colour === 'rgba(0, 0, 0, 0)' || colour === 'transparent' || !!neutrals[colour];
  }

  /* Paint the shimmer inline, because a component's own rule can be
     more specific than .skeleton and would otherwise keep its colour.
     Only the two longhands — the `background` shorthand would reset
     background-size along with them, and the sweep is sized in
     percentages of the element. */
  function shimmer(el) {
    /* Clear first: a thumbnail arrives carrying style="background:…"
       from the markup, and that shorthand pins background-size and
       -repeat inline where the stylesheet can't reach them. */
    el.style.background = '';
    el.style.backgroundColor = 'transparent';
    el.style.backgroundImage = 'var(--skeleton-fill)';
  }

  function box(rect) {
    var el = document.createElement('span');
    el.className = 'skeleton skeleton-box';
    el.style.width = rect.width + 'px';
    el.style.height = rect.height + 'px';
    return el;
  }

  /* One bar per rendered line, the last one short — the ragged edge
     is what stops a paragraph placeholder reading as a solid slab. */
  function bars(rect, cs) {
    var lh = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.4;
    var lines = Math.max(1, Math.min(4, Math.round(rect.height / lh)));
    var thick = Math.max(8, Math.min(15, parseFloat(cs.fontSize) * 0.72));

    var wrap = document.createElement('span');
    wrap.className = 'skeleton-text';
    wrap.style.width = Math.max(28, rect.width) + 'px';
    for (var i = 0; i < lines; i++) {
      var bar = document.createElement('i');
      bar.className = 'skeleton';
      bar.style.height = thick + 'px';
      if (lines > 1 && i === lines - 1) bar.style.width = '62%';
      wrap.appendChild(bar);
    }
    return wrap;
  }

  function strip(el) {
    el.removeAttribute('id');
    el.removeAttribute('tabindex');
    el.removeAttribute('href');
    /* Other scripts on the page find their targets by data-attribute
       — admin.js counts [data-row-check] rows, for one. A clone that
       kept them would join those queries as a phantom. */
    Array.prototype.slice.call(el.attributes).forEach(function(a) {
      if (a.name.indexOf('data-') === 0) el.removeAttribute(a.name);
    });
  }

  /* orig is laid out and measurable; copy is its counterpart in the
     detached clone. Walk them together and blank the copy out. */
  function paint(orig, copy) {
    var tag = orig.tagName.toLowerCase();
    var rect = orig.getBoundingClientRect();

    // Before any early return: a copy that keeps its id or its data
    // hooks is a duplicate the rest of the page can trip over.
    strip(copy);

    /* Not rendered — a collapsed table row, a hidden input. It has no
       place in the placeholder, and dropping it also takes its whole
       subtree, which the walk would otherwise never reach to clean. */
    if (!rect.width || !rect.height) {
      if (copy.parentNode) copy.parentNode.removeChild(copy);
      return;
    }

    if (REPLACE[tag]) {
      copy.parentNode.replaceChild(box(rect), copy);
      return;
    }

    // A leaf that draws its own block — a rule, an avatar circle.
    // Keep its geometry, shimmer its surface.
    if (!orig.firstChild) {
      copy.classList.add('skeleton');
      shimmer(copy);
      copy.style.borderColor = 'transparent';
      return;
    }

    var own = getComputedStyle(orig);

    /* A picture area: a gradient or image standing in for a photo.
       It usually has children — a sale badge, a wishlist button — so
       it can't be caught as a leaf, but the artwork still has to go.
       Shimmer the surface and carry on into the children. */
    if (own.backgroundImage !== 'none') {
      copy.classList.add('skeleton');
      shimmer(copy);
    } else if (!isNeutral(own.backgroundColor)) {
      copy.style.background = 'var(--input-bg)';
    }

    /* Nothing in a placeholder should be carrying an accent — a
       selected size chip, a wishlist button outlined in red. Any
       border it has flattens to the ordinary hairline. */
    if (parseFloat(own.borderTopWidth) || parseFloat(own.borderLeftWidth)) {
      copy.style.borderColor = 'var(--border)';
    }

    /* Snapshots, not the live lists: the walk replaces and removes
       nodes as it goes, and a live NodeList would renumber underneath
       it — pairing each original with the wrong copy from there on. */
    var kids = Array.prototype.slice.call(orig.childNodes);
    var copies = Array.prototype.slice.call(copy.childNodes);
    for (var i = 0; i < kids.length; i++) {
      var o = kids[i], c = copies[i];
      if (!c) break;
      if (o.nodeType === 3) {
        if (!o.data.trim()) continue;
        // Loose text beside an element — measured through a range,
        // since a text node has no box of its own to ask.
        var range = document.createRange();
        range.selectNodeContents(o);
        copy.replaceChild(bars(range.getBoundingClientRect(), getComputedStyle(orig)), c);
      } else if (o.nodeType === 1) {
        paint(o, c);
      }
    }

    // An element holding only text keeps its padding and gets bars.
    if (orig.children.length === 0 && orig.textContent.trim()) {
      var cs = getComputedStyle(orig);
      var inner = rect.width - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      copy.textContent = '';
      copy.appendChild(bars({ width: inner, height: rect.height }, cs));
    }
  }

  /* Targets a page module cleared before this script reached them —
     data that arrived first. They must not get a placeholder over
     what is already real. */
  var cleared = new WeakSet();

  function mount(target) {
    if (mounted.has(target) || cleared.has(target)) return;
    var clone = target.cloneNode(true);
    paint(target, clone);
    clone.classList.add('skeleton-clone');
    clone.setAttribute('aria-hidden', 'true');
    strip(clone);
    // A nested target copied into this clone is a picture of one, not
    // a target: it must not be hidden as unloaded or mounted itself.
    clone.querySelectorAll('[data-skeleton]').forEach(function(el) {
      el.removeAttribute('data-skeleton');
    });

    /*
      After the target, not before it. The clone is a full copy —
      same classes, same structure — so while it is mounted every
      `querySelector` on the page can match it. Placed first, it
      shadowed the real element, and a page module that filled in
      the product title wrote into the placeholder instead; the
      swap back then "restored" the wireframe's dummy text over
      live data. The target is display:none either way, so the two
      orderings look identical.
    */
    target.parentNode.insertBefore(clone, target.nextSibling);
    // Inline display so the swap back restores whatever the
    // stylesheet had — grid, table-row-group, flex.
    target.style.display = 'none';
    target.setAttribute('aria-busy', 'true');

    mounted.set(target, { clone: clone });
  }

  /* Idempotent. Called by a page module the moment its data lands —
     and safe to call before the placeholder exists, in which case the
     target is simply marked loaded and never gets one. */
  function unmount(target) {
    var entry = mounted.get(target);
    if (!entry) {
      cleared.add(target);
      target.classList.add('is-loaded');
      return;
    }

    mounted.delete(target);
    if (entry.clone.parentNode) entry.clone.parentNode.removeChild(entry.clone);

    target.style.display = '';
    target.removeAttribute('aria-busy');
    target.classList.add('is-loaded');
  }

  function start() {
    document.querySelectorAll('[data-skeleton]').forEach(mount);
  }

  /* Whoever gets the response calls `clear` with the target it filled.
     Pages that fail to load should still call it, after drawing their
     error state, so the shimmer does not run forever. */
  window.Skeleton = {
    clear: function(target) {
      if (!target) return;
      unmount(target);
    },
  };

  /* This script is loaded at the end of <body>, so every target above
     it is already parsed and laid out (invisibly) — mount them now,
     before the first paint, and again at DOMContentLoaded for any that
     come later. mount() skips targets it has already handled. */
  start();
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  }

})();
