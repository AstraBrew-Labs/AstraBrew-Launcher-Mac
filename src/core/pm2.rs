//! PM2 进程管理封装。
//!
//! 所有 PM2 CLI 调用都由控制台运行时线程执行，避免阻塞 iced 主线程。

use std::fs;
use std::io::{Read, Seek, SeekFrom};
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
        let values: Vec<Value> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("PM2 状态格式无效：{error}"))?;
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

    /// 返回 PM2 日志文件当前长度；恢复已有服务时从末尾开始读取。
    pub fn log_length(&self, error_log: bool) -> u64 {
        fs::metadata(log_path(error_log))
            .map(|metadata| metadata.len())
            .unwrap_or(0)
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
        let mut text = String::new();
        file.read_to_string(&mut text)
            .map_err(|error| format!("无法读取 PM2 日志：{error}"))?;
        *offset = file.stream_position().unwrap_or(length);
        Ok(text.lines().map(str::to_owned).collect())
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
    crate::core::settings::env_detect::cmd("pm2")
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
