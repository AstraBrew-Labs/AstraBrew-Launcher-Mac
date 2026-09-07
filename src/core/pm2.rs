//! PM2 进程管理封装。
//!
//! 所有 PM2 CLI 调用都由控制台运行时线程执行，避免阻塞 iced 主线程。

use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

/// 启动器托管的固定 PM2 进程名。
pub const PROCESS_NAME: &str = "astrabrew-launcher-sillytavern";

/// PM2 返回的精简进程状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub status: String,
    pub pid: Option<u32>,
    pub cwd: Option<String>,
}

/// PM2 CLI 管理器。
#[derive(Debug, Default)]
pub struct Pm2Manager;

impl Pm2Manager {
    /// 检查 PM2 是否可执行。
    pub fn is_installed() -> bool {
        pm2_command()
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// 使用 PM2 启动 SillyTavern。
    pub fn start(
        &self,
        working_dir: &Path,
        node_args: Option<&str>,
        app_args: &[String],
        environment: &[(String, String)],
    ) -> Result<(), String> {
        if self.info()?.is_some() {
            self.delete()?;
        }
        let mut command = pm2_command();
        command
            .arg("start")
            .arg("server.js")
            .arg("--name")
            .arg(PROCESS_NAME)
            .current_dir(working_dir);
        if let Some(node_args) = node_args {
            command.arg("--node-args").arg(node_args);
        }
        if !app_args.is_empty() {
            command.arg("--").args(app_args);
        }
        for (key, value) in environment {
            command.env(key, value);
        }
        run_checked(command, "PM2 启动失败")
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut command = pm2_command();
        command.arg("stop").arg(PROCESS_NAME);
        run_checked(command, "PM2 停止失败")
    }

    pub fn restart(&self) -> Result<(), String> {
        let mut command = pm2_command();
        command.arg("restart").arg(PROCESS_NAME).arg("--update-env");
        run_checked(command, "PM2 重启失败")
    }

    pub fn delete(&self) -> Result<(), String> {
        let mut command = pm2_command();
        command.arg("delete").arg(PROCESS_NAME);
        run_checked(command, "PM2 删除进程失败")
    }

    /// 读取 PM2 中当前托管进程的状态。
    pub fn info(&self) -> Result<Option<ProcessInfo>, String> {
        let output = pm2_command()
            .arg("jlist")
            .output()
            .map_err(|error| format!("无法查询 PM2 状态：{error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        let values = parse_jlist_output(&output.stdout).map_err(|error| {
            let preview = String::from_utf8_lossy(&output.stdout)
                .chars()
                .take(240)
                .collect::<String>();
            if preview.trim().is_empty() {
                format!("PM2 状态格式无效：{error}；命令没有返回 JSON。")
            } else {
                format!("PM2 状态格式无效：{error}；输出：{}", preview.trim())
            }
        })?;
        let Some(value) = values.iter().find(|value| {
            value.get("name").and_then(Value::as_str) == Some(PROCESS_NAME)
        }) else {
            return Ok(None);
        };
        let environment = value.get("pm2_env").and_then(Value::as_object);
        Ok(Some(ProcessInfo {
            status: environment
                .and_then(|env| env.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            pid: value
                .get("pid")
                .and_then(Value::as_u64)
                .and_then(|pid| u32::try_from(pid).ok())
                .filter(|pid| *pid > 0),
            cwd: environment
                .and_then(|env| env.get("pm_cwd"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        }))
    }

    /// 返回日志尾部安全读取起点，并对齐到下一行边界。
    pub fn tail_offset(&self, error_log: bool, max_bytes: u64) -> u64 {
        let path = log_path(error_log);
        let Ok(file) = fs::File::open(path) else {
            return 0;
        };
        let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let start = length.saturating_sub(max_bytes);
        if start == 0 {
            return 0;
        }
        let mut reader = BufReader::new(file);
        // 起点正好位于换行之后时已经对齐，不应再丢弃一条完整日志。
        if reader.seek(SeekFrom::Start(start - 1)).is_err() {
            return 0;
        }
        let mut previous = [0_u8; 1];
        if reader.read_exact(&mut previous).is_ok() && previous[0] == b'\n' {
            return start;
        }
        if reader.seek(SeekFrom::Start(start)).is_err() {
            return 0;
        }
        let mut partial_line = Vec::new();
        if reader.read_until(b'\n', &mut partial_line).is_err() {
            return start;
        }
        reader.stream_position().unwrap_or(start)
    }

    /// 从 PM2 日志文件的指定字节位置读取增量内容。
    pub fn read_log(&self, error_log: bool, offset: &mut u64) -> Result<Vec<String>, String> {
        let path = log_path(error_log);
        let mut file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("无法读取 PM2 日志 {}：{error}", path.display())),
        };
        let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        if *offset > length {
            *offset = 0;
        }
        file.seek(SeekFrom::Start(*offset))
            .map_err(|error| format!("无法定位 PM2 日志：{error}"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("无法读取 PM2 日志：{error}"))?;
        *offset = file.stream_position().unwrap_or(length);
        Ok(String::from_utf8_lossy(&bytes)
            .lines()
            .map(str::to_owned)
            .collect())
    }

    /// 清空当前托管进程的日志，确保新会话不会混入旧输出。
    pub fn clear_logs(&self) {
        // 先通知 PM2 关闭并刷新当前日志流，再直接截断文件处理残留内容。
        let _ = pm2_command()
            .arg("flush")
            .arg(PROCESS_NAME)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        for path in [log_path(false), log_path(true)] {
            let _ = fs::File::create(path);
        }
    }
}

fn pm2_command() -> Command {
    let mut command = crate::core::settings::env_detect::cmd("pm2");
    // 首次拉起 PM2 daemon 时默认会在 jlist JSON 前输出提示；静默模式减少混合输出。
    command
        .env("PM2_SILENT", "true")
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0");
    command
}

/// 从 PM2 的混合 stdout 中提取首个合法 JSON 数组。
///
/// PM2 首次启动守护进程时可能输出 `[PM2] Spawning...`、ANSI 颜色提示，随后才
/// 输出真正的 `[]`/`[{...}]`。逐个尝试 `[` 起点可跳过这些非 JSON 前缀，同时
/// `StreamDeserializer` 允许 JSON 后仍有额外提示。
fn parse_jlist_output(output: &[u8]) -> Result<Vec<Value>, serde_json::Error> {
    if let Ok(values) = serde_json::from_slice::<Vec<Value>>(output) {
        return Ok(values);
    }

    let text = String::from_utf8_lossy(output);
    let mut last_error = serde_json::from_str::<Vec<Value>>(text.trim()).unwrap_err();
    for (start, character) in text.char_indices() {
        if character != '[' {
            continue;
        }
        let mut stream = serde_json::Deserializer::from_str(&text[start..])
            .into_iter::<Vec<Value>>();
        match stream.next() {
            Some(Ok(values)) => return Ok(values),
            Some(Err(error)) => last_error = error,
            None => {}
        }
    }
    Err(last_error)
}

fn run_checked(mut command: Command, context: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{context}：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if detail.is_empty() {
            context.to_owned()
        } else {
            format!("{context}：{detail}")
        })
    }
}

fn log_path(error_log: bool) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let suffix = if error_log { "error" } else { "out" };
    home.join(".pm2")
        .join("logs")
        .join(format!("{PROCESS_NAME}-{suffix}.log"))
}

#[cfg(test)]
mod tests {
    use super::parse_jlist_output;

    #[test]
    fn parses_clean_pm2_jlist() {
        assert!(parse_jlist_output(b"[]").unwrap().is_empty());
        let values = parse_jlist_output(
            br#"[{"name":"astrabrew-launcher-sillytavern","pid":42}]"#,
        )
        .unwrap();
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn skips_pm2_daemon_banner_before_json() {
        let output = b"[PM2] Spawning PM2 daemon\n[PM2] PM2 Successfully daemonized\n[]\n";
        assert!(parse_jlist_output(output).unwrap().is_empty());
    }

    #[test]
    fn accepts_trailing_pm2_messages_after_json() {
        let output = b"[]\n[PM2] Done\n";
        assert!(parse_jlist_output(output).unwrap().is_empty());
    }
}
