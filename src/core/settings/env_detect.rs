use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// macOS 上 Homebrew 可能的 bin 目录。
/// 打包后的 .app PATH 不含这些路径，必须手动解析命令位置。
const HOMEBREW_BIN_PATHS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

/// 解析命令的完整路径：
/// 1. 在 Homebrew bin 目录中查找
/// 2. 回退到裸命令名（系统 PATH 中的工具，如 git）
pub fn resolve_command(name: &str) -> String {
    for base in HOMEBREW_BIN_PATHS {
        let full = format!("{}/{}", base, name);
        if Path::new(&full).exists() {
            return full;
        }
    }
    name.to_string()
}

/// 创建 Command，自动解析路径并确保子进程 PATH 包含 Homebrew bin 目录。
///
/// 打包后的 .app 中 PATH 极简（/usr/bin:/bin:/usr/sbin:/sbin），
/// 不包含 Homebrew 路径。即使 resolve_command 找到了命令的绝对路径，
/// 如果命令内部通过 shebang（如 `#!/usr/bin/env node`）依赖其他工具，
/// 仍会因子进程找不到依赖而失败。因此必须在启动子进程前补全 PATH。
fn cmd(name: &str) -> Command {
    let mut cmd = Command::new(resolve_command(name));
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extra_paths = HOMEBREW_BIN_PATHS.join(":");
    let new_path = format!("{}:{}", extra_paths, current_path);
    cmd.env("PATH", new_path);
    cmd
}

/// 检测 Homebrew 版本，返回版本号字符串，如 "4.2.0"
pub fn detect_homebrew() -> Option<String> {
    let output = cmd("brew").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 输出格式: "Homebrew 4.2.0" 或 "Homebrew 4.2.0-xxx"
    parse_homebrew_version(&stdout)
}

fn parse_homebrew_version(output: &str) -> Option<String> {
    // 提取 "Homebrew X.Y.Z" 中的版本号
    let prefix = "Homebrew ";
    if let Some(pos) = output.find(prefix) {
        let rest = &output[pos + prefix.len()..];
        // 取第一个空白字符之前的部分
        let version = rest.split_whitespace().next()?;
        // 去掉尾部可能的非数字后缀（如 -xxx）
        let version = version.split('-').next()?;
        Some(version.to_string())
    } else {
        None
    }
}

/// 检测 Git 版本，返回版本号字符串，如 "2.39.0"
pub fn detect_git() -> Option<String> {
    let output = cmd("git").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 输出格式: "git version 2.39.0" 或 "git version 2.39.0 (Apple Git-xxx)"
    parse_git_version(&stdout)
}

fn parse_git_version(output: &str) -> Option<String> {
    let prefix = "git version ";
    if let Some(pos) = output.find(prefix) {
        let rest = &output[pos + prefix.len()..];
        let version = rest.split_whitespace().next()?;
        Some(version.to_string())
    } else {
        None
    }
}

/// 检测 Node.js 版本，返回版本号字符串，如 "v22.1.0"。
///
/// `node@24` 在 Homebrew 中通常是 keg-only，未必会链接到 PATH；优先读取
/// Homebrew formula 的实际 bin/node，再回退到系统 PATH 中的 node。
pub fn detect_nodejs() -> Option<String> {
    if let Some(version) = detect_nodejs_formula() {
        return Some(version);
    }

    let output = cmd("node").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn detect_nodejs_formula() -> Option<String> {
    let prefix = cmd("brew").args(["--prefix", "node@24"]).output().ok()?;
    if !prefix.status.success() {
        return None;
    }

    let prefix = String::from_utf8_lossy(&prefix.stdout).trim().to_owned();
    let node = Path::new(&prefix).join("bin/node");
    let output = Command::new(node).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!version.is_empty()).then_some(version)
}

/// 检测 Caddy 版本，返回版本号字符串，如 "v2.9.1"
pub fn detect_caddy() -> Option<String> {
    let output = cmd("caddy").arg("version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 输出格式: "v2.9.1 h1:..." 取第一段
    let version = stdout.split_whitespace().next()?;
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// 检测 PM2 版本，返回版本号字符串，如 "7.0.1"
/// pm2 --version 或 pm2 -v 在首次运行时可能夹杂 daemon 启动日志，
/// 因此合并 stdout + stderr 后用正则提取 X.Y.Z 格式的版本号。
pub fn detect_pm2() -> Option<String> {
    let output = cmd("pm2").arg("-v").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // 从混合输出中提取第一个 MAJOR.MINOR.PATCH 格式的版本号
    extract_semver(&combined)
}

/// 从文本中提取第一个符合 X.Y.Z 模式的版本号
fn extract_semver(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // 找数字开头
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0u8;
            let mut valid = true;
            i += 1;
            while i < len && dots < 2 {
                if bytes[i].is_ascii_digit() {
                    i += 1;
                } else if bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                    dots += 1;
                    i += 1; // 跳过 '.'
                } else {
                    valid = false;
                    break;
                }
            }
            if valid && dots == 2 {
                // 截断尾部非数字字符（如换行后的额外文本）
                let mut end = i;
                while end > start && !bytes[end - 1].is_ascii_digit() {
                    end -= 1;
                }
                return Some(String::from_utf8_lossy(&bytes[start..end]).to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

/// 解析 semver 主版本号
fn parse_major(version: &str) -> Option<u32> {
    let version = version.trim_start_matches('v').trim_start_matches('V');
    version.split('.').next()?.parse::<u32>().ok()
}

/// Homebrew 版本是否低于 5.0.0
pub fn is_homebrew_outdated(version: &str) -> bool {
    match parse_major(version) {
        Some(major) => major < 5,
        None => false,
    }
}

/// Node.js 版本是否低于 v22
pub fn is_nodejs_outdated(version: &str) -> bool {
    match parse_major(version) {
        Some(major) => major < 22,
        None => false,
    }
}

/// 运行 brew install <package> 并返回日志。
pub fn run_brew_install(package: &str, sender: std::sync::mpsc::Sender<String>) {
    let detect_target = match package {
        "git" => "git",
        "node@24" => "nodejs",
        "caddy" => "caddy",
        _ => "",
    };

    let mut command = cmd("brew");
    command.args(["install", package]);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            send_failure(sender, format!("无法启动 brew install {package}：{error}"));
            return;
        }
    };

    run_logged_command(child, sender, detect_target);
}

fn detect_version(target: &str) -> Option<String> {
    match target {
        "homebrew" => detect_homebrew(),
        "git" => detect_git(),
        "nodejs" => detect_nodejs(),
        "caddy" => detect_caddy(),
        "pm2" => detect_pm2(),
        _ => None,
    }
}

/// 运行命令、转发输出并严格根据退出状态报告成功或失败。
///
/// 旧实现无论子进程是否启动成功都会发送 `__DONE__`，导致 UI 显示安装成功，
/// 但实际上 brew/npm 可能已经失败。这里先等待命令退出，再检测版本，只有两者
/// 都成功时才发送 `__DONE__`。
fn run_logged_command(
    mut child: Child,
    sender: std::sync::mpsc::Sender<String>,
    detect_target: &str,
) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let tx_stdout = sender.clone();
    let tx_stderr = sender.clone();
    let tx_final = sender;
    let (done_tx, done_rx) = std::sync::mpsc::channel();

    if let Some(out) = stdout {
        let done = done_tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(out);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        let cleaned = strip_ansi(&line).trim().to_string();
                        if !cleaned.is_empty() {
                            let _ = tx_stdout.send(cleaned);
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = done.send(());
        });
    } else {
        let _ = done_tx.send(());
    }

    if let Some(err) = stderr {
        let done = done_tx;
        std::thread::spawn(move || {
            let reader = BufReader::new(err);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        let cleaned = strip_ansi(&line).trim().to_string();
                        if !cleaned.is_empty() {
                            let _ = tx_stderr.send(cleaned);
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = done.send(());
        });
    } else {
        let _ = done_tx.send(());
    }

    // 先等输出线程完成，确保日志完整；随后等待并检查真实退出码。
    let _ = done_rx.recv();
    let _ = done_rx.recv();

    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            send_failure(tx_final, format!("等待安装进程结束失败：{error}"));
            return;
        }
    };

    if !status.success() {
        let code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "未知".to_owned());
        send_failure(
            tx_final,
            format!("命令执行失败（退出码：{code}），请查看上方日志。"),
        );
        return;
    }

    if let Some(version) = detect_version(detect_target) {
        let _ = tx_final.send(format!("__VERSION__:{version}"));
        let _ = tx_final.send("__DONE__".to_string());
    } else {
        send_failure(
            tx_final,
            format!("命令已完成，但未检测到 {detect_target} 版本，请确认安装是否成功。"),
        );
    }
}

/// 运行 npm install -g <package> 并返回日志（用于 PM2 等全局 npm 包安装）。
pub fn run_npm_install_global(package: &str, sender: std::sync::mpsc::Sender<String>) {
    let mut command = cmd("npm");
    command.args(["install", "-g", package]);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            send_failure(
                sender,
                format!("无法启动 npm install -g {package}：{error}"),
            );
            return;
        }
    };

    run_logged_command(child, sender, "pm2");
}

fn send_failure(sender: std::sync::mpsc::Sender<String>, message: String) {
    let _ = sender.send(format!("__ERROR__:{message}"));
    let _ = sender.send("__FAILED__".to_string());
}

/// 简易 ANSI 转义序列清理（SGR 颜色码 + 光标控制）
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // skip '['
            // 跳过参数部分 (数字和分号)
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == ';' {
                    chars.next();
                } else {
                    break;
                }
            }
            // 跳过终止字符 (通常是 m，但也可能是其他)
            chars.next();
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_homebrew_version() {
        assert_eq!(
            parse_homebrew_version("Homebrew 4.2.0"),
            Some("4.2.0".to_string())
        );
        assert_eq!(
            parse_homebrew_version("Homebrew 5.0.0-xxx"),
            Some("5.0.0".to_string())
        );
    }

    #[test]
    fn test_parse_git_version() {
        assert_eq!(
            parse_git_version("git version 2.39.0"),
            Some("2.39.0".to_string())
        );
        assert_eq!(
            parse_git_version("git version 2.39.0 (Apple Git-xxx)"),
            Some("2.39.0".to_string())
        );
    }

    #[test]
    fn test_is_homebrew_outdated() {
        assert!(is_homebrew_outdated("4.2.0"));
        assert!(!is_homebrew_outdated("5.0.0"));
        assert!(!is_homebrew_outdated("5.1.0"));
    }

    #[test]
    fn test_is_nodejs_outdated() {
        assert!(is_nodejs_outdated("v18.19.0"));
        assert!(!is_nodejs_outdated("v22.0.0"));
        assert!(!is_nodejs_outdated("v23.1.0"));
    }

    #[test]
    fn test_parse_major() {
        assert_eq!(parse_major("4.2.0"), Some(4));
        assert_eq!(parse_major("v22.1.0"), Some(22));
        assert_eq!(parse_major("v5.0.0"), Some(5));
    }

    #[test]
    fn failed_command_emits_failure_marker_instead_of_done() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf 'failed output\n'; exit 7"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().expect("spawn test command");
        let (sender, receiver) = std::sync::mpsc::channel();

        run_logged_command(child, sender, "");
        let messages = receiver.into_iter().collect::<Vec<_>>();

        assert!(messages.iter().any(|message| message == "failed output"));
        assert!(
            messages
                .iter()
                .any(|message| message.starts_with("__ERROR__:"))
        );
        assert!(messages.iter().any(|message| message == "__FAILED__"));
        assert!(!messages.iter().any(|message| message == "__DONE__"));
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("hello"), "hello");
        assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi("\x1b[1;32mworld\x1b[0m"), "world");
        assert_eq!(strip_ansi("no ansi here"), "no ansi here");
    }
}
