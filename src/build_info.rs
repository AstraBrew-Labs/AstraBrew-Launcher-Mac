//! 编译期构建信息。
//!
//! 构建渠道由 `build.rs` 通过 `cargo:rustc-env=ASTRA_BUILD_CHANNEL` 注入，
//! 取值由构建脚本的环境变量决定（`beta` / `release`），缺省为正式版。界面据此在左上角
//! 标注测试版。渠道是编译期常量，运行期不可变；这里用 `OnceLock` 缓存一次解析结果。

use std::sync::OnceLock;

/// 构建渠道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildChannel {
    /// 正式版。
    Release,
    /// 测试版：界面左上角展示「测试版」标记。
    Beta,
}

/// 把注入的渠道取值解析为枚举；无法识别时一律按正式版处理。
pub fn parse_channel(value: &str) -> BuildChannel {
    match value {
        "beta" => BuildChannel::Beta,
        _ => BuildChannel::Release,
    }
}

/// 本次编译注入的构建渠道，进程内只解析一次。
pub fn build_channel() -> BuildChannel {
    static CHANNEL: OnceLock<BuildChannel> = OnceLock::new();
    *CHANNEL.get_or_init(|| parse_channel(env!("ASTRA_BUILD_CHANNEL")))
}

impl BuildChannel {
    /// 是否需要在界面上标注为测试版。
    pub const fn is_beta(self) -> bool {
        matches!(self, Self::Beta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_maps_beta_only() {
        assert_eq!(parse_channel("beta"), BuildChannel::Beta);
        assert_eq!(parse_channel("release"), BuildChannel::Release);
        assert_eq!(parse_channel(""), BuildChannel::Release);
        assert_eq!(parse_channel("nightly"), BuildChannel::Release);
    }

    #[test]
    fn default_build_is_release() {
        // 未显式指定渠道（例如本地 `cargo check`）时必须是正式版，不显示标记。
        assert_eq!(build_channel(), BuildChannel::Release);
        assert!(!build_channel().is_beta());
    }
}
