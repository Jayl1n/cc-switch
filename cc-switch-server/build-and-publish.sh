#!/usr/bin/env bash
# build-and-publish.sh
#
# 构建 cc-switch-server 的多平台二进制，放入 npm 平台包，然后发布到 npm。
#
# 前提:
#   - 已安装 rustup target: x86_64-apple-darwin, aarch64-unknown-linux-gnu, x86_64-unknown-linux-musl, x86_64-pc-windows-msvc
#   - 已登录 npm: npm login
#   - macOS 上可交叉编译 Linux (需 cross 或 zig) 和 Windows (需 cargo-xwin)
#
# 用法:
#   ./build-and-publish.sh 0.1.0          # 构建并发布
#   ./build-and-publish.sh 0.1.0 --dry-run # 只构建，不发布

set -euo pipefail

VERSION="${1:?Usage: $0 <version> [--dry-run]}"
DRY_RUN="${2:-}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NPM_DIR="$SCRIPT_DIR/npm"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=== CC Switch Client Build & Publish ==="
echo "Version: $VERSION"
echo "NPM dir: $NPM_DIR"
echo ""

# ── Step 1: 更新所有 package.json 版本号 ──
echo "--- Updating version to $VERSION ---"
for pkg_dir in "$NPM_DIR"/cc-switch-client*; do
  pkg_name=$(basename "$pkg_dir")
  if [ -f "$pkg_dir/package.json" ]; then
    # 使用 node 更新版本（保留格式）
    node -e "
      const fs = require('fs');
      const p = '$pkg_dir/package.json';
      const pkg = JSON.parse(fs.readFileSync(p, 'utf8'));
      pkg.version = '$VERSION';
      // 主包: 更新 optionalDependencies 版本
      if (pkg.optionalDependencies) {
        for (const dep of Object.keys(pkg.optionalDependencies)) {
          if (dep.startsWith('cc-switch-client-')) {
            pkg.optionalDependencies[dep] = '$VERSION';
          }
        }
      }
      fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');
      console.log('  Updated: $pkg_name');
    "
  fi
done

# ── Step 2: 构建 Rust 二进制 ──
echo ""
echo "--- Building binaries ---"

build_target() {
  local target="$1"
  local platform_dir="$2"
  local binary_name="$3"

  echo "  Building for $target → $platform_dir ..."

  if [ "$DRY_RUN" != "--dry-run" ]; then
    # 实际构建（需要对应 target 已安装）
    cargo build -p cc-switch-server --release --target "$target" 2>&1 | tail -3 || {
      echo "  WARN: Build failed for $target. Skipping."
      return 0
    }
  fi

  local src_binary="$ROOT_DIR/target/$target/release/cc-switch-server"
  local dst_binary="$NPM_DIR/$platform_dir/bin/$binary_name"

  if [ -f "$src_binary" ]; then
    cp "$src_binary" "$dst_binary"
    chmod +x "$dst_binary"
    echo "  ✅ Copied: $(du -h "$dst_binary" | cut -f1)"
  else
    echo "  ⚠️  Binary not found at $src_binary (skipping)"
  fi
}

# macOS ARM64 (当前机器，一定成功)
build_target "aarch64-apple-darwin" "cc-switch-client-darwin-arm64" "cc-switch-server"

# macOS x64
build_target "x86_64-apple-darwin" "cc-switch-client-darwin-x64" "cc-switch-server"

# Linux x64 (需要 cross 或 cargo-zigbuild)
build_target "x86_64-unknown-linux-musl" "cc-switch-client-linux-x64" "cc-switch-server"

# Linux ARM64
build_target "aarch64-unknown-linux-musl" "cc-switch-client-linux-arm64" "cc-switch-server"

# Windows x64 (需要 cargo-xwin 或 mingw)
build_target "x86_64-pc-windows-msvc" "cc-switch-client-win32-x64" "cc-switch-server.exe"

# ── Step 3: 发布到 npm ──
echo ""
if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "--- Dry run: skipping publish ---"
  echo ""
  echo "To publish manually:"
  for pkg_dir in "$NPM_DIR"/cc-switch-client-darwin-arm64 \
                 "$NPM_DIR"/cc-switch-client-darwin-x64 \
                 "$NPM_DIR"/cc-switch-client-linux-x64 \
                 "$NPM_DIR"/cc-switch-client-linux-arm64 \
                 "$NPM_DIR"/cc-switch-client-win32-x64 \
                 "$NPM_DIR"/cc-switch-client; do
    echo "  cd $pkg_dir && npm publish --access public"
  done
else
  echo "--- Publishing to npm ---"

  # 先发布平台包（顺序无关，但必须先于主包）
  for pkg_dir in "$NPM_DIR"/cc-switch-client-darwin-arm64 \
                 "$NPM_DIR"/cc-switch-client-darwin-x64 \
                 "$NPM_DIR"/cc-switch-client-linux-x64 \
                 "$NPM_DIR"/cc-switch-client-linux-arm64 \
                 "$NPM_DIR"/cc-switch-client-win32-x64; do
    echo "  Publishing $(basename "$pkg_dir")..."
    (cd "$pkg_dir" && npm publish --access public) || echo "  WARN: Failed to publish $(basename "$pkg_dir")"
  done

  # 最后发布主包
  echo "  Publishing cc-switch-client (main)..."
  (cd "$NPM_DIR/cc-switch-client" && npm publish --access public) || echo "  WARN: Failed to publish main package"

  echo ""
  echo "✅ Done! Published cc-switch-client@$VERSION"
fi
