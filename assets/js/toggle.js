/* ═══════════════════════════════════════════════════════════════
   Bambee Web — toggle.js

   Behaviour for the .toggle switch. It used to live inside admin.js,
   which meant the component only worked on admin pages — the account
   profile drew the same switch and nothing happened when you clicked
   it. Pulled out here so any page can load the component it uses.

       <div class="toggle on"></div>     on
       <div class="toggle"></div>        off

   The a11y attributes are stamped on at load; the flip itself is
   delegated, so switches rendered after load still respond. Anything
   built at runtime is expected to carry its own role and tabindex.

   Only one script may own this — two listeners would flip the class
   twice and cancel each other out.
═══════════════════════════════════════════════════════════════ */
(function() {

  function mark(toggle) {
    toggle.setAttribute('role', 'switch');
    toggle.setAttribute('tabindex', '0');
    toggle.setAttribute('aria-checked', toggle.classList.contains('on') ? 'true' : 'false');
  }

  function start() {
    document.querySelectorAll('.toggle').forEach(mark);
  }

  document.addEventListener('click', function(e) {
    var toggle = e.target.closest ? e.target.closest('.toggle') : null;
    if (!toggle) return;
    /* Some switches show a state the page has no way to change —
       two-factor is stored on the account but nothing exposes a way to
       set it. Marked data-readonly, they display and don't flip. */
    if (toggle.dataset.readonly === 'true') return;
    toggle.classList.toggle('on');
    toggle.setAttribute('aria-checked', toggle.classList.contains('on') ? 'true' : 'false');
  });

  /* It is a div, not a real checkbox, so Enter and Space have to be
     wired by hand to match what a switch is expected to do. */
  document.addEventListener('keydown', function(e) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    var el = document.activeElement;
    if (!el || !el.matches || !el.matches('.toggle')) return;
    e.preventDefault();
    el.click();
  });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }

})();
