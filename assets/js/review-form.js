/* ═══════════════════════════════════════════════════════════════
   Bambe Web — review-form.js

   `POST /v1/review` has been live the whole time with nothing
   calling it: the product page's "Write a Review" button used to
   send people to their orders, and the orders page's own button
   was wireframe markup. So a customer could not review anything.

   Two surfaces open the same composer, which is why it lives here:

   - the product page, which knows the product but has to go and
     find whether the caller ever bought it;
   - the orders page, which knows the order and asks which of its
     lines is being reviewed.

   `order_id` is what earns the "Verified Purchase" badge. The
   backend re-checks it against the caller's own orders
   (review/create.rs), so sending one we cannot prove just loses
   the badge — it never fakes it.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { openModal, field, textInput, textArea } from './modal.js';
import { imageUrl } from './catalog.js';

const STAR_PATH = 'M6 1l1.1 3.4H11L8.2 6.5l1 3.4L6 8.1 2.8 9.9l1-3.4L1 4.4h3.9L6 1Z';
const RATING_WORD = ['', 'Poor', 'Fair', 'Good', 'Very good', 'Excellent'];

/* ── Star picker ──────────────────────────────────────────────
   Keyboard-operable, because it is the one required field and a
   mouse-only control makes the form impossible to complete from
   the keyboard.                                                */
function starPicker(onChange) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'display:flex;align-items:center;gap:10px;';

  const row = document.createElement('div');
  row.className = 'stars';
  row.setAttribute('role', 'radiogroup');
  row.setAttribute('aria-label', 'Rating');
  row.style.cssText = 'display:flex;gap:4px;cursor:pointer;';

  const word = document.createElement('span');
  word.style.cssText = 'font-size:13px;color:var(--subtle);';

  let value = 0;

  const paint = (upTo) => {
    [...row.children].forEach((star, index) => {
      star.querySelector('path').setAttribute('fill', index < upTo ? '#FAAA39' : '#E8E5E0');
    });
    word.textContent = RATING_WORD[upTo] || '';
  };

  const select = (i) => {
    value = i;
    paint(i);
    [...row.children].forEach((star, index) => {
      star.setAttribute('aria-checked', String(index + 1 === i));
      star.setAttribute('tabindex', index + 1 === i ? '0' : '-1');
    });
    onChange(i);
  };

  for (let i = 1; i <= 5; i++) {
    const star = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    star.setAttribute('viewBox', '0 0 12 12');
    star.setAttribute('width', '24');
    star.setAttribute('height', '24');
    star.setAttribute('role', 'radio');
    star.setAttribute('tabindex', i === 1 ? '0' : '-1');
    star.setAttribute('aria-label', i === 1 ? '1 star' : i + ' stars');
    star.setAttribute('aria-checked', 'false');
    star.innerHTML = '<path d="' + STAR_PATH + '" fill="#E8E5E0"/>';

    star.addEventListener('click', () => select(i));
    star.addEventListener('mouseenter', () => paint(i));
    star.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); select(i); }
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        e.preventDefault();
        const next = Math.min(5, Math.max(1, (value || 1) + (e.key === 'ArrowRight' ? 1 : -1)));
        select(next);
        row.children[next - 1].focus();
      }
    });

    row.appendChild(star);
  }

  row.addEventListener('mouseleave', () => paint(value));

  wrap.appendChild(row);
  wrap.appendChild(word);
  return { el: wrap, get value() { return value; } };
}

/* ── The composer ─────────────────────────────────────────────  */

/**
 * @param {string} options.productId
 * @param {string} options.productTitle  shown at the top, so a picker's choice stays visible
 * @param {string} [options.orderId]     earns the verified badge when it checks out
 * @param {string} [options.variantId]
 * @param {*}      [options.image]       ImageStruct or bare id
 * @param {Function} [options.onDone]
 */
export function openReviewComposer({ productId, productTitle, orderId, variantId, image, onDone }) {
  if (!session.isSignedIn()) {
    toast.info('Sign in to write a review');
    setTimeout(() => session.requireSignedIn(), 700);
    return;
  }

  const form = document.createElement('div');

  if (productTitle) {
    const head = document.createElement('div');
    head.style.cssText = 'display:flex;gap:12px;align-items:center;padding-bottom:16px;'
      + 'margin-bottom:18px;border-bottom:1px solid var(--border);';

    const thumb = document.createElement('div');
    thumb.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
      + 'background:var(--input-bg);flex-shrink:0;';
    const url = imageUrl(image);
    if (url) thumb.style.background = "url('" + url + "') center / cover no-repeat";

    const name = document.createElement('div');
    name.style.cssText = 'font-size:13.5px;font-weight:600;color:var(--dark);line-height:1.4;';
    name.textContent = productTitle;

    head.appendChild(thumb);
    head.appendChild(name);
    form.appendChild(head);
  }

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;margin-top:-6px;';

  const picker = starPicker(() => { error.style.display = 'none'; });
  form.appendChild(field('Your rating', picker.el));

  const title = textInput({ placeholder: 'Sum it up in a few words (optional)', maxLength: 120 });
  form.appendChild(field('Headline', title));

  const body = textArea({ placeholder: 'What did you like, or not like, about it?', rows: 5 });
  form.appendChild(field('Your review', body,
    'Reviews are published once a moderator has read them.'));

  form.appendChild(error);

  const { close } = openModal({
    title: 'Write a review',
    body: form,
    width: '520px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (done) => done() },
      {
        label: 'Submit review',
        className: 'btn-primary',
        primary: true,
        onClick: async (done, button) => {
          error.style.display = 'none';

          if (!picker.value) {
            error.textContent = 'Pick a rating first.';
            error.style.display = '';
            return;
          }
          if (!body.value.trim()) {
            error.textContent = 'Tell other shoppers what you thought.';
            error.style.display = '';
            return;
          }

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/review',
            body: {
              product_id: productId,
              order_id: orderId || null,
              variant_id: variantId || null,
              rating: picker.value,
              title: title.value.trim() || null,
              body: body.value.trim(),
            },
          });
          button.disabled = false;

          if (!response.ok) return;

          done();
          // It lands as ReviewStatus::Pending, so promising it will show
          // up on the product page straight away would be a lie.
          toast.success('Thanks — your review is with our moderators');
          if (onDone) onDone();
        },
      },
    ],
  });

  return close;
}

/*
  Asks which line of an order is being reviewed. A one-line order
  skips the question entirely.
*/
export function openOrderReviewPicker(order, onPick) {
  const items = order.items || [];
  if (!items.length) {
    toast.info('Nothing on this order to review');
    return;
  }
  if (items.length === 1) {
    onPick(items[0]);
    return;
  }

  const list = document.createElement('div');
  list.style.cssText = 'display:flex;flex-direction:column;gap:8px;';

  items.forEach((item) => {
    const row = document.createElement('div');
    row.setAttribute('role', 'button');
    row.tabIndex = 0;
    row.style.cssText = 'display:flex;gap:12px;align-items:center;padding:10px;cursor:pointer;'
      + 'border:1px solid var(--border);border-radius:var(--radius-sm);';

    const thumb = document.createElement('div');
    thumb.style.cssText = 'width:40px;height:40px;border-radius:var(--radius-sm);'
      + 'background:var(--input-bg);flex-shrink:0;';
    const url = imageUrl(item.image_id || item.image);
    if (url) thumb.style.background = "url('" + url + "') center / cover no-repeat";

    const name = document.createElement('div');
    name.style.cssText = 'font-size:13px;color:var(--dark);line-height:1.4;';
    name.textContent = item.title;

    row.appendChild(thumb);
    row.appendChild(name);
    row.addEventListener('click', () => { close(); onPick(item); });
    row.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); row.click(); }
    });
    list.appendChild(row);
  });

  const { close } = openModal({
    title: 'Which item are you reviewing?',
    body: list,
    width: '460px',
  });
}
