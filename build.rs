fn main() {
    #[cfg(target_os = "macos")]
    {
        // 将 Info.plist 嵌入二进制，使原生对话框跟随系统语言
        println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,Info.plist");
        println!("cargo:rerun-if-changed=Info.plist");
        // SMAppService（开机自启动）依赖 ServiceManagement framework
        println!("cargo:rustc-link-framework=ServiceManagement");
    }
}
