/* ═══════════════════════════════════════════════════════════════
   Bambe Web — auth.js

   The three auth pages: login, register, forgot-password. One module
   because they share the same three jobs — validate a form, post it,
   and put whatever tokens come back into the session.

   Loaded as `<script type="module">`, so it imports Fetcher directly
   rather than waiting on the window global.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';

/* ── Shared helpers ───────────────────────────────────────── */

const $ = (id) => document.getElementById(id);

/** Guards a button against a double-click while its request is in flight. */
async function withBusy(button, label, work) {
  if (!button || button.classList.contains('is-busy')) return;
  const original = button.innerHTML;
  button.classList.add('is-busy');
  button.disabled = true;
  if (label) button.textContent = label;
  try {
    return await work();
  } finally {
    button.classList.remove('is-busy');
    button.disabled = false;
    button.innerHTML = original;
  }
}

/*
  Where to go after signing in. `?next=` is set by session.requireSignedIn
  when a guarded page bounced the caller here, so they land back where they
  were headed. Same-origin paths only — an absolute URL in a query string is
  an open redirect waiting to happen.
*/
function nextPath(fallback) {
  const next = new URLSearchParams(location.search).get('next');
  if (next && next.startsWith('/') && !next.startsWith('//')) return next;
  return fallback || '/';
}

/*
  Signs the session in from an auth payload and hands the guest cart over
  before leaving. The merge has to happen while the guest id still exists
  and the new token is already stored — that ordering is the whole point.
*/
async function completeSignIn(payload, destination) {
  session.save(payload);

  if (session.peekGuestId()) {
    // A failed merge must not block sign-in — worst case the guest cart is
    // still there next time, which is better than refusing to let them in.
    await Fetcher.post({
      endpoint: '/v1/cart/merge',
      // The id goes in the body, not only the header: the handler takes
      // `web::Json<ReqBody>`, which rejects a request with no body at all
      // — so the header-only version 400'd every time, silently, and the
      // guest cart was never folded in.
      body: { session_id: session.peekGuestId() },
      headers: { 'X-Guest-Session': session.peekGuestId() },
      showError: false,
    });
    session.dropGuestId();
  }

  location.href = destination;
}

/** Turns the eye icon into a real show/hide toggle. */
function wirePasswordToggle(toggleId, inputId) {
  const toggle = $(toggleId);
  const input = $(inputId);
  if (!toggle || !input) return;
  toggle.onclick = () => {
    input.type = input.type === 'password' ? 'text' : 'password';
  };
}

/*
  Strength meter. Deliberately crude — it scores length and variety, and
  says so plainly. The server is what actually enforces a password policy;
  this is a hint, not a gate.
*/
function wireStrength(inputId, barIds, labelId) {
  const input = $(inputId);
  const label = $(labelId);
  const bars = barIds.map($);
  if (!input || !label || bars.some((b) => !b)) return;

  input.addEventListener('input', () => {
    const value = input.value;
    let score = 0;
    if (value.length >= 8) score++;
    if (value.length >= 12) score++;
    if (/[0-9]/.test(value) && /[a-zA-Z]/.test(value)) score++;
    if (/[^a-zA-Z0-9]/.test(value)) score++;
    score = Math.min(score, 3);

    const fill = ['', 'filled-weak', 'filled-mid', 'filled-strong'][score];
    bars.forEach((bar, i) => {
      bar.className = 'pw-bar' + (i < score && fill ? ' ' + fill : '');
    });

    if (!value)            { label.textContent = 'Use 8+ characters'; label.style.color = ''; }
    else if (score <= 1)   { label.textContent = 'Weak — make it longer'; label.style.color = ''; }
    else if (score === 2)  { label.textContent = 'Medium — add numbers to strengthen'; label.style.color = ''; }
    else                   { label.textContent = 'Strong'; label.style.color = 'var(--success)'; }
  });
}

/*
  Six single-character inputs behaving as one field: typing advances,
  backspace on an empty box steps back, and a pasted code fills the row.
  `onComplete` fires whenever all six are filled so the submit button can
  enable itself.
*/
function wireOtp(rowId, onComplete) {
  const row = $(rowId);
  if (!row) return { value: () => '', reset: () => {}, focus: () => {} };
  const boxes = Array.from(row.querySelectorAll('.otp-box'));

  const value = () => boxes.map((b) => b.value).join('');
  const report = () => onComplete && onComplete(value());

  boxes.forEach((box, i) => {
    box.addEventListener('input', () => {
      box.value = box.value.replace(/\D/g, '').slice(0, 1);
      box.classList.toggle('filled', !!box.value);
      if (box.value && i < boxes.length - 1) boxes[i + 1].focus();
      report();
    });

    box.addEventListener('keydown', (e) => {
      if (e.key === 'Backspace' && !box.value && i > 0) {
        boxes[i - 1].focus();
        boxes[i - 1].value = '';
        boxes[i - 1].classList.remove('filled');
        report();
        e.preventDefault();
      }
      if (e.key === 'ArrowLeft' && i > 0) boxes[i - 1].focus();
      if (e.key === 'ArrowRight' && i < boxes.length - 1) boxes[i + 1].focus();
    });

    box.addEventListener('paste', (e) => {
      const digits = (e.clipboardData || window.clipboardData).getData('text').replace(/\D/g, '');
      if (!digits) return;
      e.preventDefault();
      boxes.forEach((b, j) => {
        b.value = digits[j] || '';
        b.classList.toggle('filled', !!b.value);
      });
      boxes[Math.min(digits.length, boxes.length - 1)].focus();
      report();
    });
  });

  return {
    value,
    focus: () => boxes[0] && boxes[0].focus(),
    reset: () => {
      boxes.forEach((b) => { b.value = ''; b.classList.remove('filled'); });
      report();
    },
  };
}

/* ── Login ────────────────────────────────────────────────── */

function initLogin() {
  const submit = $('login-submit');
  if (!submit) return false;

  const email = $('login-email');
  const password = $('login-password');
  wirePasswordToggle('login-password-toggle', 'login-password');

  // The admin console signs in against a different collection, so the same
  // form posts to a different route depending on where it was reached from.
  const isAdmin = location.pathname.startsWith('/admin');
  const endpoint = isAdmin ? '/v1/auth/admin/sign-in' : '/v1/auth/sign-in';

  const run = () => withBusy(submit, 'Signing in…', async () => {
    if (!email.value.trim())    { toast.error('Enter your email'); email.focus(); return; }
    if (!password.value)        { toast.error('Enter your password'); password.focus(); return; }

    const res = await Fetcher.post({
      endpoint,
      body: { email_or_username: email.value.trim(), password: password.value },
    });
    if (!res.ok) return;

    // Both sign-in routes wrap the tokens in `auth_payload` alongside a
    // two_afa_enabled flag; 2FA is not implemented server-side yet, so the
    // payload is always present.
    const payload = res.data && res.data.auth_payload;
    if (!payload) { toast.error('Sign-in returned no session'); return; }

    await completeSignIn(payload, nextPath(isAdmin ? '/admin' : '/'));
  });

  submit.onclick = run;
  [email, password].forEach((field) => {
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter') run(); });
  });

  return true;
}

/* ── Register ─────────────────────────────────────────────── */

function initRegister() {
  const submit = $('reg-submit');
  if (!submit) return false;

  const firstName = $('reg-first-name');
  const lastName  = $('reg-last-name');
  const email     = $('reg-email');
  const phone     = $('reg-phone');
  const password  = $('reg-password');
  const confirm   = $('reg-confirm-password');
  const terms     = $('reg-terms');

  wirePasswordToggle('reg-password-toggle', 'reg-password');
  wireStrength('reg-password', ['reg-pw-bar-1', 'reg-pw-bar-2', 'reg-pw-bar-3'], 'reg-pw-label');

  let termsAccepted = true;
  terms.onclick = () => {
    termsAccepted = !termsAccepted;
    terms.querySelector('.form-check-input').classList.toggle('checked', termsAccepted);
  };

  // Held between the two steps: sign-up returns a user_id, and
  // validate-email needs it back alongside the code.
  let pendingUserId = null;

  const verifySubmit = $('reg-verify-submit');
  const otp = wireOtp('reg-otp', (code) => {
    verifySubmit.disabled = code.length !== 6;
  });

  submit.onclick = () => withBusy(submit, 'Creating account…', async () => {
    if (!firstName.value.trim()) { toast.error('First name is required'); firstName.focus(); return; }
    if (!email.value.trim())     { toast.error('Email is required'); email.focus(); return; }
    if (password.value.length < 8) { toast.error('Password must be at least 8 characters'); password.focus(); return; }
    if (password.value !== confirm.value) { toast.error("Passwords don't match"); confirm.focus(); return; }
    if (!termsAccepted)          { toast.error('Please accept the terms to continue'); return; }

    const res = await Fetcher.post({
      endpoint: '/v1/auth/sign-up',
      body: {
        first_name: firstName.value.trim(),
        last_name: lastName.value.trim(),
        email_address: email.value.trim(),
        password: password.value,
        confirm_password: confirm.value,
      },
    });
    if (!res.ok) return;

    pendingUserId = res.data.user_id;
    $('reg-verify-email').textContent = email.value.trim();
    $('reg-form').style.display = 'none';
    $('reg-verify').style.display = '';
    $('reg-step-1').classList.remove('active');
    $('reg-step-2').classList.remove('active');
    $('reg-step-3').classList.add('active');
    otp.focus();
    toast.success('Account created — check your email for the code');
  });

  verifySubmit.onclick = () => withBusy(verifySubmit, 'Verifying…', async () => {
    const res = await Fetcher.post({
      endpoint: '/v1/auth/validate-email',
      body: { user_id: pendingUserId, verification_code: otp.value() },
    });
    if (!res.ok) { otp.reset(); otp.focus(); return; }

    /*
      validate-email is what returns the tokens — sign-up alone leaves the
      account unverified and unauthenticated. Stored here rather than left
      to completeSignIn below, because the profile call that follows needs
      the token to already be in place to authenticate itself.
    */
    session.save(res.data);

    /*
      The phone number is collected on this form but sign-up takes no such
      field. It goes to the profile instead, which is only reachable once
      the tokens above exist — hence here rather than a step earlier.
    */
    if (phone.value.trim()) {
      await Fetcher.patch({
        endpoint: '/v1/account/profile',
        body: { phone_number: phone.value.trim() },
        showError: false,
      });
    }

    await completeSignIn(res.data, nextPath('/'));
  });

  $('reg-resend').onclick = async () => {
    const res = await Fetcher.post({
      endpoint: '/v1/auth/resend-verification-code',
      body: { user_id: pendingUserId },
    });
    if (res.ok) toast.success('New code sent');
  };

  return true;
}

/* ── Forgot password ──────────────────────────────────────── */

function initForgotPassword() {
  const send = $('fp-send');
  if (!send) return false;

  const email    = $('fp-email');
  const verify   = $('fp-verify');
  const reset    = $('fp-reset');
  const password = $('fp-password');
  const confirm  = $('fp-confirm-password');

  wirePasswordToggle('fp-password-toggle', 'fp-password');
  wireStrength('fp-password', ['fp-pw-bar-1', 'fp-pw-bar-2', 'fp-pw-bar-3'], 'fp-pw-label');

  const otp = wireOtp('fp-otp', (code) => { verify.disabled = code.length !== 6; });

  confirm.addEventListener('input', () => {
    const matched = !!confirm.value && confirm.value === password.value;
    $('fp-match').style.display = matched ? '' : 'none';
  });

  const step = (n) => {
    $('fp-step-1').style.display = n === 1 ? '' : 'none';
    $('fp-step-2').style.display = n === 2 ? '' : 'none';
    $('fp-step-3').style.display = n === 3 ? '' : 'none';
  };

  /*
    The reset is a three-legged handshake, not three independent posts:
    forgot-password answers with a user_id, verify-reset-code trades the
    emailed digits for a secret_key, and only that key lets reset-password
    through. Both are carried between the steps here.
  */
  let resetUserId = null;
  let resetSecretKey = null;

  send.onclick = () => withBusy(send, 'Sending…', async () => {
    if (!email.value.trim()) { toast.error('Enter your email'); email.focus(); return; }

    const res = await Fetcher.post({
      endpoint: '/v1/auth/forgot-password',
      // Takes a username here too, which is why the field is not email-only
      // on the wire even though the label asks for an email.
      body: { email_or_username: email.value.trim() },
    });
    if (!res.ok) return;

    resetUserId = res.data.user_id;
    $('fp-sent-to').textContent = email.value.trim();
    step(2);
    otp.focus();
  });

  verify.onclick = () => withBusy(verify, 'Verifying…', async () => {
    const res = await Fetcher.post({
      endpoint: '/v1/auth/verify-reset-code',
      body: { user_id: resetUserId, validation_code: otp.value() },
    });
    if (!res.ok) { otp.reset(); otp.focus(); return; }

    resetSecretKey = res.data.secret_key;
    step(3);
    password.focus();
  });

  reset.onclick = () => withBusy(reset, 'Saving…', async () => {
    if (password.value.length < 8) { toast.error('Password must be at least 8 characters'); password.focus(); return; }
    if (password.value !== confirm.value) { toast.error("Passwords don't match"); confirm.focus(); return; }

    const res = await Fetcher.post({
      endpoint: '/v1/auth/reset-password',
      body: {
        user_id: resetUserId,
        secret_key: resetSecretKey,
        new_password: password.value,
        confirm_password: confirm.value,
      },
    });
    if (!res.ok) return;

    toast.success('Password changed — sign in with it now');
    setTimeout(() => { location.href = '/login'; }, 900);
  });

  step(1);
  return true;
}

/* ── Social buttons ───────────────────────────────────────── */

/*
  /v1/auth/social-login expects a token minted by Google or Facebook's own
  SDK, and neither SDK is loaded. Saying so beats a button that silently
  does nothing.
*/
function initSocialButtons() {
  document.querySelectorAll('.social-btn').forEach((button) => {
    button.onclick = () => toast.info('Social sign-in is not connected yet');
  });
}

/* ── Boot ─────────────────────────────────────────────────── */

initLogin() || initRegister() || initForgotPassword();
initSocialButtons();
