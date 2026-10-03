/*
  Smooth, eased wheel scrolling for the storefront (Lenis, vendored in
  ./vendor/lenis.mjs — MIT). Started from navbar.js, so every storefront
  page has it and the admin console keeps plain native scrolling.

  The page still scrolls natively underneath — Lenis only eases the
  wheel — so sticky bars, scroll listeners and scrollIntoView() all keep
  working. Touch screens keep their own native momentum, and anyone who
  asks their OS for reduced motion gets ordinary scrolling.
*/
import Lenis from './vendor/lenis.mjs';

function start() {
  if (window.lenis) return window.lenis;

  const lenis = new Lenis({
    autoRaf: true,
    // How quickly the page catches up with the wheel: lower is silkier,
    // higher is snappier. 0.1 is Lenis's default and feels natural.
    lerp: 0.1,
    // In-page links (#section) glide too, stopping short of the sticky
    // nav by each target's scroll-margin-top.
    anchors: true,
    // The filter sidebar, dropdowns, the cart drawer and other boxes with
    // their own scrollbar scroll themselves instead of the page.
    allowNestedScroll: true,
  });

  // A modal or the mobile filter sheet locks the page by setting
  // overflow: hidden on <body>; Lenis drives the window directly, so it
  // is paused for as long as that lock is on.
  const syncLock = () => {
    if (document.body.style.overflow === 'hidden') lenis.stop();
    else lenis.start();
  };
  new MutationObserver(syncLock).observe(document.body, {
    attributes: true, attributeFilter: ['style'],
  });
  syncLock();

  window.lenis = lenis;
  return lenis;
}

start();
