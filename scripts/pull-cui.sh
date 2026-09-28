#!/bin/bash
# 获取 CUI 源码（cui-desktop 的构建时依赖，非 submodule）
#
# 环境变量：
#   CUI_REPO  仓库地址（默认 https://github.com/YaoApp/cui.git）
#   CUI_REF   分支 / 标签 / commit（默认 main）
#             - nightly：用 main（求新）
#             - stable ：建议 pin 到某个已验证的 commit/tag，保证可复现
#
# 产物：把实际使用的 commit SHA 写入 $PROJECT_DIR/.cui-sha（供 build-cui.sh 记录）
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CUI_DIR="$PROJECT_DIR/cui"
SHA_FILE="$PROJECT_DIR/.cui-sha"

CUI_REPO="${CUI_REPO:-https://github.com/YaoApp/cui.git}"
CUI_REF="${CUI_REF:-main}"

# 清理残留（symlink / 非 git 目录）
if { [ -e "$CUI_DIR" ] || [ -L "$CUI_DIR" ]; } && [ ! -d "$CUI_DIR/.git" ]; then
  echo "Removing stale cui entry..."
  rm -rf "$CUI_DIR"
fi

if [ ! -d "$CUI_DIR/.git" ]; then
  echo "Cloning CUI ($CUI_REPO)..."
  git clone --filter=blob:none --no-checkout "$CUI_REPO" "$CUI_DIR"
fi

echo "Fetching CUI ref: $CUI_REF"
cd "$CUI_DIR"
if git fetch --depth 1 origin "$CUI_REF" 2>/dev/null; then
  git checkout -q --force FETCH_HEAD
else
  echo "  shallow fetch by ref failed; falling back to full fetch..."
  git fetch --tags origin
  git fetch origin
  if ! git checkout -q --force "$CUI_REF" 2>/dev/null; then
    echo "ERROR: cannot resolve CUI_REF=$CUI_REF" >&2
    exit 1
  fi
fi

CUI_SHA="$(git rev-parse HEAD)"
echo "$CUI_SHA" > "$SHA_FILE"
echo "CUI source ready. ref=$CUI_REF sha=$CUI_SHA"
