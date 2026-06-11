#!/usr/bin/env bash
# publish-npm.sh
#
# 发布 cc-switch-client 到 npm。
#
# 模式 1（推荐）: 从 GitHub Release 下载预编译二进制
#   ./publish-npm.sh --from-release v0.1.0
#   ./publish-npm.sh --from-release cc-switch-client-v0.1.0
#
# 模式 2（本地构建）: 使用本地已编译的二进制
#   ./publish-npm.sh
#   ./publish-npm.sh --dry-run
#
# 前提:
#   - npm login 已完成（或设置了 NPM_TOKEN）
#   - 模式 1 需要 gh CLI 已登录

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NPM_DIR="$SCRIPT_DIR/npm"
DRY_RUN=""
FROM_RELEASE=""
VERSION=""

# ── 解析参数 ──
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)      DRY_RUN="$1"; shift ;;
    --from-release) FROM_RELEASE="$2"; shift 2 ;;
    --version)      VERSION="$2"; shift 2 ;;
    *)              echo "Unknown arg: $1"; exit 1 ;;
  esac
done

# ── 从 GitHub Release 下载二进制 ──
if [ -n "$FROM_RELEASE" ]; then
  TAG="$FROM_RELEASE"

  # 提取版本号
  if [ -z "$VERSION" ]; then
    # cc-switch-client-v0.1.0 → 0.1.0 或 v0.1.0 → 0.1.0
    VERSION="${TAG#cc-switch-client-v}"
    VERSION="${VERSION#v}"
  fi

  echo "=== Downloading binaries from GitHub Release: $TAG ==="
  echo "Version: $VERSION"

  # 检查 gh CLI
  if ! command -v gh &>/dev/null; then
    echo "ERROR: gh CLI not found. Install: brew install gh"
    exit 1
  fi

  # 获取仓库
  REPO=$(git -C "$SCRIPT_DIR" remote get-url origin 2>/dev/null | sed -E 's|.*github.com[:/]([^/]+/[^/]+)(\.git)?|\1|' || echo "")

  if [ -z "$REPO" ]; then
    echo "ERROR: Cannot determine GitHub repo from git remote"
    exit 1
  fi

  echo "Repo: $REPO"

  # 创建临时目录
  DL_DIR=$(mktemp -d)
  trap "rm -rf $DL_DIR" EXIT

  # 下载所有平台二进制
  PLATFORMS="darwin-arm64 darwin-x64 linux-x64 linux-arm64 win32-x64"

  for platform in $PLATFORMS; do
    ext=""
    [[ "$platform" == "win32-x64" ]] && ext=".exe"
    remote_name="cc-switch-server-${platform}${ext}"

    echo "  Downloading $remote_name ..."
    if gh release download "$TAG" --repo "$REPO" --pattern "$remote_name" --dir "$DL_DIR" 2>/dev/null; then
      pkg_dir="$NPM_DIR/cc-switch-client-${platform}/bin"
      mkdir -p "$pkg_dir"
      cp "$DL_DIR/$remote_name" "$pkg_dir/cc-switch-server${ext}"
      chmod +x "$pkg_dir/cc-switch-server${ext}"
      echo "  ✅ $platform → $(du -h "$pkg_dir/cc-switch-server${ext}" | cut -f1)"
    else
      echo "  ⏭️  $platform — not found in release (skipped)"
    fi
  done
else
  # 本地模式：使用已有的二进制
  if [ -z "$VERSION" ]; then
    VERSION=$(node -e "console.log(require('$NPM_DIR/cc-switch-client/package.json').version)")
  fi
  echo "=== Using local binaries ==="
fi

echo ""
echo "Version: $VERSION"

# ── 检查哪些平台包有二进制 ──
echo ""
echo "--- Checking platform binaries ---"
publishable=()
PLATFORM_PACKAGES=(
  "cc-switch-client-darwin-arm64"
  "cc-switch-client-darwin-x64"
  "cc-switch-client-linux-x64"
  "cc-switch-client-linux-arm64"
  "cc-switch-client-win32-x64"
)

for pkg in "${PLATFORM_PACKAGES[@]}"; do
  bin="$NPM_DIR/$pkg/bin/cc-switch-server"
  binexe="$NPM_DIR/$pkg/bin/cc-switch-server.exe"
  if [ -f "$bin" ] || [ -f "$binexe" ]; then
    size=$(du -h "$bin" 2>/dev/null || du -h "$binexe" 2>/dev/null | cut -f1)
    echo "  ✅ $pkg — $size"
    publishable+=("$pkg")
  else
    echo "  ⏭️  $pkg — skip (no binary)"
  fi
done

if [ ${#publishable[@]} -eq 0 ]; then
  echo "ERROR: No publishable platform packages!"
  exit 1
fi

# ── 更新版本号 ──
echo ""
echo "--- Updating version to $VERSION ---"
MAIN_PKG="$NPM_DIR/cc-switch-client"

for pkg_dir in "$NPM_DIR"/cc-switch-client*; do
  node -e "
    const fs = require('fs');
    const p = '$pkg_dir/package.json';
    const pkg = JSON.parse(fs.readFileSync(p, 'utf8'));
    pkg.version = '$VERSION';
    if (pkg.optionalDependencies) {
      for (const dep of Object.keys(pkg.optionalDependencies)) {
        if (dep.startsWith('cc-switch-client-')) {
          pkg.optionalDependencies[dep] = '$VERSION';
        }
      }
    }
    fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');
  "
done
echo "  All packages updated to $VERSION"

# ── 发布 ──
echo ""
if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "--- DRY RUN: showing what would be published ---"
  for pkg in "${publishable[@]}"; do
    echo "  npm publish --access public  $NPM_DIR/$pkg"
  done
  echo "  npm publish --access public  $MAIN_PKG"
  echo ""
  echo "Run without --dry-run to publish."
else
  echo "--- Publishing platform packages ---"
  for pkg in "${publishable[@]}"; do
    echo "  Publishing $pkg..."
    (cd "$NPM_DIR/$pkg" && npm publish --access public) || echo "  ⚠️  Failed: $pkg"
  done

  echo ""
  echo "--- Publishing main package ---"
  echo "  Publishing cc-switch-client@$VERSION ..."
  (cd "$MAIN_PKG" && npm publish --access public) || echo "  ⚠️  Failed: cc-switch-client"

  echo ""
  echo "✅ Done! Published cc-switch-client@$VERSION"
  echo ""
  echo "Install:"
  echo "  bun add cc-switch-client"
  echo "  npm install cc-switch-client"
fi
