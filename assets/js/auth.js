/* ═══════════════════════════════════════════════════════════════
   Bambee Web — auth.js

   The three auth pages: login, register, forgot-password. One module
   because they share the same three jobs — validate a form, post it,
   and put whatever tokens come back into the session.

   Loaded as `<script type="module">`, so it imports Fetcher directly
   rather than waiting on the window global.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { DIAL_CODES, DEFAULT_DIAL_ISO } from './dial-codes.js';

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

/* ── Two-step sign-in ─────────────────────────────────────────
   Swaps the password form for a code field rather than opening a
   dialog: this is the same act of signing in, not a detour, and the
   browser's password manager should not be asked to fill anything
   here.                                                            */
function askForCode(requestId, destination) {
  /* The login page has no single form element to swap — it's a flat run
     of `.form-group` blocks, a social block and a divider, all siblings
     of the submit button, with the "create an account" link after it.
     So: hide everything before the button, and the button, and put the
     code step in its place. The link below stays. */
  const anchor = $('login-submit');
  if (!anchor || !anchor.parentElement) {
    toast.error('Enter the code from your email to finish signing in');
    return;
  }
  const parent = anchor.parentElement;
  const hidden = [];
  for (let node = anchor.previousElementSibling; node; node = node.previousElementSibling) {
    if (node.style.display !== 'none') { hidden.push(node); node.style.display = 'none'; }
  }
  anchor.style.display = 'none';

  const step = document.createElement('div');
  step.className = 'auth-2fa-step';

  const heading = document.createElement('h2');
  heading.style.cssText = 'margin:0 0 6px;font-size:19px;font-weight:800;color:var(--black);';
  heading.textContent = 'Check your email';

  const blurb = document.createElement('p');
  blurb.style.cssText = 'margin:0 0 18px;font-size:13.5px;line-height:1.6;color:var(--mid);';
  blurb.textContent = 'We sent you a 6-digit code. It expires in 15 minutes.';

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'form-input';
  input.inputMode = 'numeric';
  input.autocomplete = 'one-time-code';
  input.maxLength = 6;
  input.placeholder = '000000';
  input.setAttribute('aria-label', 'Six-digit sign-in code');
  input.style.cssText = 'letter-spacing:6px;font-size:20px;text-align:center;';

  const button = document.createElement('button');
  button.className = 'btn btn-primary';
  button.style.cssText = 'width:100%;margin-top:14px;';
  button.textContent = 'Verify and sign in';

  step.append(heading, blurb, input, button);

  /* The password field is emptied as well as hidden, so a stale value
     can't be resubmitted from behind the code step. */
  const password = $('login-password');
  if (password) password.value = '';

  parent.insertBefore(step, anchor);
  input.focus();

  const submit = () => withBusy(button, 'Verifying…', async () => {
    const code = input.value.trim();
    if (code.length < 6) { toast.error('Enter the 6-digit code'); input.focus(); return; }

    const res = await Fetcher.post({
      endpoint: '/v1/auth/verify-sign-in-code',
      body: { request_id: requestId, validation_code: code },
    });
    if (!res.ok) { input.select(); return; }

    await completeSignIn(res.data, destination);
  });

  button.onclick = submit;
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
}

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
  if (isAdmin) dressAsAdminSignIn();

  const run = () => withBusy(submit, 'Signing in…', async () => {
    if (!email.value.trim())    { toast.error('Enter your email'); email.focus(); return; }
    if (!password.value)        { toast.error('Enter your password'); password.focus(); return; }

    const res = await Fetcher.post({
      endpoint,
      body: { email_or_username: email.value.trim(), password: password.value },
    });
    if (!res.ok) return;

    /* Both sign-in routes wrap the tokens in `auth_payload` alongside a
       `two_afa_enabled` flag. When that flag is set there is no payload:
       the account has two-step sign-in on, a code has just been emailed,
       and the request id is what exchanges it for a session. */
    if (res.data && res.data.two_afa_enabled) {
      askForCode(res.data.request_id, nextPath(isAdmin ? '/admin' : '/'));
      return;
    }

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

/*
  Fills the dial-code <select> and hands it to select.js to skin. The
  options are built here rather than written into the page because
  there are two hundred of them, and being a real <select> is what
  buys keyboard control and type-ahead without writing either.

  select.js is a classic script, so it has already run by the time
  this module executes; its own DOMContentLoaded sweep would catch
  the element anyway, just a frame later and after a flash of the
  unstyled native control.
*/
function initDialCodes(select) {
  if (!select) return null;

  select.innerHTML = '';
  DIAL_CODES.forEach((country) => {
    const option = document.createElement('option');
    option.value = country.dial;
    // The name is what the menu is searched and read by; the button
    // has room for the flag and the code only.
    option.textContent = `${country.name} (${country.dial})`;
    option.dataset.short = `${country.flag} ${country.dial}`;
    option.selected = country.iso === DEFAULT_DIAL_ISO;
    select.appendChild(option);
  });

  window.BambeSelect?.scan();
  return select;
}

/*
  Turns the shared login page into the admin one.

  /login and /admin/login render the same template (markup.rs), which
  left two identical forms searching two different collections — an
  admin who landed on /login was told their email did not exist, with
  nothing on the page to suggest they were in the wrong place.

  Done here rather than in a second template so the form, its
  validation and its markup stay in one file: only the labelling
  differs, and the parts that have no meaning for staff come off.
*/
function dressAsAdminSignIn() {
  const title = document.querySelector('.auth-form-title');
  const sub = document.querySelector('.auth-form-sub');
  if (title) title.textContent = 'Admin sign-in';
  if (sub) sub.textContent = 'Staff access to the Bambee console.';

  // Social sign-in mints a token for a shopper account; it cannot
  // produce an admin session, so it would fail whatever the outcome.
  const social = document.querySelector('.social-login');
  if (social) social.remove();
  const divider = document.querySelector('.divider-text');
  if (divider) divider.remove();

  // There is no admin self-registration — accounts are created against
  // ADMIN_SIGNUP_SECRET — so the invitation to make one leads nowhere.
  const register = document.querySelector('.auth-footer-link');
  if (register) register.remove();

  /* The panel beside the form sells the shop to shoppers. The brand
     lockup sits directly above this, so the tagline must not repeat
     the name — it reads as one phrase with the logo. */
  const tagline = document.querySelector('.auth-panel-tagline');
  if (tagline) tagline.innerHTML = 'Store<br><em>console.</em>';
  const panelSub = document.querySelector('.auth-panel-sub');
  if (panelSub) panelSub.textContent = 'Orders, catalogue, customers and settings — staff only.';
  const features = document.querySelector('.auth-feature-list');
  if (features) features.remove();

  document.title = 'Admin sign-in — Bambee';
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
  const dial      = initDialCodes($('reg-dial'));

  /*
    The profile keeps one free-text phone field, and the rest of the
    app writes it dial code first ("+1 917 555 0142"), so the picker
    has to be folded back into the number before it is sent.
  */
  const fullPhone = () => {
    const number = phone.value.trim();
    if (!number) return '';
    // A number already typed with its own "+" is left as the user wrote it.
    if (number.startsWith('+') || !dial) return number;
    return `${dial.value} ${number}`;
  };

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
    const phoneNumber = fullPhone();
    if (phoneNumber) {
      await Fetcher.patch({
        endpoint: '/v1/account/profile',
        body: { phone_number: phoneNumber },
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
  Google goes through Firebase Auth: its popup returns a Firebase ID token,
  and /v1/auth/social-login verifies that against Google's keys and answers
  with our own session tokens — the same payload a password sign-in gets.

  The SDK is fetched on the first click rather than with the page: it is a
  few hundred KB that most visitors, signing in by email or not at all,
  never need. Pinned to one version so a Firebase release can't change the
  sign-in page under us.
*/
const FIREBASE_SDK = 'https://www.gstatic.com/firebasejs/10.12.2';

let firebaseAuth = null;
async function loadFirebaseAuth() {
  if (firebaseAuth) return firebaseAuth;

  const config = await Fetcher.get({ endpoint: '/v1/auth/firebase-config', showError: false });
  if (!config.ok) return null;

  const [{ initializeApp }, auth] = await Promise.all([
    import(`${FIREBASE_SDK}/firebase-app.js`),
    import(`${FIREBASE_SDK}/firebase-auth.js`),
  ]);
  firebaseAuth = { auth: auth.getAuth(initializeApp(config.data)), sdk: auth };
  return firebaseAuth;
}

/* Popup outcomes that are the customer changing their mind, not a fault. */
const POPUP_DISMISSED = ['auth/popup-closed-by-user', 'auth/cancelled-popup-request'];

async function signInWithGoogle(button) {
  await withBusy(button, 'Opening Google…', async () => {
    let firebase;
    try {
      firebase = await loadFirebaseAuth();
    } catch (error) {
      console.error('[auth] Firebase SDK failed to load', error);
    }
    if (!firebase) {
      toast.error('Google sign-in is unavailable right now');
      return;
    }

    const { auth, sdk } = firebase;
    let idToken;
    try {
      const provider = new sdk.GoogleAuthProvider();
      provider.setCustomParameters({ prompt: 'select_account' });
      const result = await sdk.signInWithPopup(auth, provider);
      idToken = await result.user.getIdToken();
    } catch (error) {
      if (POPUP_DISMISSED.includes(error.code)) return;
      if (error.code === 'auth/popup-blocked') {
        toast.error('Your browser blocked the Google window — allow pop-ups for this site and try again');
        return;
      }
      console.error('[auth] Google sign-in failed', error);
      toast.error('Google sign-in failed — please try again');
      return;
    }

    const res = await Fetcher.post({
      endpoint: '/v1/auth/social-login',
      body: { provider: 'google', token: idToken },
    });
    // Our session is the one that counts; Firebase's own is not needed again.
    sdk.signOut(auth).catch(() => {});
    if (!res.ok) return;

    await completeSignIn(res.data, nextPath('/'));
  });
}

/*
  Facebook would need its own app review before it could sign anyone in,
  so its button says so rather than silently doing nothing.
*/
function initSocialButtons() {
  document.querySelectorAll('.social-btn').forEach((button) => {
    button.onclick = button.dataset.provider === 'google'
      ? () => signInWithGoogle(button)
      : () => toast.info('Facebook sign-in is not connected yet');
  });
}

/* ── Boot ─────────────────────────────────────────────────── */

initLogin() || initRegister() || initForgotPassword();
initSocialButtons();
