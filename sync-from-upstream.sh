#!/usr/bin/env bash
# sync-from-upstream.sh
#
# 将 src-tauri/src/ 的上游更新同步到 cc-switch-core/src/
#
# 工作流:
#   1. git merge upstream/main         # 先合并上游更新到 src-tauri/
#   2. ./sync-from-upstream.sh         # 运行本脚本
#   3. cargo check -p cc-switch-core   # 编译验证
#
# 策略:
#   - 126 个零修改文件: 直接覆盖
#   - 14 个有修改文件: 先覆盖，再自动应用机械替换规则
#   - cc-switch-core 独有文件 (events.rs, auth_provider.rs, core.rs 等): 不动

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SRC_DIR="src-tauri/src"
DST_DIR="cc-switch-core/src"

# ── Step 1: 复制零修改文件 ──
echo "=== Step 1: Syncing unchanged files ==="
synced=0
for f in $(find "$DST_DIR" -name '*.rs' -not -path '*/tests*' | sort); do
    rel="${f#$DST_DIR/}"
    orig="$SRC_DIR/$rel"
    if [ -f "$orig" ]; then
        if diff -q "$f" "$orig" > /dev/null 2>&1; then
            # 当前完全相同，直接用上游最新版覆盖
            cp "$orig" "$f"
            synced=$((synced + 1))
        fi
    fi
done
echo "  Synced $synced identical files from upstream"

# ── Step 2: 需要解耦的文件列表 ──
DECOUPLED_FILES=(
    "config.rs"
    "database/mod.rs"
    "proxy/failover_switch.rs"
    "proxy/forwarder.rs"
    "proxy/handler_context.rs"
    "proxy/response_processor.rs"
    "proxy/server.rs"
    "services/provider/mod.rs"
    "services/proxy.rs"
    "services/s3_auto_sync.rs"
    "services/speedtest.rs"
    "services/webdav_auto_sync.rs"
    "usage_events.rs"
)

echo ""
echo "=== Step 2: Re-decoupling modified files ==="

for f in "${DECOUPLED_FILES[@]}"; do
    orig="$SRC_DIR/$f"
    dest="$DST_DIR/$f"

    if [ ! -f "$orig" ]; then
        echo "  SKIP $f (not in upstream)"
        continue
    fi

    echo "  Decoupling $f ..."

    # 先用上游版本覆盖
    cp "$orig" "$dest"

    # ── 应用机械替换规则 ──

    # 规则 1: 替换 tauri imports
    # 1a: 移除 tauri::{Emitter, Manager} 等整行
    sed -i '' '/^use tauri::.*Emitter/d' "$dest"
    sed -i '' '/^use tauri::.*Manager/d' "$dest"
    sed -i '' '/^use tauri::.*AppHandle/d' "$dest"
    sed -i '' '/^use tauri::.*{AppHandle, Emitter}/d' "$dest"
    sed -i '' '/^use tauri::{AppHandle, Emitter}/d' "$dest"

    # 1b: 替换 commands imports
    sed -i '' 's/^use crate::commands::{CodexOAuthState, CopilotAuthState}/use crate::auth_provider::AuthProvider;\nuse crate::events::CoreEvents;/' "$dest"
    sed -i '' 's/^use crate::commands::{CopilotAuthState, CodexOAuthState}/use crate::auth_provider::AuthProvider;\nuse crate::events::CoreEvents;/' "$dest"

    # 规则 2: app_handle → events / auth_provider
    # 在 forwarder.rs 的 struct 定义和函数签名中
    # app_handle: Option<tauri::AppHandle> → events + auth_provider
    # 这个替换太复杂无法用 sed 准确完成，需要手动检查

    # 规则 3: app.emit → events.emit_xxx (针对 failover_switch, s3, webdav)
    # 这些也需要上下文感知，标记为需手动检查

    # 规则 4: OnceLock → RwLock (config.rs, usage_events.rs)
    sed -i '' 's/use std::sync::OnceLock;/use std::sync::RwLock;/' "$dest"
    sed -i '' 's/static .*: OnceLock</static &/' "$dest"  # 需要手动确认

    # 规则 5: usage_events.rs 特定替换
    if [ "$f" = "usage_events.rs" ]; then
        sed -i '' 's/use crate::events::CoreEvents;/use crate::events::CoreEvents;/' "$dest"
    fi

    # 规则 6: speedtest.rs: tauri::async_runtime → tokio::test
    sed -i '' 's/tauri::async_runtime::block_on/tokio::test/g' "$dest"

    echo "    → Applied mechanical rules. REVIEW MANUALLY."
done

# ── Step 3: 检查是否有新增文件 ──
echo ""
echo "=== Step 3: Checking for new upstream files ==="
new_files=0
for f in $(find "$SRC_DIR" -name '*.rs' -not -path '*/commands/*' -not -path '*/tray*' -not -path '*/app_store*' -not -path '*/bridge/*' | sort); do
    rel="${f#$SRC_DIR/}"
    dest="$DST_DIR/$rel"
    if [ ! -f "$dest" ]; then
        echo "  NEW: $rel"
        new_files=$((new_files + 1))
    fi
done
if [ $new_files -eq 0 ]; then
    echo "  No new files"
fi

# ── Step 4: 检查是否有已删除的上游文件 ──
echo ""
echo "=== Step 4: Checking for deleted upstream files ==="
deleted=0
for f in $(find "$DST_DIR" -name '*.rs' -not -path '*/tests*' | sort); do
    rel="${f#$DST_DIR/}"
    orig="$SRC_DIR/$rel"
    # 只检查从 src-tauri 复制来的文件（排除 cc-switch-core 独有文件）
    case "$rel" in
        events.rs|auth_provider.rs|core.rs) continue ;;
    esac
    if [ ! -f "$orig" ] && [ "$rel" != "lib.rs" ]; then
        echo "  DELETED in upstream: $rel (may need manual cleanup)"
        deleted=$((deleted + 1))
    fi
done
if [ $deleted -eq 0 ]; then
    echo "  No deleted files"
fi

echo ""
echo "=== Sync complete ==="
echo ""
echo "Next steps:"
echo "  1. Manually review the ${#DECOUPLED_FILES[@]} decoupled files"
echo "     (sed rules handle ~70% of changes; complex ones need manual work)"
echo "  2. Key patterns to apply manually:"
echo "     - app_handle: Option<tauri::AppHandle>  →  events: Option<Arc<dyn CoreEvents>>"
echo "     - app_handle.state::<CopilotAuthState>() →  self.auth_provider.get_copilot_token()"
echo "     - app.emit(\"provider-switched\", data)    →  events.emit_provider_switched(...)"
echo "     - APP_HANDLE: OnceLock<AppHandle>         →  EVENTS: RwLock<Option<Arc<dyn CoreEvents>>>"
echo "  3. Run: cargo check -p cc-switch-core"
echo "  4. Run: cargo test -p cc-switch-core -- --test-threads=1"
