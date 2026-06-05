//! 工具函数模块 — macOS 标准路径管理
//!
//! ```text
//! ~/Library/Application Support/AstraBrew Launcher/    ← 根目录 (root)
//! ├── data/                    ← 用户数据目录
//! │   ├── sillytavern/        ← 全局统一酒馆数据目录
//! │   ├── config.yaml         ← 全局统一酒馆配置文件
//! │   └── local_instances.json
//! ├── sillytavern/            ← 酒馆核心文件目录 (ST installation)
//! └── settings.json           ← 启动器配置文件
//!
//! ~/Library/Logs/AstraBrew Launcher/      ← 日志目录 (logs)
//!
//! ~/Library/Caches/AstraBrew Launcher/    ← 缓存目录 (caches)
//!
//! /tmp/AstraBrew Launcher/                ← 临时目录 (temp)
//! ```
//!
//! 开发调试时设置 `ASTRA_DEV=1`，所有路径切换为项目本地 `data/` 子目录。

use std::path::PathBuf;
use std::sync::OnceLock;

// ============================================================================
// AppPaths — 全局路径管理器
// ============================================================================

/// 应用所有标准 macOS 路径的集中管理器
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// `~/Library/Application Support/AstraBrew Launcher/`
    pub root: PathBuf,
    /// `~/Library/Logs/AstraBrew Launcher/`
    pub logs: PathBuf,
    /// `~/Library/Caches/AstraBrew Launcher/`
    pub caches: PathBuf,
    /// `/tmp/AstraBrew Launcher/`
    pub temp: PathBuf,
    /// `root/data/`
    pub data: PathBuf,
}

/// 全局单例
static PATHS: OnceLock<AppPaths> = OnceLock::new();

/// 获取全局 AppPaths 实例
///
/// 首次调用时自动初始化并创建所有必要目录。
pub fn app_paths() -> &'static AppPaths {
    PATHS.get_or_init(|| {
        let paths = AppPaths::init();
        paths.ensure_dirs();
        paths
    })
}

impl AppPaths {
    // -- 初始化 --

    fn init() -> Self {
        let dev = Self::is_dev_mode();

        if dev {
            let base = Self::dev_root();
            Self {
                root: base.clone(),
                data: base.join("data"),
                logs: base.join("logs"),
                caches: base.join("caches"),
                temp: base.join("temp"),
            }
        } else {
            Self {
                root: Self::prod_root(),
                data: Self::prod_root().join("data"),
                logs: Self::logs_root(),
                caches: Self::cache_root(),
                temp: Self::temp_root(),
            }
        }
    }

    fn is_dev_mode() -> bool {
        std::env::var("ASTRA_DEV").as_deref() == Ok("1")
    }

    /// 开发模式根目录 → 项目 `data/`，所有路径归一到此
    fn dev_root() -> PathBuf {
        let mut exe = std::env::current_exe().unwrap_or_default();
        exe.pop(); // exe name
        exe.pop(); // debug/release
        exe.pop(); // target
        exe.join("data")
    }

    // -- 生产模式 macOS 标准路径 --

    fn prod_root() -> PathBuf {
        Self::home().join("Library").join("Application Support").join("AstraBrew Launcher")
    }

    fn logs_root() -> PathBuf {
        Self::home().join("Library").join("Logs").join("AstraBrew Launcher")
    }

    fn cache_root() -> PathBuf {
        Self::home().join("Library").join("Caches").join("AstraBrew Launcher")
    }

    fn temp_root() -> PathBuf {
        PathBuf::from("/tmp").join("AstraBrew Launcher")
    }

    fn home() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
    }

    /// 创建所有必要的目录
    fn ensure_dirs(&self) {
        for dir in [&self.root, &self.logs, &self.caches, &self.temp, &self.data] {
            let _ = std::fs::create_dir_all(dir);
        }
        // 确保子目录
        for sub in [
            self.sillytavern_dir(),
            self.data.join("sillytavern"),
        ] {
            let _ = std::fs::create_dir_all(&sub);
        }
    }

    // -- 便捷路径方法 --

    /// 酒馆核心文件目录: `root/sillytavern/`
    pub fn sillytavern_dir(&self) -> PathBuf {
        self.root.join("sillytavern")
    }

    /// 内置酒馆配置文件: `root/sillytavern/config.yaml`
    pub fn tavern_config_file(&self) -> PathBuf {
        self.sillytavern_dir().join("config.yaml")
    }

    /// 全局酒馆配置文件: `root/data/config.yaml`
    pub fn global_tavern_config_file(&self) -> PathBuf {
        self.data.join("config.yaml")
    }

    /// 酒馆配置模板: `root/data/sillytavern/config.yaml`
    pub fn tavern_template_file(&self) -> PathBuf {
        self.data.join("sillytavern").join("config.yaml")
    }

    /// 启动器配置文件: `root/settings.json`
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// 本地实例列表: `root/data/local_instances.json`
    pub fn instances_file(&self) -> PathBuf {
        self.data.join("local_instances.json")
    }
}
