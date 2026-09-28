#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CUI_DIR="$PROJECT_DIR/cui"
DIST_DIR="$PROJECT_DIR/cui-dist"

if [ ! -d "$CUI_DIR" ]; then
  echo "Error: CUI source not found. Run: bash scripts/pull-cui.sh"
  exit 1
fi

echo "Building CUI..."
cd "$CUI_DIR"

# Set CUI build env (same as yao Makefile)
echo "BASE=__yao_admin_root" > packages/cui/.env

# Install dependencies (--no-frozen-lockfile for CI compatibility)
echo "  Installing dependencies..."
pnpm install --no-frozen-lockfile

# Build
echo "  Building (this may take a few minutes)..."
pnpm run build:cui

# Copy build output
echo "  Copying build output..."
rm -rf "$DIST_DIR"
cp -r packages/cui/dist "$DIST_DIR"

# Record build metadata (traceability: version + which cui/desktop commit was built)
VERSION="$(CUI_VERSION_FILE="$CUI_DIR/version.json" node "$PROJECT_DIR/scripts/version.mjs" current 2>/dev/null || echo unknown)"
CUI_SHA="$(cat "$PROJECT_DIR/.cui-sha" 2>/dev/null || git -C "$CUI_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
DESKTOP_SHA="$(git -C "$PROJECT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
CUI_REF="${CUI_REF:-main}"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
cat > "$DIST_DIR/build-info.json" <<EOF
{
  "version": "${VERSION}",
  "desktop_sha": "${DESKTOP_SHA}",
  "cui_ref": "${CUI_REF}",
  "cui_sha": "${CUI_SHA}",
  "built_at": "${BUILT_AT}"
}
EOF
echo "  build-info: version=$VERSION desktop=$DESKTOP_SHA ref=$CUI_REF cui=$CUI_SHA"

echo "CUI build complete!"
echo "  Output: $DIST_DIR"
echo "  Size: $(du -sh "$DIST_DIR" | cut -f1)"
