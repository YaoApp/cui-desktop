#!/bin/bash
# DMG codesign with retry + timestamp fallback (macOS).
#
# Why: `codesign --timestamp` contacts Apple's timestamp service, which is
# occasionally unreachable on CI runners ("The timestamp service is not
# available."). A DMG does not strictly require a secure timestamp — the .app
# inside it is timestamped, notarized and stapled — so instead of hard-failing
# the release we retry, then fall back to signing without `--timestamp`.
#
# Usage: codesign-dmg-retry.sh <identity> <dmg-path> [attempts]
set -u

IDENTITY="${1:?identity required}"
TARGET="${2:?target required}"
ATTEMPTS="${3:-3}"

for i in 1 2 3; do
  [ "$i" -le "$ATTEMPTS" ] || break
  echo "codesign (DMG, with timestamp) attempt ${i}/${ATTEMPTS}: ${TARGET}"
  if codesign --force --verbose --timestamp --sign "$IDENTITY" "$TARGET"; then
    echo "codesign with timestamp OK."
    exit 0
  fi
  echo "::warning::codesign --timestamp failed (attempt ${i}/${ATTEMPTS}); retrying in 15s..."
  sleep 15
done

echo "::warning::timestamp service unavailable after ${ATTEMPTS} attempts; signing DMG without timestamp (app is already notarized & timestamped)."
codesign --force --verbose --sign "$IDENTITY" "$TARGET"
