//! 工具函数模块 — macOS 路径管理
//!
//! 路径规范（macOS）：
//!
//! ```text
//! ~/Library/Application Support/AstraBrew Launcher/    ← 应用根目录
//! ├── sillytavern/          ← 酒馆核心文件
//! │   ├── config.yaml       ← 酒馆配置文件
//! │   └── default/
//! │       └── config.yaml   ← 酒馆配置模板
//! ├── data/                 ← 数据目录
//! │   └── local_instances.json
//! ├── settings.json         ← 启动器配置文件
//! ├── logs/                 ← 日志目录
//! └── temp/                 ← 临时目录
//! ```
//!
//! 开发模式（cargo run）时，根目录自动切换为项目 `data/` 目录，
//! 所有子目录结构保持一致。

use std::path::PathBuf;
use std::sync::OnceLock;

// ============================================================================
// AppPaths — 全局路径管理器
// ============================================================================

/// 应用所有标准路径的集中管理器
///
/// 生产环境根目录: `~/Library/Application Support/AstraBrew Launcher/`
/// 开发环境根目录: `{项目根目录}/data/`
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// 应用根目录
    pub root: PathBuf,
    /// 数据子目录: `root/data/`
    pub data: PathBuf,
    /// 日志子目录: `root/logs/`
    pub logs: PathBuf,
    /// 临时子目录: `root/temp/`
    pub temp: PathBuf,
}

/// 全局单例
static PATHS: OnceLock<AppPaths> = OnceLock::new();

/// 获取全局 AppPaths 实例
///
/// 首次调用时自动初始化并创建所有必要目录。
/// 后续调用返回同一实例的引用。
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
        let root = if Self::is_dev_mode() {
            Self::dev_root()
        } else {
            Self::prod_root()
        };

        Self {
            data: root.join("data"),
            logs: root.join("logs"),
            temp: root.join("temp"),
            root,
        }
    }

    /// 判断是否开发模式（cargo run）
    fn is_dev_mode() -> bool {
        let exe = std::env::current_exe().unwrap_or_default();
        let s = exe.to_string_lossy();
        s.contains("/target/debug/") || s.contains("/target/release/")
    }

    /// 开发模式根目录 → 项目 `data/`
    ///
    /// 从可执行文件路径推导项目根目录：
    /// `.../project/target/debug/astrabrew-launcher-mac` → `.../project/data/`
    fn dev_root() -> PathBuf {
        let mut exe = std::env::current_exe().unwrap_or_default();
        // pop: executable name
        exe.pop();
        // pop: debug or release
        exe.pop();
        // pop: target
        exe.pop();
        exe.join("data")
    }

    /// 生产模式根目录 → `~/Library/Application Support/AstraBrew Launcher/`
    fn prod_root() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("AstraBrew Launcher")
    }

    /// 创建所有必要的目录
    fn ensure_dirs(&self) {
        for dir in [&self.root, &self.data, &self.logs, &self.temp] {
            let _ = std::fs::create_dir_all(dir);
        }
        // 同时确保 sillytavern 子目录存在
        let st = self.sillytavern_dir();
        let _ = std::fs::create_dir_all(&st);
    }

    // -- 便捷路径方法 --

    /// 酒馆实例根目录: `root/sillytavern/`
    pub fn sillytavern_dir(&self) -> PathBuf {
        self.root.join("sillytavern")
    }

    /// 酒馆配置文件: `root/sillytavern/config.yaml`
    pub fn tavern_config_file(&self) -> PathBuf {
        self.sillytavern_dir().join("config.yaml")
    }

    /// 酒馆配置模板: `root/sillytavern/default/config.yaml`
    pub fn tavern_template_file(&self) -> PathBuf {
        self.sillytavern_dir().join("default").join("config.yaml")
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
