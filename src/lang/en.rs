//! 英文界面文案。

/// 翻译静态或运行时拼接的界面文案。
pub fn translate_owned(content: &str) -> String {
    let translated = match content {
        "星酿启动器" => "AstraBrew Launcher",
        "AstraBrew Launcher" => "AstraBrew Launcher",
        "主页" => "Home",
        "运行环境" => "Runtime",
        "酒馆配置" => "Tavern Config",
        "版本管理" => "Versions",
        "扩展管理" => "Extensions",
        "资源管理" => "Resources",
        "控制台" => "Console",
        "设置" | "软件设置" => "Settings",
        "当前版本" => "Current Version",
        "版本:" => "Version:",
        "本地实例" => "Local Instance",
        "在线实例" => "Online Instance",
        "未设置" => "Not Set",
        "一键启动" => "Launch",
        "启动" => "Start",
        "停止" => "Stop",
        "重启" => "Restart",
        "强行停止" => "Force Stop",
        "访问酒馆" | "打开酒馆" => "Open Tavern",
        "清空" => "Clear",
        "刷新" | "重新扫描" => "Refresh",
        "取消" => "Cancel",
        "完成" | "确认" => "Done",
        "关闭" => "Close",
        "启用" => "Enable",
        "停用" => "Disable",
        "删除" => "Delete",
        "保存" => "Save",
        "导入" => "Import",
        "导出" => "Export",
        "管理" => "Manage",
        "更改" | "更改位置" => "Change",
        "浏览" | "浏览..." => "Browse",
        "检查更新" => "Check for Updates",
        "检测全部" => "Check All",
        "刷新节点" => "Refresh Nodes",
        "开始测试" => "Start Test",
        "恢复默认" => "Restore Defaults",
        "已恢复默认设置" => "Default settings restored",
        "设置已保存" => "Settings saved",
        "设置未能保存" => "Settings could not be saved",
        "界面设置" => "Interface Settings",
        "语言" => "Language",
        "简体中文" | "中文" => "Simplified Chinese",
        "English" => "English",
        "跟随系统" => "Follow System",
        "代理关闭" => "Off",
        "直连" => "Direct",
        "自定义" => "Custom",
        "华为云" => "Huawei Cloud",
        "腾讯云" => "Tencent Cloud",
        "官方源" => "Official",
        "自定义代理" => "Custom Proxy",
        "主题" => "Theme",
        "浅色" | "明亮主题" => "Light",
        "深色" | "夜晚主题" => "Dark",
        "选择显示语言或跟随系统。" => "Choose a display language or follow the system.",
        "选择浅色、深色或跟随系统外观。" => {
            "Choose light, dark, or follow the system appearance."
        }
        "选择显示语言或跟随系统" => "Choose a display language or follow the system",
        "选择明亮、夜晚或跟随系统模式" => {
            "Choose light, dark, or follow the system mode"
        }
        "调整启动器的显示语言、主题与窗口行为。" => {
            "Choose the launcher language, theme, and window behavior."
        }
        "记住上次窗口位置" => "Remember Last Window Position",
        "启动时恢复上次窗口的位置。" => "Restore the last window position on startup.",
        "启动时恢复上次窗口的位置和大小" => {
            "Restore the last window position on startup"
        }
        "开启" | "已启用" => "On",
        "未启用" => "Off",
        "未检测" => "Not Checked",
        "必装" => "Required",
        "可选" => "Optional",
        "基本设置" => "General",
        "控制台设置" => "Console Settings",
        "环境依赖" => "Dependencies",
        "GitHub 设置" | "Github 设置" => "GitHub Settings",
        "网络设置" => "Network Settings",
        "软件与更新" => "Software & Updates",
        "扫描占用核心数" => "CPU Cores Used for Scanning",
        "一半核心" | "1/2核心" => "Half the Cores",
        "全部核心" | "所有核心" => "All Cores",
        "酒馆启动模式" => "Tavern Launch Mode",
        "正常模式直接使用浏览器；桌面模式使用内置 WebView；服务器模式下固定为服务器模式。" => {
            "Normal mode uses a browser; Desktop mode uses the built-in WebView; Server mode is locked while enabled."
        }
        "正常模式" => "Normal Mode",
        "桌面模式" => "Desktop Mode",
        "服务器模式" => "Server Mode",
        "普通模式" => "Normal Mode",
        "局域网" => "LAN",
        "互联网" => "Internet",
        "自动" => "Automatic",
        "全局数据" | "全局" => "Shared Data",
        "独立数据" | "独立" => "Independent Data",
        "关闭酒馆窗口自动停止服务" => "Stop Service When Tavern Window Closes",
        "允许酒馆后台运行" => "Allow Tavern to Run in Background",
        "启用服务器模式" => "Enable Server Mode",
        "酒馆服务模式" => "Tavern Service Mode",
        "酒馆数据模式" => "Tavern Data Mode",
        "全局数据存放位置" => "Shared Data Location",
        "导出保存目录" => "Export Folder",
        "酒馆资源保存" => "Tavern Resource Folder",
        "桌面模式下酒馆页面导出资源的默认保存位置。" => {
            "Default folder for resources exported from Tavern in Desktop mode."
        }
        "待开发" => "Coming Soon",
        "显示完整的启动命令" => "Show Full Startup Command",
        "NPM 源设置" => "NPM Registry",
        "华为云镜像" => "Huawei Cloud Mirror",
        "源 URL：" => "Registry URL:",
        "GitHub 资源加速" => "GitHub Acceleration",
        "加速节点地址" => "Acceleration Node URL",
        "替换节点列表" => "Acceleration Nodes",
        "代理设置" => "Proxy Settings",
        "自定义代理地址" => "Custom Proxy URL",
        "系统代理：已启用" => "System proxy: enabled",
        "系统代理：未启用" => "System proxy: disabled",
        "系统代理：未知" => "System proxy: unknown",
        "GitHub 连接测试" => "GitHub Connectivity Test",
        "测试模式" => "Test mode",
        "代理地址" => "Proxy address",
        "加速地址" => "Acceleration URL",
        "测试失败" => "Test failed",
        "测试完成" => "Test complete",
        "测试失败，请查看详情后重试。" => {
            "Test failed. Check the details and try again."
        }
        "测试超时，请检查网络或代理设置后重试。" => {
            "Test timed out. Check the network or proxy and try again."
        }
        "正在测试 GitHub 连接…" => "Testing GitHub connectivity...",
        "测试中…" => "Testing...",
        "文件访问" => "Raw file",
        "仓库访问" => "Repository",
        "首页访问" => "Homepage",
        "API 访问" => "API",
        "下载速度" => "Download speed",
        "反向代理" => "Reverse Proxy",
        "互联网服务模式使用的域名、端口与证书功能。" => {
            "Domain, port, and certificate support for Internet service mode."
        }
        "服务未运行" => "Service not running",
        "服务正在启动" => "Service is starting",
        "服务正在运行" => "Service is running",
        "服务正在停止" => "Service is stopping",
        "运行中" | "服务运行中" => "Running",
        "启动中..." | "正在启动..." => "Starting...",
        "停止中..." | "正在停止..." => "Stopping...",
        "日志输出" => "Log Output",
        "日志已清空" => "Log cleared",
        "加载中..." | "正在加载..." => "Loading...",
        "等待输出..." => "Waiting for output...",
        "安装" => "Install",
        "安装中..." | "正在安装…" => "Installing...",
        "等待安装输出…" => "Waiting for install output...",
        "安装超时，请关闭窗口后重试。" => {
            "Installation timed out. Close this window and try again."
        }
        "安装失败，请查看日志后重试。" => {
            "Installation failed. Check the log and try again."
        }
        "安装完成，窗口将在 3 秒后自动关闭。" => {
            "Installation complete. This window will close in 3 seconds."
        }
        "Homebrew 安装" => "Install Homebrew",
        "Git 安装" => "Install Git",
        "Node.js 安装" => "Install Node.js",
        "Caddy 安装" => "Install Caddy",
        "PM2 安装" => "Install PM2",
        "Homebrew 安装仍沿用旧版占位入口。" => {
            "The Homebrew installer remains the legacy placeholder entry."
        }
        "正在运行 brew install git，请稍候…" => {
            "Running brew install git. Please wait..."
        }
        "正在运行 brew install node@24，请稍候…" => {
            "Running brew install node@24. Please wait..."
        }
        "正在运行 brew install caddy，请稍候…" => {
            "Running brew install caddy. Please wait..."
        }
        "正在运行 npm install -g pm2，请稍候…" => {
            "Running npm install -g pm2. Please wait..."
        }
        "更新" => "Update",
        "升级" => "Upgrade",
        "重试" => "Retry",
        "信息获取失败" => "Failed to fetch information",
        "暂无本地实例" | "尚未发现本地实例" => "No local instances found",
        "暂无扩展" | "没有找到扩展" => "No extensions found",
        "已安装扩展" => "Installed Extensions",
        "显示系统扩展" => "Show System Extensions",
        "自动更新" => "Automatic Updates",
        "打开目录" => "Open Folder",
        "前往版本管理" => "Go to Version Management",
        "未选择酒馆实例" => "No tavern instance selected",
        "请先在版本管理中选择或安装一个酒馆实例" => {
            "Please select or install a tavern instance in Version Management"
        }
        "欢迎使用星酿启动器" => "Welcome to AstraBrew Launcher",
        "一键管理你的酒馆服务" => "Manage your Tavern service with one click",
        "准备就绪" => "Ready",
        "已发送启动请求" => "Launch requested",
        "macOS 的软件包管理器（必装）。" => "Package manager for macOS (required).",
        "用于管理酒馆版本与下载酒馆（必装）。" => {
            "Manages and downloads Tavern versions (required)."
        }
        "用于运行酒馆（必装）。" => "Runs Tavern (required).",
        "用于给酒馆添加反向代理（必装）。" => {
            "Adds a reverse proxy for Tavern (required)."
        }
        "用于给酒馆添加反向代理（可选）。" => {
            "Adds a reverse proxy for Tavern (optional)."
        }
        "让酒馆脱离启动器在后台运行（必装，需要 Node.js）。" => {
            "Keeps Tavern running without the launcher (required; needs Node.js)."
        }
        "让酒馆脱离启动器在后台运行（可选，需要 Node.js）。" => {
            "Keeps Tavern running without the launcher (optional; needs Node.js)."
        }
        "检查、安装并管理酒馆运行所需的本机工具。" => {
            "Check, install, and manage the local tools Tavern needs."
        }
        _ => return translate_dynamic(content),
    };
    translated.to_owned()
}

/// 兼容按键查询接口；未收录键保持原文，便于逐步迁移页面文案。
pub fn translate(key: &'static str) -> &'static str {
    match key {
        "星酿启动器" => "AstraBrew Launcher",
        "设置未能保存" => "Settings could not be saved",
        _ => key,
    }
}

fn translate_dynamic(content: &str) -> String {
    if let Some(value) = content.strip_prefix("当前版本：") {
        return format!("Current Version: {value}");
    }
    if let Some(value) = content.strip_prefix("发布于 ") {
        return format!("Published at {value}");
    }
    if let Some(value) = content.strip_prefix("创建于 ") {
        return format!("Created at {value}");
    }
    if let Some(value) = content.strip_suffix(" 项") {
        return format!("{value} items");
    }
    if let Some(name) = content.strip_suffix("  ⚠ 版本过低") {
        return format!("{name}  ⚠ Version too old");
    }
    if let Some(value) = content.strip_prefix("Receiving objects ") {
        return format!("Receiving objects {value}");
    }
    if let Some(value) = content.strip_prefix("Counting objects ") {
        return format!("Counting objects {value}");
    }
    if let Some(value) = content.strip_prefix("Compressing objects ") {
        return format!("Compressing objects {value}");
    }
    if let Some(value) = content.strip_prefix("Resolving deltas ") {
        return format!("Resolving deltas {value}");
    }
    if let Some(value) = content.strip_suffix(" · 总大小未知") {
        return format!("{value} · Total size unknown");
    }
    if let Some(value) = content.strip_prefix("仓库克隆") {
        return format!("Repository clone{value}");
    }
    if content == "管理启动器外观、酒馆运行方式、环境依赖与网络连接" {
        return "Manage appearance, Tavern behavior, dependencies, and network settings".into();
    }
    content.to_owned()
}
