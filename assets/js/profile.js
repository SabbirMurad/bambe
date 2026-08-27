/* ═══════════════════════════════════════════════════════════════
   Bambe Web — profile.js

   The account's own page: who you are, the four tallies, your
   password, your notification switches, and the two books the
   checkout draws from — addresses and payment methods.

   Three of the wireframe's fields are gone rather than wired:
   date of birth, gender and language. Nothing on the backend
   stores them, so a Save that appeared to accept them and then
   dropped them on the floor is worse than not offering them. The
   email field stays, read-only, because changing an email means
   re-verifying it and there is no endpoint for that yet.

   Two-factor is shown, not switched: `two_factor_enabled` is on
   the account record but nothing exposes a way to change it.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { mountAccountChrome, currentProfile } from './account.js';

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.prototype.slice.call(document.querySelectorAll(sel));

let addresses = [];
let methods = [];
let editingAddress = null;

/* ── Header and stats ─────────────────────────────────────────  */

function fullName(profile) {
  return [profile.first_name, profile.last_name].filter(Boolean).join(' ').trim()
    || profile.username || 'Your account';
}

const SEGMENT_LABEL = {
  Vip: 'VIP member', Repeat: 'Returning customer',
  New: 'New customer', Dormant: 'Member',
};

function renderHeader(profile, stats) {
  $('.profile-name').textContent = fullName(profile);
  $('.profile-email').textContent = profile.email || '';

  const since = profile.member_since
    ? 'Member since ' + new Date(profile.member_since).toLocaleDateString(undefined, { year: 'numeric', month: 'long' })
    : '';
  const segment = stats && stats.segment ? SEGMENT_LABEL[stats.segment] || stats.segment : '';
  $('.profile-member-since').textContent = [since, segment].filter(Boolean).join(' · ');

  /* Uploading an avatar needs POST /image with a ProfilePic slot and
     then a PATCH — worth doing, but it is a feature rather than a
     wiring job, so the pencil is hidden instead of lying. */
  const avatarEdit = $('.profile-avatar-edit');
  if (avatarEdit) avatarEdit.style.display = 'none';

  const editBtn = $('.profile-edit-btn');
  if (editBtn) {
    editBtn.onclick = () => {
      $('.profile-form-card').scrollIntoView({ behavior: 'smooth', block: 'start' });
      const first = $('#p-first');
      if (first) first.focus();
    };
  }
}

function renderStats(stats) {
  const cards = $$('.stat-card');
  const values = [
    stats.order_count,
    stats.wishlist_count,
    stats.review_count,
    stats.loyalty_points,
  ];
  cards.forEach((card, index) => {
    const number = card.querySelector('.stat-num');
    if (number) number.textContent = (values[index] || 0).toLocaleString();
  });
}

/* ── Personal information ─────────────────────────────────────  */

function renderPersonal(profile) {
  $('#p-first').value = profile.first_name || '';
  $('#p-last').value = profile.last_name || '';
  $('#p-email').value = profile.email || '';
  $('#p-phone').value = profile.phone_number || '';

  const marketing = $('#p-marketing');
  if (marketing) marketing.classList.toggle('on', !!profile.accepts_marketing);

  const save = $('.profile-form-card .btn-outline');
  save.onclick = async () => {
    save.disabled = true;
    const response = await Fetcher.patch({
      endpoint: '/v1/account/profile',
      body: {
        first_name: $('#p-first').value.trim(),
        last_name: $('#p-last').value.trim(),
        phone_number: $('#p-phone').value.trim() || null,
        accepts_marketing: marketing ? marketing.classList.contains('on') : undefined,
      },
    });
    save.disabled = false;
    if (!response.ok) return;

    toast.success('Profile saved');
    // The sidebar and the header both show the name that just changed.
    const name = fullName({ ...currentProfile(), first_name: $('#p-first').value.trim(), last_name: $('#p-last').value.trim() });
    $('.profile-name').textContent = name;
    const sidebarName = $('.account-user-name');
    if (sidebarName) sidebarName.textContent = name;
  };
}

/* ── Security ─────────────────────────────────────────────────  */

function renderSecurity(profile) {
  const state = $('#twofa-state');
  const toggle = $('#twofa');
  if (state && toggle) {
    const on = !!profile.two_factor_enabled;
    toggle.classList.toggle('on', on);
    state.textContent = on ? 'Enabled' : 'Not enabled';
    state.style.color = on ? 'var(--success)' : 'var(--subtle)';
    /* Nothing exposes a way to turn this on or off, so the switch is
       shown as a state rather than a control that silently does
       nothing. toggle.js is told to leave it alone. */
    toggle.dataset.readonly = 'true';
    toggle.style.cursor = 'default';
    toggle.style.opacity = '0.6';
    toggle.title = 'Managed from account security settings';
  }

  const button = $$('.profile-form-card')[1].querySelector('.btn-outline');
  button.onclick = async () => {
    const current = $('#p-current-password');
    const next = $('#p-new-password');
    const confirm = $('#p-confirm-password');

    if (!current.value || !next.value) {
      toast.info('Enter your current and new password');
      return;
    }
    if (next.value !== confirm.value) {
      toast.error("The new passwords don't match");
      return;
    }

    button.disabled = true;
    const response = await Fetcher.patch({
      endpoint: '/v1/auth/change-password',
      body: {
        current_password: current.value,
        new_password: next.value,
        confirm_password: confirm.value,
      },
    });
    button.disabled = false;
    if (!response.ok) return;

    [current, next, confirm].forEach((input) => { input.value = ''; });
    toast.success('Password changed');
  };
}

/* ── Notification preferences ─────────────────────────────────
   Three switches over nine flags, so each one owns the group it
   plainly describes and writes all of them together.            */

const SWITCH_GROUPS = [
  ['order_updates', 'delivery_updates'],
  ['promotions', 'weekly_digest'],
  ['price_drops', 'back_in_stock'],
];

async function renderPreferences() {
  const response = await Fetcher.get({ endpoint: '/v1/notification/preferences', showError: false });
  if (!response.ok) return;
  const prefs = response.data;

  const toggles = $$('.profile-form-card')[2].querySelectorAll('.toggle');
  SWITCH_GROUPS.forEach((group, index) => {
    const toggle = toggles[index];
    if (!toggle) return;
    toggle.classList.toggle('on', group.every((flag) => prefs[flag]));

    toggle.addEventListener('click', () => {
      /* toggle.js flips the class from a delegated listener on the
         document, and a listener bound to the element itself runs
         first — so reading the class here reads the state it is
         leaving, not the one it is going to. Deferring a tick puts
         this after every listener, whatever order they were added in. */
      setTimeout(async () => {
        const on = toggle.classList.contains('on');
        const patch = {};
        group.forEach((flag) => { patch[flag] = on; });

        const saved = await Fetcher.patch({
          endpoint: '/v1/notification/preferences',
          body: patch,
          showError: false,
        });
        if (!saved.ok) {
          toggle.classList.toggle('on', !on);
          toast.error("That preference couldn't be saved");
        }
      }, 0);
    });
  });
}

/* ── Addresses ────────────────────────────────────────────────  */

function addressText(address) {
  const region = [address.state, address.zip].filter(Boolean).join(' ');
  return [address.street, [address.city, region].filter(Boolean).join(', '), address.country]
    .filter(Boolean).join(', ');
}

function addressRow(address) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:14px;align-items:flex-start;padding:14px 0;border-bottom:1px solid var(--border);';

  const body = document.createElement('div');
  body.style.flex = '1';

  const heading = document.createElement('div');
  heading.style.cssText = 'display:flex;align-items:center;gap:8px;margin-bottom:3px;';
  const label = document.createElement('span');
  label.style.cssText = 'font-size:13.5px;font-weight:700;color:var(--dark);';
  label.textContent = address.label || 'Address';
  heading.appendChild(label);
  if (address.is_default) {
    const badge = document.createElement('span');
    badge.className = 'address-card-default';
    badge.textContent = 'Default';
    heading.appendChild(badge);
  }

  const who = document.createElement('div');
  who.style.cssText = 'font-size:13px;color:var(--dark);';
  who.textContent = address.recipient_name;

  const where = document.createElement('div');
  where.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:2px;line-height:1.6;';
  where.textContent = addressText(address) + (address.phone ? ' · ' + address.phone : '');

  body.append(heading, who, where);

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:6px;flex-shrink:0;';

  const edit = document.createElement('button');
  edit.className = 'btn btn-ghost btn-sm';
  edit.textContent = 'Edit';
  edit.onclick = () => openAddressForm(address);
  actions.appendChild(edit);

  if (!address.is_default) {
    const makeDefault = document.createElement('button');
    makeDefault.className = 'btn btn-ghost btn-sm';
    makeDefault.textContent = 'Make default';
    makeDefault.onclick = async () => {
      const response = await Fetcher.post({ endpoint: '/v1/address/' + address.uuid + '/default' });
      if (response.ok) { toast.success('Default address updated'); loadAddresses(); }
    };
    actions.appendChild(makeDefault);

    /* The default one has no Remove: something has to be the default,
       and `address/remove` would leave the book without one. */
    const remove = document.createElement('button');
    remove.className = 'btn btn-ghost btn-sm';
    remove.style.color = 'var(--error)';
    remove.textContent = 'Remove';
    remove.onclick = async () => {
      if (!window.confirm('Remove this address?')) return;
      const response = await Fetcher.delete({ endpoint: '/v1/address/' + address.uuid });
      if (response.ok) { toast.success('Address removed'); loadAddresses(); }
    };
    actions.appendChild(remove);
  }

  row.append(body, actions);
  return row;
}

const ADDRESS_FIELDS = [
  ['label', 'Label', 'Home, Office…'],
  ['recipient_name', 'Full name', 'Jane Doe'],
  ['street', 'Street address', '42 Maple Street, Apt 3B'],
  ['city', 'City', 'Brooklyn'],
  ['state', 'State / Region', 'New York'],
  ['zip', 'ZIP / Postcode', '11201'],
  ['country', 'Country', 'United States'],
  ['phone', 'Phone', '+1 (917) 555-0142'],
];

function openAddressForm(address) {
  editingAddress = address || null;
  const form = $('#addr-editor');
  form.hidden = false;
  form.querySelector('.addr-editor-title').textContent = address ? 'Edit address' : 'Add a new address';

  ADDRESS_FIELDS.forEach(([key]) => {
    form.querySelector('#pa-' + key).value = address ? (address[key] || '') : '';
  });
  form.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
}

function closeAddressForm() {
  $('#addr-editor').hidden = true;
  editingAddress = null;
}

async function saveAddress() {
  const form = $('#addr-editor');
  const values = {};
  ADDRESS_FIELDS.forEach(([key]) => { values[key] = form.querySelector('#pa-' + key).value.trim(); });

  const missing = ['recipient_name', 'street', 'city', 'country', 'phone'].find((key) => !values[key]);
  if (missing) {
    toast.info('Fill in the ' + missing.replace('_', ' ') + ' first');
    form.querySelector('#pa-' + missing).focus();
    return;
  }

  const body = {
    label: values.label || 'Address',
    recipient_name: values.recipient_name,
    street: values.street,
    city: values.city,
    state: values.state || null,
    zip: values.zip || null,
    country: values.country,
    phone: values.phone,
  };

  const response = editingAddress
    ? await Fetcher.patch({ endpoint: '/v1/address/' + editingAddress.uuid, body })
    : await Fetcher.post({ endpoint: '/v1/address', body });
  if (!response.ok) return;

  toast.success(editingAddress ? 'Address updated' : 'Address saved');
  closeAddressForm();
  loadAddresses();
}

async function loadAddresses() {
  const response = await Fetcher.get({ endpoint: '/v1/address/list' });
  addresses = response.ok ? (response.data || []) : [];

  const host = $('#address-list');
  host.innerHTML = '';
  if (!addresses.length) {
    host.innerHTML = '<div style="font-size:13px;color:var(--subtle);padding:14px 0;">No saved addresses yet.</div>';
    return;
  }
  addresses.forEach((address) => host.appendChild(addressRow(address)));
}

/* ── Payment methods ──────────────────────────────────────────  */

function methodRow(method) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:14px;align-items:center;padding:14px 0;border-bottom:1px solid var(--border);';

  const body = document.createElement('div');
  body.style.flex = '1';
  const title = document.createElement('div');
  title.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:13.5px;font-weight:700;color:var(--dark);';
  title.textContent = method.card_brand && method.last4
    ? method.card_brand + ' ending ' + method.last4
    : (method.account_email || method.kind);
  if (method.is_default) {
    const badge = document.createElement('span');
    badge.className = 'address-card-default';
    badge.textContent = 'Default';
    title.appendChild(badge);
  }

  const detail = document.createElement('div');
  detail.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:2px;';
  detail.textContent = method.exp_month && method.exp_year
    ? 'Expires ' + String(method.exp_month).padStart(2, '0') + '/' + String(method.exp_year).slice(-2)
    : method.kind;

  body.append(title, detail);

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:6px;';

  if (!method.is_default) {
    const makeDefault = document.createElement('button');
    makeDefault.className = 'btn btn-ghost btn-sm';
    makeDefault.textContent = 'Make default';
    makeDefault.onclick = async () => {
      const response = await Fetcher.post({ endpoint: '/v1/payment-method/' + method.uuid + '/default' });
      if (response.ok) { toast.success('Default payment method updated'); loadMethods(); }
    };
    actions.appendChild(makeDefault);
  }

  const remove = document.createElement('button');
  remove.className = 'btn btn-ghost btn-sm';
  remove.style.color = 'var(--error)';
  remove.textContent = 'Remove';
  remove.onclick = async () => {
    if (!window.confirm('Remove this payment method?')) return;
    const response = await Fetcher.delete({ endpoint: '/v1/payment-method/' + method.uuid });
    if (response.ok) { toast.success('Payment method removed'); loadMethods(); }
  };
  actions.appendChild(remove);

  row.append(body, actions);
  return row;
}

async function loadMethods() {
  const response = await Fetcher.get({ endpoint: '/v1/payment-method/list' });
  methods = response.ok ? (response.data || []) : [];

  const host = $('#payment-list');
  host.innerHTML = '';
  if (!methods.length) {
    host.innerHTML = '<div style="font-size:13px;color:var(--subtle);padding:14px 0;line-height:1.7;">'
      + 'No saved payment methods. New cards are tokenised by a payment processor, and none is connected to this store yet — '
      + 'so cards can’t be added here, and no card number is ever stored by Bambe.</div>';
    return;
  }
  methods.forEach((method) => host.appendChild(methodRow(method)));
}

/* ── Delete account ───────────────────────────────────────────  */

function wireDeleteAccount() {
  const button = $('#delete-account');
  if (!button) return;
  button.onclick = async () => {
    /* This raises a request an administrator actions — it is not an
       immediate delete, and the wording says so rather than promising
       something the endpoint doesn't do. */
    if (!window.confirm('Request account deletion? An administrator will review it, and your account stays active until they do.')) return;

    const response = await Fetcher.post({
      endpoint: '/v1/account/delete',
      body: { reason: 'Requested from profile page' },
    });
    if (response.ok) toast.success('Deletion request submitted — we’ll be in touch');
  };
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  const stats = await Fetcher.get({ endpoint: '/v1/account/stats', showError: false });

  renderHeader(profile, stats.ok ? stats.data : null);
  if (stats.ok) renderStats(stats.data);
  renderPersonal(profile);
  renderSecurity(profile);
  renderPreferences();

  $('#addr-add-btn').onclick = () => openAddressForm(null);
  $('#addr-editor-save').onclick = saveAddress;
  $('#addr-editor-cancel').onclick = closeAddressForm;
  loadAddresses();
  loadMethods();
  wireDeleteAccount();

  // Arriving from the sidebar's Addresses / Payment Methods items.
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
})();
