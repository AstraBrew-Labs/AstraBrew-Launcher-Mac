//! 旧版快速扫描的共享事件：后台 find 查找清单，界面展示近期路径。

use super::{LocalError, LocalInstance};

#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    pub checked: u64,
    pub path: String,
    pub found: u64,
    pub elapsed_seconds: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub progress: ScanProgress,
    pub partial: bool,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Progress(ScanProgress),
    /// 与旧版一致，每发现一个清单就报告其所在目录。
    ScanningPath(String),
    Found(LocalInstance),
    Warning(LocalError),
    Finished(Result<ScanReport, LocalError>),
}
