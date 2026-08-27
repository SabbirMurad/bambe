/* ═══════════════════════════════════════════════════════════════
   Bambe Web — modal.js

   `common.css` has carried .modal-overlay / .modal / .modal-header
   / .modal-body / .modal-footer since the wireframe, but nothing
   ever opened one — every page that needed a decision used
   window.confirm. Four surfaces need a real one now (write a
   review, request a return, open a support ticket, reply to one),
   so the plumbing lives here once.

   Deliberately small: no stacking, no transitions, no focus trap
   beyond moving focus in and putting it back. Escape and a click
   on the backdrop both close, because a dialog you cannot get out
   of is worse than no dialog.
═══════════════════════════════════════════════════════════════ */

/**
 * @param {string}  options.title    heading text (inserted as text, never markup)
 * @param {Node|string} options.body content for .modal-body
 * @param {Array}   options.actions  [{ label, className, onClick(close), primary }]
 * @param {string}  options.width    overrides the stylesheet's 480px
 * @param {Function} options.onClose  called once, however the dialog was dismissed
 * @returns {{ overlay: HTMLElement, body: HTMLElement, close: Function }}
 */
export function openModal({ title, body, actions = [], width, onClose } = {}) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';

  overlay.innerHTML =
    '<div class="modal" role="dialog" aria-modal="true">'
      + '<div class="modal-header">'
        + '<div class="modal-title"></div>'
        + '<div class="modal-close" role="button" tabindex="0" aria-label="Close" '
          + 'style="cursor:pointer;line-height:0;padding:4px;">'
          + '<svg width="16" height="16" viewBox="0 0 16 16" fill="none">'
          + '<path d="M3 3l10 10M13 3L3 13" stroke="#94908A" stroke-width="1.5" stroke-linecap="round"/></svg>'
        + '</div>'
      + '</div>'
      + '<div class="modal-body"></div>'
    + '</div>';

  const dialog = overlay.querySelector('.modal');
  if (width) dialog.style.maxWidth = width;
  overlay.querySelector('.modal-title').textContent = title || '';

  const bodyEl = overlay.querySelector('.modal-body');
  if (body instanceof Node) bodyEl.appendChild(body);
  else if (typeof body === 'string') bodyEl.innerHTML = body;

  const previouslyFocused = document.activeElement;
  let closed = false;

  function close() {
    if (closed) return;
    closed = true;
    document.removeEventListener('keydown', onKey);
    overlay.remove();
    document.body.style.overflow = '';
    if (previouslyFocused && previouslyFocused.focus) previouslyFocused.focus();
    if (onClose) onClose();
  }

  function onKey(e) {
    if (e.key === 'Escape') close();
  }

  if (actions.length) {
    const footer = document.createElement('div');
    footer.className = 'modal-footer';
    actions.forEach((action) => {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'btn ' + (action.className || 'btn-ghost');
      button.textContent = action.label;
      button.addEventListener('click', () => action.onClick(close, button));
      footer.appendChild(button);
      if (action.primary) action.__el = button;
    });
    dialog.appendChild(footer);
  }

  overlay.querySelector('.modal-close').addEventListener('click', close);
  overlay.querySelector('.modal-close').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); close(); }
  });
  // Backdrop only — a click that started inside the dialog and drifted
  // out on release must not close it.
  overlay.addEventListener('mousedown', (e) => { if (e.target === overlay) close(); });
  document.addEventListener('keydown', onKey);

  document.body.appendChild(overlay);
  document.body.style.overflow = 'hidden';

  const focusable = dialog.querySelector('input, textarea, select, button');
  if (focusable) focusable.focus();

  return { overlay, body: bodyEl, close };
}

/*
  Replaces window.confirm for anything destructive. Same shape as the
  modal above so the two read alike at the call site.
*/
export function confirmModal({ title, message, confirmLabel = 'Confirm', danger = true }) {
  return new Promise((resolve) => {
    const text = document.createElement('p');
    text.style.cssText = 'font-size:13.5px;color:var(--subtle);line-height:1.6;margin:0;';
    text.textContent = message;

    let answered = false;
    const settle = (value, close) => { answered = true; resolve(value); close(); };

    openModal({
      title,
      body: text,
      width: '420px',
      // Escape, the backdrop and the X all read as "no".
      onClose: () => { if (!answered) resolve(false); },
      actions: [
        { label: 'Cancel', className: 'btn-ghost', onClick: (close) => settle(false, close) },
        {
          label: confirmLabel,
          className: danger ? 'btn-danger' : 'btn-primary',
          onClick: (close) => settle(true, close),
        },
      ],
    });
  });
}

/* ── Small form pieces the four composers share ──────────────  */

export function field(label, control, hint) {
  const group = document.createElement('div');
  group.className = 'form-group';
  const labelEl = document.createElement('label');
  labelEl.className = 'form-label';
  labelEl.textContent = label;
  group.appendChild(labelEl);
  group.appendChild(control);
  if (hint) {
    const hintEl = document.createElement('div');
    hintEl.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:6px;';
    hintEl.textContent = hint;
    group.appendChild(hintEl);
  }
  return group;
}

export function textInput({ placeholder = '', value = '', maxLength } = {}) {
  const input = document.createElement('input');
  input.className = 'form-input';
  input.type = 'text';
  input.placeholder = placeholder;
  input.value = value;
  if (maxLength) input.maxLength = maxLength;
  return input;
}

export function textArea({ placeholder = '', rows = 5, value = '' } = {}) {
  const area = document.createElement('textarea');
  area.className = 'form-input';
  area.rows = rows;
  area.placeholder = placeholder;
  area.value = value;
  area.style.resize = 'vertical';
  area.style.height = 'auto';
  area.style.padding = '10px 12px';
  return area;
}

export function selectInput(options, selected) {
  const select = document.createElement('select');
  select.className = 'form-input';
  options.forEach(({ value, label }) => {
    const option = document.createElement('option');
    option.value = value;
    option.textContent = label;
    if (value === selected) option.selected = true;
    select.appendChild(option);
  });
  return select;
}
