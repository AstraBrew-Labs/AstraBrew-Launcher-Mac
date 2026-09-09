//! 启动器核心业务模块。

pub(crate) mod auto_launch;
pub(crate) mod extensions;
pub(crate) mod network;
pub(crate) mod settings;

pub(crate) mod local_instances;

pub(crate) mod tavern_config;
pub(crate) mod typography;

#[cfg(target_os = "macos")]
pub(crate) mod desktop_webview;
pub(crate) mod pm2;
pub(crate) mod tavern_process;
