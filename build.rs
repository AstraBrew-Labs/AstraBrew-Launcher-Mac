//! 构建期脚本。
//!
//! 做两件事：
//! 1. 把构建渠道注入为编译期环境变量 `ASTRA_BUILD_CHANNEL`，供界面标注测试版；
//! 2. 在 macOS 上把 `Info.plist` 内嵌进二进制（`__TEXT,__info_plist`），
//!    使未打包的裸二进制也能带上应用名 / 标识符，原生对话框跟随系统语言。
//!
//! 注意：本项目的自启动能力通过 `dlopen` 在运行时加载 `ServiceManagement.framework`
//! （见 `core/auto_launch.rs`），因此**不需要**在构建期链接该框架。

fn main() {
    // 渠道由构建脚本通过 `ASTRA_BUILD_CHANNEL` 传入；改动后需要触发重编译。
    println!("cargo:rerun-if-env-changed=ASTRA_BUILD_CHANNEL");
    let channel = std::env::var("ASTRA_BUILD_CHANNEL").unwrap_or_else(|_| "release".to_owned());
    println!("cargo:rustc-env=ASTRA_BUILD_CHANNEL={channel}");

    #[cfg(target_os = "macos")]
    {
        // 将 Info.plist 嵌入二进制，使原生对话框跟随系统语言。
        println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,Info.plist");
        println!("cargo:rerun-if-changed=Info.plist");
    }
}
