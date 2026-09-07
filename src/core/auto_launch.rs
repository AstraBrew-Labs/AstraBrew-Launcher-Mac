//! macOS 开机自启动管理。
//!
//! 通过用户级 LaunchAgent 管理登录自启动：
//! - 启用时写入 `~/Library/LaunchAgents/com.astrabrew.launcher.plist`
//! - 立即尝试使用 `launchctl bootstrap` 加载
//! - 禁用时执行 `launchctl bootout` 并移除 plist
//!
//! 这样既能让老用户直接升级保留开机自启动，也能在本应用内完成切换。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LABEL: &str = "com.astrabrew.launcher";

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn plist_path() -> PathBuf {
    home_dir()
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

fn launchctl_domain() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|error| format!("无法获取当前用户 UID：{error}"))?;
    if !output.status.success() {
        return Err("无法获取当前用户 UID。".to_owned());
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if uid.is_empty() {
        return Err("当前用户 UID 为空。".to_owned());
    }
    Ok(format!("gui/{uid}"))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn executable_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| format!("无法获取当前可执行文件路径：{error}"))
}

fn write_plist(executable: &Path) -> Result<(), String> {
    let plist = plist_path();
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建 LaunchAgents 目录：{error}"))?;
    }

    let executable = escape_xml(&executable.to_string_lossy());
    let contents = format!(
        r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
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

    fs::write(&plist, contents).map_err(|error| format!("无法写入自启动配置：{error}"))
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|error| format!("无法执行 launchctl：{error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if stderr.is_empty() {
        format!("launchctl {:?} 执行失败。", args)
    } else {
        stderr
    })
}

fn launchctl_print_enabled() -> bool {
    let Ok(domain) = launchctl_domain() else {
        return false;
    };
    let target = format!("{domain}/{LABEL}");
    Command::new("launchctl")
        .args(["print", &target])
        // 查询未注册服务是正常分支，不应把 launchctl 的错误输出到启动器终端。
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 设置开机自启动。
pub fn set_auto_launch(enabled: bool) -> Result<(), String> {
    let plist = plist_path();
    let domain = launchctl_domain()?;
    if enabled {
        write_plist(&executable_path()?)?;
        let bootstrap = launchctl(&["bootstrap", &domain, plist.to_string_lossy().as_ref()]);
        if bootstrap.is_err() && !launchctl_print_enabled() {
            let _ = fs::remove_file(&plist);
            return bootstrap;
        }
        Ok(())
    } else {
        let _ = launchctl(&["bootout", &domain, plist.to_string_lossy().as_ref()]);
        let _ = fs::remove_file(&plist);
        Ok(())
    }
}

/// 当前是否已经注册了开机自启动。
pub fn is_auto_launch_enabled() -> bool {
    launchctl_print_enabled() || plist_path().exists()
}

/// 打开系统设置中的「登录项」页面。
pub fn open_login_item_settings() -> Result<(), String> {
    let status = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
        .status()
        .map_err(|error| format!("无法打开系统设置：{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("无法打开系统设置中的登录项页面。".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_path_points_into_launch_agents() {
        assert!(plist_path().ends_with("com.astrabrew.launcher.plist"));
    }
}
