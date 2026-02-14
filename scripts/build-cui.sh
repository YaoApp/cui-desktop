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

# Install dependencies
echo "  Installing dependencies..."
pnpm install

# Build
echo "  Building (this may take a few minutes)..."
pnpm run build:cui

# Copy build output
echo "  Copying build output..."
rm -rf "$DIST_DIR"
cp -r packages/cui/dist "$DIST_DIR"

echo "CUI build complete!"
echo "  Output: $DIST_DIR"
echo "  Size: $(du -sh "$DIST_DIR" | cut -f1)"
