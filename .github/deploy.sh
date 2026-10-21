#!/usr/bin/env bash
#
# Runs ON THE SERVER, fed to `bash -s` over SSH by .github/workflows/deploy.yml.
#
# Two modes, decided by whether a new binary is waiting:
#
#   $PROJ/release/$APP.new present  ->  full deploy: swap the binary, restart
#   absent                         ->  pull and restart only
#
# The second is what a pages/ or assets/ change needs. Nothing under those
# directories is compiled into the binary, but Tera globs `pages/**` once at
# startup (main.rs) and once behind a OnceLock (integrations/mail_template.rs),
# so the templates already in memory are only replaced by a restart. Files under
# assets/ are read per request and would not even need that.
#
# Can also be run by hand:
#   APP=my_api PROJ=/srv/my_api bash .github/deploy.sh
set -euo pipefail

: "${APP:?APP must be set}"
: "${PROJ:?PROJ must be set}"
BRANCH="${BRANCH:-main}"
HEALTH_URL="${HEALTH_URL:-http://127.0.0.1/}"

BIN="$PROJ/release/$APP"
NEW="$BIN.new"
OLD="$BIN.old"
STAMP="$BIN.sha"

# Deploying as root is fine; sudo may not even be installed in that case.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

cd "$PROJ"

# STAGED says whether this run is supposed to be shipping a binary: "true",
# "false", or "auto" when run by hand, where the file's presence decides.
#
# Stating it beats inferring it. A transfer that dies halfway leaves a truncated
# $APP.new behind, and the next restart-only deploy would otherwise install it
# as though it were a real build.
STAGED="${STAGED:-auto}"

case "$STAGED" in
  true)
    [ -f "$NEW" ] || { echo "!! expected a staged binary at $NEW"; exit 1; }
    ;;
  false)
    if [ -f "$NEW" ]; then
      echo "==> discarding stale $NEW (this run ships no binary)"
      rm -f "$NEW"
    fi
    ;;
esac

swapped=0
if [ -f "$NEW" ]; then
  echo "==> new binary staged; this is a full deploy"
else
  echo "==> no staged binary; pull and restart only"
fi

# Captured before anything moves, so a failed start can put both the working
# tree and the binary back the way they were.
PREV_COMMIT="$(git rev-parse HEAD)"
PREV_SHA="$(cat "$STAMP" 2>/dev/null || true)"

echo "==> stopping $APP"
# The socket alone is enough. $APP.service declares Requires=$APP.socket, and
# systemd propagates an explicit stop to every unit that requires the one being
# stopped, so the service goes down with it. Keeping the pipeline to this one
# unit is also why the deploy user's sudoers rule needs only two commands.
$SUDO systemctl stop "$APP.socket"

# That propagation only happens if the unit actually declares the dependency.
# Without Requires= the old process keeps running and keeps serving the old
# code, the binary underneath it is replaced with nobody reading it, and the
# health check below reports a perfectly healthy deploy that never happened.
# Silent and expensive, so it is worth the four lines to catch.
for _ in $(seq 1 10); do
  [ "$(systemctl is-active "$APP.service" || true)" = "active" ] || break
  sleep 1
done
if [ "$(systemctl is-active "$APP.service" || true)" = "active" ]; then
  echo "!! $APP.service is still running after stopping $APP.socket"
  echo "   add 'Requires=$APP.socket' to the [Unit] section of $APP.service"
  exit 1
fi

echo "==> updating working tree"
# --ff-only rather than a plain pull: if the server's tree has drifted, fail here
# loudly instead of writing a merge commit nobody will ever look at.
git fetch origin "$BRANCH"
git merge --ff-only "origin/$BRANCH"

if [ -f "$NEW" ]; then
  echo "==> installing binary"
  # The previous binary is kept, not deleted, so there is something to roll
  # back to below.
  if [ -f "$BIN" ]; then mv -f "$BIN" "$OLD"; fi
  mv -f "$NEW" "$BIN"
  chmod 755 "$BIN"
  swapped=1
fi

# Stamped before the start so it is in place either way; rolled back with
# everything else if the service does not come up. This is the value the
# workflow reads next time to decide whether a rebuild is needed at all.
printf '%s\n' "${SHA:-$(git rev-parse HEAD)}" > "$STAMP"

echo "==> starting $APP"
$SUDO systemctl start "$APP.socket"

# With Accept=no socket activation the service does not start until something
# connects, so poll the URL to wake it and only then ask systemd how it went.
echo "==> waiting for $APP.service"
ok=0
state=""
for _ in $(seq 1 15); do
  curl -sk -o /dev/null --max-time 5 "$HEALTH_URL" || true
  state="$(systemctl is-active "$APP.service" || true)"
  case "$state" in
    active) ok=1; break ;;
    failed) break ;;
  esac
  sleep 2
done

if [ "$ok" -ne 1 ]; then
  echo "!! $APP.service did not come up (state: ${state:-unknown})"
  journalctl -u "$APP.service" -n 50 --no-pager || true

  echo "==> rolling back"
  $SUDO systemctl stop "$APP.socket" || true

  # The tree goes back too, not just the binary. A restart-only deploy that
  # fails was almost certainly broken by a template: main.rs does
  # `Tera::new("pages/**/*").unwrap()`, so one malformed file panics every
  # worker at startup, and restoring the binary alone would change nothing.
  git reset --hard "$PREV_COMMIT"

  if [ "$swapped" -eq 1 ] && [ -f "$OLD" ]; then
    mv -f "$BIN" "$BIN.failed"
    mv -f "$OLD" "$BIN"
    chmod 755 "$BIN"
    echo "   the rejected binary is at $BIN.failed"
  fi

  if [ -n "$PREV_SHA" ]; then
    printf '%s\n' "$PREV_SHA" > "$STAMP"
  else
    rm -f "$STAMP"
  fi

  $SUDO systemctl start "$APP.socket"
  echo "rolled back to $PREV_COMMIT"
  exit 1
fi

echo "==> deployed $(git rev-parse --short HEAD)"
