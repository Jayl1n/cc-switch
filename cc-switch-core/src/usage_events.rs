//! 使用统计实时刷新事件模块
//!
//! 当 `proxy_request_logs` 表写入新数据时（代理日志、会话同步、归档等），
//! 通过本模块调用 CoreEvents 回调，让消费者立刻得到通知而无需等待轮询。
//!
//! ## 设计要点
//! - 全局单例 CoreEvents：写日志路径上不持有具体实现，用 OnceCell 共享。
//! - 200ms 防抖合并：流式响应等场景在短时间内可能写入多条日志，
//!   合并成一次通知避免消费者连续处理。
//! - 不阻塞写入：通知失败仅记录 warn 日志，不向上传播错误。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use crate::events::CoreEvents;

/// 事件名常量（保留用于日志）
pub const EVENT_USAGE_LOG_RECORDED: &str = "usage-log-recorded";

/// 防抖窗口：合并 200ms 内的多次通知。
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(200);

static EVENTS: OnceLock<std::sync::Arc<dyn CoreEvents>> = OnceLock::new();

/// 防抖标记：true 表示已有调度任务在等待 emit，后续通知合并到该任务。
static EMIT_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// 在应用 setup 阶段调用一次，注入 CoreEvents。
///
/// 重复调用是无害的（OnceLock 仅首次写入生效）。
pub fn init(events: std::sync::Arc<dyn CoreEvents>) {
    if EVENTS.set(events).is_err() {
        log::debug!("usage_events::init 重复调用，已忽略");
    } else {
        log::info!("[usage-event] CoreEvents 已注入，事件推送启用");
    }
}

/// 通知有新的使用日志写入。
///
/// 调用方**不**需要持有 CoreEvents，可以从任意线程/任意写入路径调用。
/// 内部 200ms 防抖合并，绝不阻塞调用线程。
pub fn notify_log_recorded() {
    // CoreEvents 未注入（典型出现在单元测试或 setup 之前）：直接放弃。
    let Some(events) = EVENTS.get() else {
        return;
    };

    // 已有调度任务：本次通知被合并到既有任务里，无需再起线程。
    if EMIT_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }

    let events = events.clone();
    std::thread::spawn(move || {
        std::thread::sleep(DEBOUNCE_WINDOW);
        // 必须先清标志再通知：万一通知期间又有新通知进来，
        // 下一轮防抖窗口会重新调度，不会丢失。
        EMIT_SCHEDULED.store(false, Ordering::Release);

        events.emit_usage_log_recorded();
    });
}
