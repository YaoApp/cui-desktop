#!/bin/bash
set -e

echo "=== Yao Agents Setup ==="
echo ""

# 1. Check dependencies
echo "[1/5] Checking dependencies..."
command -v node >/dev/null 2>&1 || { echo "Error: Node.js >= 18 is required"; exit 1; }
command -v pnpm >/dev/null 2>&1 || { echo "Error: pnpm is required (npm install -g pnpm)"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "Error: Rust toolchain is required (https://rustup.rs)"; exit 1; }

NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
  echo "Error: Node.js >= 18 required, current: $(node -v)"
  exit 1
fi

echo "  Node.js: $(node -v)"
echo "  pnpm: $(pnpm -v)"
echo "  Rust: $(rustc --version)"
echo "  Cargo: $(cargo --version)"
echo ""

# 2. Install Tauri CLI
echo "[2/5] Installing Tauri CLI..."
if ! cargo install --list | grep -q "tauri-cli"; then
  cargo install tauri-cli --version "^2"
else
  echo "  Tauri CLI already installed"
fi
echo ""

# 3. Install frontend dependencies
echo "[3/5] Installing frontend dependencies..."
npm install
echo ""

# 4. Pull CUI source
echo "[4/5] Pulling CUI source..."
bash scripts/pull-cui.sh
echo ""

# 5. Build CUI
echo "[5/5] Building CUI..."
bash scripts/build-cui.sh
echo ""

echo "=== Setup complete! ==="
echo ""
echo "Run the following to start development mode:"
echo "  cargo tauri dev"
echo ""
