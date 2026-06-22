//! macOS 权限检测与引导
//!
//! 目前覆盖「完全磁盘访问权限」（Full Disk Access, FDA）。
//!
//! 检测原理：macOS 对未授予 FDA 的应用，`NSFileManager` 返回的
//! `homeDirectoryForCurrentUser` 会被重定向到沙箱容器路径
//! （`~/Library/Containers/<bundle-id>/...`），而非真实用户主目录。
//! 通过对比二者路径前缀即可判断权限是否已授予。
//!
//! Apple 未提供查询 FDA 状态的官方 API，这是应用层唯一的非侵入检测方式。

#[cfg(target_os = "macos")]
mod imp {
    use objc::{class, msg_send, sel};
    #[allow(unused_imports)]
    use objc::sel_impl;

    /// 读取真实环境变量 HOME（不受沙箱重定向影响）
    fn real_home() -> Option<String> {
        std::env::var("HOME").ok().filter(|h| !h.is_empty())
    }

    /// 通过 NSFileManager 读取 homeDirectoryForCurrentUser（沙箱应用会被重定向）
    fn ns_home() -> Option<String> {
        unsafe {
            let fm: *mut objc::runtime::Object = msg_send![class!(NSFileManager), defaultManager];
            if fm.is_null() {
                return None;
            }
            let url: *mut objc::runtime::Object = msg_send![fm, homeDirectoryForCurrentUser];
            if url.is_null() {
                return None;
            }
            let path: *mut objc::runtime::Object = msg_send![url, path];
            if path.is_null() {
                return None;
            }
            let c_str: *const std::os::raw::c_char = msg_send![path, UTF8String];
            if c_str.is_null() {
                return None;
            }
            let s = std::ffi::CStr::from_ptr(c_str).to_string_lossy().to_string();
            Some(s)
        }
    }

    /// 是否已授予完全磁盘访问权限。
    ///
    /// 判定：NSFileManager 给出的 HOME 与真实 HOME 一致 → 已授权；
    /// 若被重定向到 `Library/Containers/...` → 未授权。
    pub fn is_full_disk_access_granted() -> bool {
        let real = match real_home() {
            Some(h) => h,
            None => return true, // 无法读取环境变量时不阻断，保守视为已授权
        };
        let ns = match ns_home() {
            Some(h) => h,
            None => return true, // ObjC 调用失败时不阻断
        };
        // 关键判定：NSFileManager 路径包含 Library/Containers → 被沙箱重定向
        !ns.contains("Library/Containers") && ns == real
    }

    /// 打开「系统设置 → 隐私与安全性 → 完全磁盘访问权限」
    pub fn open_full_disk_access_settings() {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_full_disk_access_granted() -> bool {
        true
    }
    pub fn open_full_disk_access_settings() {}
}

/// 是否已授予完全磁盘访问权限
pub fn is_full_disk_access_granted() -> bool {
    imp::is_full_disk_access_granted()
}

/// 跳转到「系统设置 → 完全磁盘访问权限」面板
pub fn open_full_disk_access_settings() {
    imp::open_full_disk_access_settings();
}
