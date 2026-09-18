//! macOS 开机自启动管理。
//!
//! 使用系统原生的登录项接口 `SMAppService`（macOS 13+，ServiceManagement.framework）：
//!
//! - 启用：`SMAppService.mainApp.register()` —— 应用会出现在
//!   「系统设置 → 通用 → 登录项 → 打开时」，由系统在下次登录时代为启动，**不会立刻拉起新实例**；
//! - 禁用：`unregister()`；
//! - 状态：`status`（未注册 / 已启用 / 待用户批准 / 找不到应用）。
//!
//! ## 为什么不继续用 LaunchAgent
//!
//! 老版本把 `~/Library/LaunchAgents/com.astrabrew.launcher.plist` 写进启动项，然后用
//! `launchctl bootstrap` 加载。这条路径有两个硬伤：
//!
//! 1. plist 里 `RunAtLoad` 为真，`bootstrap` 会**立刻运行**该作业 —— 表现为「打开自启动开关就弹出
//!    一个新窗口」；
//! 2. 用户级 LaunchAgent 不属于系统「登录项」，因此在登录项列表里看不到它。
//!
//! 这里保留 [legacy] 相关代码只做两件事：清理历史遗留的 plist / 已注册服务（避免与新机制重复启动），
//! 以及在系统不支持 `SMAppService`（macOS 12 及更早）时**只写 plist、不 bootstrap** 地兜底。

use crate::lang::t;
use crate::lang::tf;
use std::ffi::{CString, c_char, c_int, c_void};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use objc2_06::msg_send;
use objc2_06::runtime::{AnyClass, AnyObject};
use objc2_foundation_06::NSError;

unsafe extern "C" {
    /// 动态加载系统框架（实现在 libdyld，经 libSystem 导出）。
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
}

const RTLD_LAZY: c_int = 0x1;
const SERVICE_MANAGEMENT: &str =
    "/System/Library/Frameworks/ServiceManagement.framework/ServiceManagement";

/// 确保 ServiceManagement 框架已加载（幂等），框架里的 `SMAppService` 才会被注册进运行时。
fn load_service_management() -> bool {
    static LOADED: OnceLock<bool> = OnceLock::new();
    *LOADED.get_or_init(|| {
        let Ok(path) = CString::new(SERVICE_MANAGEMENT) else {
            return false;
        };
        !unsafe { dlopen(path.as_ptr(), RTLD_LAZY) }.is_null()
    })
}

/// 老版本使用的 LaunchAgent 标签。
const LEGACY_LABEL: &str = "com.astrabrew.launcher";

/// 系统登录项状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItemStatus {
    /// 尚未注册。
    NotRegistered,
    /// 已启用，登录时会自动启动。
    Enabled,
    /// 已注册，但需要用户在「系统设置 → 登录项」中批准。
    RequiresApproval,
    /// 系统找不到应用（通常是没以 .app 形式放在可注册的位置）。
    NotFound,
    /// 当前系统没有 `SMAppService`（macOS 12 及更早）。
    Unsupported,
}

impl LoginItemStatus {
    /// 从 `SMAppServiceStatus` 原始值解析。
    const fn from_raw(raw: isize) -> Self {
        match raw {
            1 => Self::Enabled,
            2 => Self::RequiresApproval,
            3 => Self::NotFound,
            _ => Self::NotRegistered,
        }
    }

    /// 该状态是否表示「登录时会自动启动」。
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}

/// 取 `SMAppService.mainApp` 实例；系统不支持时返回 `None`。
fn main_app_service() -> Option<*mut AnyObject> {
    if !load_service_management() {
        return None;
    }
    // 类不存在即系统过老，回退到 legacy 方案。
    let class = AnyClass::get(c"SMAppService")?;
    let service: *mut AnyObject = unsafe { msg_send![class, mainAppService] };
    (!service.is_null()).then_some(service)
}

/// 查询登录项状态。
pub fn login_item_status() -> LoginItemStatus {
    let Some(service) = main_app_service() else {
        return LoginItemStatus::Unsupported;
    };
    let raw: isize = unsafe { msg_send![service, status] };
    LoginItemStatus::from_raw(raw)
}

/// 注册 / 注销登录项，返回是否成功。
///
/// `NSError**` 传空指针：失败原因用 [login_item_status] 复述即可，不必把系统错误原文透给用户。
fn register(service: *mut AnyObject) -> bool {
    let error: *mut *mut NSError = std::ptr::null_mut();
    unsafe { msg_send![service, registerAndReturnError: error] }
}

fn unregister(service: *mut AnyObject) -> bool {
    let error: *mut *mut NSError = std::ptr::null_mut();
    unsafe { msg_send![service, unregisterAndReturnError: error] }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn legacy_plist_path() -> PathBuf {
    home_dir()
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LEGACY_LABEL}.plist"))
}

/// 清理老版本留下的 LaunchAgent（只删除 plist）。
///
/// 必须清理：否则老用户的 LaunchAgent 会与新的登录项同时生效，登录时启动两份实例。
///
/// **刻意不调用 `launchctl bootout`**：`bootout` 会终止该作业的进程，
/// 而当前实例有可能正是本次登录由该 LaunchAgent 启动的 —— 那等于「切换自启动开关时把
/// 正在使用的应用杀掉」。只删文件即可：已加载的作业本次会话不会再触发（`KeepAlive` 为假），
/// 下次登录时 plist 已不存在，自然不会重复启动。
fn cleanup_legacy_agent() {
    let plist = legacy_plist_path();
    if plist.exists() {
        let _ = fs::remove_file(&plist);
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// macOS 12 及更早的兜底：只写 plist，**不调用 launchctl**。
///
/// `~/Library/LaunchAgents` 下的 plist 会在下次登录时被 launchd 自动读取，
/// 因此不需要（也不应该）手动 bootstrap —— 那会立刻拉起一份新实例。
fn legacy_set_auto_launch(enabled: bool) -> Result<(), String> {
    let plist = legacy_plist_path();
    if !enabled {
        cleanup_legacy_agent();
        return Ok(());
    }

    let executable = std::env::current_exe()
        .map_err(|error| tf("autolaunch.exe_path_failed", &[("error", &error)]))?;
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| tf("autolaunch.create_dir_failed", &[("error", &error)]))?;
    }
    let executable = escape_xml(&executable.to_string_lossy());
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LEGACY_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{executable}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
    );
    fs::write(&plist, contents)
        .map_err(|error| tf("autolaunch.write_plist_failed", &[("error", &error)]))
}

/// 设置开机自启动的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoLaunchOutcome {
    /// 已按预期生效。
    Applied,
    /// 已登记为登录项，但需要用户在「系统设置 → 登录项」中允许后才真正生效。
    NeedsApproval,
}

/// 设置开机自启动。
pub fn set_auto_launch(enabled: bool) -> Result<AutoLaunchOutcome, String> {
    // 老版本遗留的 LaunchAgent 一律清掉，避免登录时启动两份。
    cleanup_legacy_agent();

    let Some(service) = main_app_service() else {
        legacy_set_auto_launch(enabled)?;
        return Ok(AutoLaunchOutcome::Applied);
    };

    if !enabled {
        // 本来就没注册过时注销会失败，这不是错误。
        if !unregister(service) && login_item_status().is_active() {
            return Err(t("autolaunch.register_failed").to_owned());
        }
        return Ok(AutoLaunchOutcome::Applied);
    }

    // 已经是登录项（含「待批准」）就不用重复注册。
    if !login_item_status().is_active() && !register(service) {
        return Err(match login_item_status() {
            LoginItemStatus::NotFound => t("autolaunch.register_not_found").to_owned(),
            _ => t("autolaunch.register_failed").to_owned(),
        });
    }

    Ok(match login_item_status() {
        LoginItemStatus::RequiresApproval => AutoLaunchOutcome::NeedsApproval,
        _ => AutoLaunchOutcome::Applied,
    })
}

/// 当前是否已经启用开机自启动。
pub fn is_auto_launch_enabled() -> bool {
    let status = login_item_status();
    if status.is_active() {
        return true;
    }
    // 系统不支持原生登录项时，以历史 LaunchAgent 是否存在为准。
    matches!(status, LoginItemStatus::Unsupported) && legacy_plist_path().exists()
}

/// 打开系统设置中的「登录项」页面。
pub fn open_login_item_settings() -> Result<(), String> {
    let status = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
        .status()
        .map_err(|error| tf("autolaunch.open_settings_failed", &[("error", &error)]))?;
    if status.success() {
        Ok(())
    } else {
        Err(t("autolaunch.login_items_failed").to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_plist_path_points_into_launch_agents() {
        assert!(legacy_plist_path().ends_with("com.astrabrew.launcher.plist"));
    }

    #[test]
    fn status_maps_native_values() {
        assert_eq!(LoginItemStatus::from_raw(0), LoginItemStatus::NotRegistered);
        assert_eq!(LoginItemStatus::from_raw(1), LoginItemStatus::Enabled);
        assert_eq!(
            LoginItemStatus::from_raw(2),
            LoginItemStatus::RequiresApproval
        );
        assert_eq!(LoginItemStatus::from_raw(3), LoginItemStatus::NotFound);
        assert_eq!(LoginItemStatus::from_raw(99), LoginItemStatus::NotRegistered);
    }

    #[test]
    fn outcome_is_comparable() {
        assert_eq!(AutoLaunchOutcome::Applied, AutoLaunchOutcome::Applied);
        assert_ne!(AutoLaunchOutcome::Applied, AutoLaunchOutcome::NeedsApproval);
    }

    #[test]
    fn approval_counts_as_active() {
        assert!(LoginItemStatus::Enabled.is_active());
        assert!(LoginItemStatus::RequiresApproval.is_active());
        assert!(!LoginItemStatus::NotRegistered.is_active());
    }
}
