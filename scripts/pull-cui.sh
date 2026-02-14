#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CUI_DIR="$PROJECT_DIR/cui"

if [ -d "$CUI_DIR/.git" ]; then
  echo "Updating CUI..."
  cd "$CUI_DIR"
  git pull origin main
else
  echo "Cloning CUI..."
  git clone --depth 1 https://github.com/YaoApp/cui.git "$CUI_DIR"
fi

echo "CUI source ready."
