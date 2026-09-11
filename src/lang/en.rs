//! 英文界面文案。

/// 翻译静态或运行时拼接的界面文案。
pub fn translate_owned(content: &str) -> String {
    if let Some(value) = translate_key(content) {
        return value.to_owned();
    }
    if let Some(value) = translate_config(content) {
        return value.to_owned();
    }
    if let Some(value) = translate_local(content) {
        return value.to_owned();
    }
    let translated = match content {
        "星酿启动器" => "AstraBrew Launcher",
        "AstraBrew Launcher" => "AstraBrew Launcher",
        "主页" => "Home",
        "运行环境" => "Runtime",
        "酒馆配置" => "Tavern Config",
        "版本管理" => "Versions",
        "扩展管理" => "Extensions",
        "资源管理" => "Resources",
        "扩展格式" => "Extension Format",
        "标准" => "Standard",
        "依赖酒馆助手" => "Requires Tavern Helper",
        "是" => "Yes",
        "否" => "No",
        "正在加载预设…" => "Loading presets…",
        "正在读取预设元数据，完成后即可查看详情。" => {
            "Reading preset metadata. Details will be available when loading finishes."
        }
        "正在加载预设详情…" => "Loading preset details…",
        "正在读取提示词内容，请稍候。" => "Reading prompt contents. Please wait.",
        "预设详情加载失败" => "Failed to load preset details",
        "上一页" => "Previous",
        "下一页" => "Next",
        "控制台" => "Console",
        "设置" | "软件设置" => "Settings",
        "当前版本" => "Current Version",
        "版本:" => "Version:",
        "本地实例" => "Local Instance",
        "在线实例" => "Online Instance",
        "本地" => "Local",
        "在线" => "Online",
        "环境正常" => "Environment ready",
        "环境不完整" => "Environment incomplete",
        "未检测到" => "Not detected",
        "请选择酒馆版本" => "Select a Tavern version",
        "暂无可切换的酒馆实例" => "No switchable Tavern instances",
        "安装" => "Install",
        "切换到此版本" => "Switch to This Version",
        "镜像已同步" => "Mirror synced",
        "镜像未同步，将使用直连" => "Mirror not synced; direct connection will be used",
        "正在获取在线版本…" => "Fetching online versions…",
        "在线版本列表已更新。" => "Online version list updated.",
        "当前显示的是旧缓存版本。" => "Showing stale cached versions.",
        "在线版本获取失败" => "Unable to fetch online versions",
        "重新获取" => "Retry",
        "酒馆安装" => "Install Tavern",
        "下载完成，3 秒后开始安装…" => {
            "Download complete; installation starts in 3 seconds…"
        }
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
        "下载设置" => "Download Settings",
        "酒馆下载渠道" => "Tavern Download Channel",
        "酒馆下载渠道测速" => "Tavern Download Channel Test",
        "自动测速缓存" => "Automatic Test Cache",
        "测速结果缓存 7 天；缓存有效期内不会重复测速。" => {
            "Test results are cached for 7 days; no repeat test is needed while valid."
        }
        "缓存有效" => "Cache valid",
        "尚未测速" => "Not tested",
        "测速中…" => "Testing…",
        "重新测速" => "Test Again",
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
        "自动" => "Automatic",
        "镜像 1" => "Mirror 1",
        "镜像 2" => "Mirror 2",
        "镜像 3" => "Mirror 3",
        "官方" => "Official",
        "服务器模式" => "Server Mode",
        "普通模式" => "Normal Mode",
        "局域网" => "LAN",
        "互联网" => "Internet",
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
        "选择酒馆核心下载、安装与更新时使用的仓库渠道。" => {
            "Choose the repository channel used to download, install, and update Tavern."
        }
        "镜像 1：" => "Mirror 1: ",
        "镜像 2：" => "Mirror 2: ",
        "镜像 3：" => "Mirror 3: ",
        "官方：" => "Official: ",
        "镜像 1：官方仓库的镜像，国内速度比较快，但版本同步会晚一些。" => {
            "Mirror 1: A mirror of the official repository. Relatively fast in China, but updates may lag."
        }
        "镜像 2：官方仓库的备用镜像，国内速度较快，但版本同步会晚一些。" => {
            "Mirror 2: A backup mirror of the official repository. Faster in China, but updates may lag."
        }
        "镜像 3：官方仓库的备用镜像，国内速度较快，但版本同步会慢很多。" => {
            "Mirror 3: A backup mirror of the official repository. Fast in China, but synchronization is much slower."
        }
        "官方：官方仓库直连，国内速度较慢，但版本更新最快。" => {
            "Official: Direct connection to the official repository. Slower in China, but updates arrive fastest."
        }
        "自动选择速度最快且可用的下载渠道。" => {
            "Automatically choose the fastest available channel."
        }
        "官方仓库的镜像，国内速度较快，但版本同步会晚一些。" => {
            "A mirror of the official repository. Faster in China, but updates may lag."
        }
        "官方仓库的备用镜像，国内速度较快，但版本同步会晚一些。" => {
            "A backup mirror of the official repository. Faster in China, but updates may lag."
        }
        "官方仓库直连，国内速度较慢，但版本更新最快。" => {
            "Direct connection to the official repository. Slower in China, but updates arrive fastest."
        }
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
        "正在测速下载渠道…" => "Testing download channels…",
        "渠道测速超时，请稍后重试。" => {
            "Download channel test timed out. Please try again later."
        }
        "所有渠道测速失败，已回退到官方渠道。" => {
            "All channel tests failed; falling back to the official channel."
        }
        "等待测试…" => "Waiting…",
        "测速成功" => "Test succeeded",
        "测速失败" => "Test failed",
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
    if let Some(value) = translate_key(key) {
        return value;
    }
    if let Some(value) = translate_config(key) {
        return value;
    }
    if let Some(value) = translate_local(key) {
        return value;
    }
    match key {
        "星酿启动器" => "AstraBrew Launcher",
        "设置未能保存" => "Settings could not be saved",
        _ => key,
    }
}

/// 新增界面统一使用稳定键名，避免再以中文原文作为翻译键。
fn translate_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "console.title" => "Service Console",
        "console.status.not_started" => "Not started",
        "console.status.starting" => "Starting",
        "console.status.running" => "Running",
        "console.status.stopping" => "Stopping",
        "console.status.stopped" => "Stopped",
        "console.status.failed" => "Failed",
        "console.start" => "Start",
        "console.stop" => "Stop",
        "console.kill" => "Force stop",
        "console.restart" => "Restart",
        "console.open" => "Open Tavern",
        "console.follow" => "Follow logs",
        "console.export" => "Export logs",
        "console.export.success" => "Logs exported to:",
        "console.export.failed" => "Failed to export logs:",
        "console.logs.empty" => "No log output",
        "console.log.ready" => "Console ready.",
        "console.log.startup_command" => "Startup command",
        "console.webview.ready" => "The desktop window loaded SillyTavern successfully.",
        "console.webview.retrying" => "The desktop window failed to load and will retry",
        "console.webview.failed" => "The desktop window failed to load:",
        "console.webview.process_terminated" => {
            "The WebView content process terminated unexpectedly."
        }
        "console.webview.timeout" => "The desktop window timed out while loading.",
        "console.webview.blank_page" => "WebView only completed a blank-page navigation:",
        "webview.download.saved" => "File saved to:",
        "webview.download.failed" => "File download failed:",
        "webview.download.reveal" => "Show in Finder",
        "webview.download.reveal_failed" => "Could not reveal the file in Finder",
        "notice.refresh_complete" => "Refresh Complete",
        "notice.refresh_warning" => "Refresh Partially Failed",
        "notice.operation_complete" => "Operation Complete",
        "notice.operation_failed" => "Operation Failed",
        "notice.operation_started" => "Operation Started",
        "notice.switch_complete" => "Switch Complete",
        "notice.action_unavailable" => "Action Unavailable",
        "notice.settings_updated" => "Settings Updated",
        "notice.directory_opened" => "Folder Opened",
        "notice.load_failed" => "Load Failed",
        "notice.import_complete" => "Import Complete",
        "notice.import_partial" => "Some Files Failed to Import",
        "notice.delete_complete" => "Delete Complete",
        "notice.delete_failed" => "Delete Failed",
        "resources.confirm.delete.title" => "Delete resource?",
        "resources.confirm.delete.description" => {
            "The following resource will be permanently deleted"
        }
        "resources.confirm.delete.warning" => {
            "This cannot be undone. Confirm that this is the resource you want to delete."
        }
        "resources.confirm.delete.cancel" => "Cancel",
        "resources.confirm.delete.confirm" => "Delete Resource",
        "resources.import.action" => "Import ",
        "resources.import.validating" => "Validating…",
        "resources.import.show_failures" => "View Import Failures",
        "resources.import.failure_title" => "Import Failure Details",
        "resources.import.failure_count" => "files were not imported",
        "resources.import.failure_hint" => {
            "Repair the files and import them again. Forced import is not available."
        }
        "resources.import.clear_failures" => "Clear Records",
        "resources.import.close" => "Close",
        "resources.import.resource_unknown" => "Unknown Resource",
        "resources.import.kind.character" => "Character Cards",
        "resources.import.kind.world_book" => "World Books",
        "resources.import.kind.preset" => "Presets",
        "resources.import.format_unrecognized" => "Unrecognized Format",
        "resources.validation.file_level" => "File",
        "resources.validation.invalid_extension" => "The file extension is not supported",
        "resources.validation.file_too_large" => "The file exceeds the allowed size",
        "resources.validation.read_failed" => "The file could not be read",
        "resources.validation.task_failed" => "The resource import task ended unexpectedly",
        "resources.validation.invalid_encoding" => "The file is not valid UTF-8 text",
        "resources.validation.invalid_png" => "The PNG structure is invalid",
        "resources.validation.invalid_png_text" => {
            "Embedded PNG text could not be decompressed or parsed"
        }
        "resources.validation.missing_metadata" => "No valid character-card metadata was found",
        "resources.validation.invalid_json" => "The JSON content is invalid",
        "resources.validation.invalid_json_root" => "The JSON root must be an object",
        "resources.validation.unrecognized_version" => {
            "The character-card version is not recognized"
        }
        "resources.validation.missing_field" => "A required field is missing",
        "resources.validation.invalid_field_type" => "A field has an invalid type",
        "resources.validation.empty_entries" => "Resource entries cannot be empty",
        "resources.validation.unrecognized_resource" => {
            "The content does not match this resource format"
        }
        "resources.validation.target_conflict" => "A file with the same name already exists",
        "resources.validation.write_failed" => "The target file could not be written",
        "workbench.open" => "Edit in Workbench",
        "workbench.character.title" => "Character Workbench",
        "workbench.world_book.title" => "World Book Workbench",
        "workbench.preset.title" => "Preset Workbench",
        "workbench.loading" => "Loading workbench…",
        "workbench.load_failed" => "Workbench failed to load",
        "workbench.load.retry" => "Reload",
        "workbench.save.saving" => "Saving automatically…",
        "workbench.save.saved" => "Automatically saved",
        "workbench.save.closing" => "Closing after save…",
        "workbench.save.retry" => "Retry save",
        "workbench.save.failed" => "Automatic save failed",
        "workbench.character.basic" => "Character information",
        "workbench.character.cover" => "Character card cover",
        "workbench.character.cover.detail" => "Keep the card text and extension data while replacing the PNG image.",
        "workbench.character.cover.change" => "Change character card cover",
        "workbench.character.cover.loading" => "Reading cover…",
        "workbench.character.tags.placeholder" => "Type a tag to add it",
        "workbench.character.description" => "Description",
        "workbench.character.personality" => "Personality",
        "workbench.character.scenario" => "Scenario",
        "workbench.character.first_message" => "First message",
        "workbench.character.world" => "Bound world book",
        "workbench.world.bind" => "Bind world book",
        "workbench.world.change" => "Change world book",
        "workbench.world.unbind" => "Unbind world book",
        "workbench.world.empty" => "This character card has no bound world book.",
        "workbench.world.select" => "Select a world book",
        "workbench.world.no_options" => "No world books are available in the resource folder.",
        "workbench.world_book.basic" => "World book information",
        "workbench.world.entries" => "World book entries",
        "workbench.world.entry" => "Entry",
        "workbench.preset.prompts" => "Preset prompts",
        "workbench.preset.order_managed" => "Enabled states are managed across prompt_order templates.",
        "workbench.preset.direct_managed" => "Enabled states are stored directly in prompt entries.",
        "workbench.preset.add" => "Add prompt",
        "workbench.preset.adding" => "Adding…",
        "workbench.preset.page" => "Page",
        "workbench.preset.items" => "prompts",
        "workbench.preset.partial" => "Partially enabled",
        "workbench.preset.marker" => "Structure marker",
        "workbench.preset.prompt" => "Prompt entry",
        "workbench.field.name" => "Name",
        "workbench.field.author" => "Author",
        "workbench.field.version" => "Version",
        "workbench.field.tags" => "Tags (comma separated)",
        "workbench.field.comment" => "Comment",
        "workbench.field.keys" => "Primary keys (comma separated)",
        "workbench.field.secondary_keys" => "Secondary keys (comma separated)",
        "workbench.field.content" => "Content",
        "workbench.field.enabled" => "Enabled",
        "workbench.field.disabled" => "Disabled",
        "workbench.field.order" => "Order",
        "workbench.field.position" => "Position",
        "workbench.field.probability" => "Probability",
        "workbench.field.depth" => "Depth",
        "workbench.field.role" => "Role",
        "workbench.confirm.delete.title" => "Delete preset prompt?",
        "workbench.confirm.delete.detail" => "This also removes matching references from every prompt_order template.",
        "workbench.confirm.replace.title" => "Replace the bound world book?",
        "workbench.confirm.replace.detail" => "The embedded world book and its edits will be replaced with a new copy.",
        "workbench.confirm.unbind.title" => "Unbind world book?",
        "workbench.confirm.unbind.detail" => "The embedded world book and its edits will be removed from this character card.",
        "workbench.confirm.cancel" => "Cancel",
        "workbench.confirm.continue" => "Continue",
        "console.webview.closed_stopping" => "The desktop window closed. Stopping SillyTavern.",
        "console.webview.closed_running" => {
            "The desktop window closed. SillyTavern is still running."
        }
        "console.log.starting" => "Preparing the SillyTavern runtime…",
        "console.log.started" => "SillyTavern started.",
        "console.log.stopping" => "Stopping SillyTavern…",
        "console.log.stopped" => "SillyTavern stopped.",
        "console.log.exited" => "Tavern process exited with code:",
        "console.pm2.unavailable" => {
            "PM2 was not found. Falling back to direct mode; closing the launcher will stop the service."
        }
        "console.pm2.restored" => "Restored the PM2-managed SillyTavern service.",
        "console.network.lan" => "LAN access",
        "access.title" => "Access Tavern",
        "access.mode.lan" => "LAN Mode",
        "access.mode.internet" => "Internet Mode",
        "access.loading" => "Detecting network addresses…",
        "access.address_not_ready" => "The Tavern access address is not ready yet.",
        "access.no_address" => "No available address obtained",
        "access.retry" => "Retry",
        "access.ipv4" => "IPv4 Address",
        "access.ipv6" => "IPv6 Address",
        "access.fetch_failed" => "Failed to fetch",
        "access.qr_failed" => "Failed to generate QR code",
        "access.open_browser" => "Open in Browser",
        "access.scan_hint" => "Scan to access Tavern",
        "console.network.internet" => "Internet access",
        "console.network.lan_hint" => "Devices on the same LAN can use the address below.",
        "console.network.internet_warning" => {
            "Internet mode only adjusts the allowlist. Reverse proxy support is not implemented; do not expose an unsecured port directly."
        }
        "console.network.security" => "Connection security",
        "console.port.title" => "Port in use",
        "console.port.description" => "SillyTavern could not listen on port:",
        "console.port.detected" => "Port conflict detected:",
        "console.port.warning_title" => "Stop another process",
        "console.port.warning" => {
            "Continue only if these processes can be stopped. The launcher never terminates an unconfirmed process."
        }
        "console.port.confirm" => "Release port and retry",
        "console.port.cancel" => "Cancel",
        "console.port.releasing" => "Stopping the confirmed port owners and retrying…",
        "console.port.cancelled" => "Port release cancelled. The service remains stopped.",
        "settings.interface.scale.title" => "Text Size",
        "settings.interface.scale.description" => {
            "Scale text, controls, and spacing together to keep the layout aligned."
        }
        "settings.interface.font.title" => "Display Font",
        "settings.interface.font.description" => {
            "Choose an interface font from the visible fonts installed on this Mac."
        }
        "settings.interface.font.placeholder" => "Search system fonts",
        "settings.interface.font.loading" => "Applying font…",
        "settings.interface.font.error_title" => "Could Not Change Font",
        "settings.interface.font.read_error" => {
            "The selected font files could not be read. The current font was kept."
        }
        "settings.interface.font.render_error" => {
            "The selected font could not be registered with the renderer. The current font was kept."
        }
        "settings.interface.font.default" => "Default (HarmonyOS Sans)",
        "environment.nodejs_required.title" => "Node.js Required",
        "environment.nodejs_required.description" => {
            "Local Tavern instances need Node.js and npm to verify dependencies and run."
        }
        "environment.nodejs_required.action_hint" => {
            "Install opens Settings and automatically starts installing Node.js 24."
        }
        "environment.nodejs_required.install" => "Install Node.js",
        "environment.nodejs_required.later" => "Install Later",
        "environment.nodejs_required.error" => "No working Node.js and npm runtime was detected.",
        "environment.install.running" => "Installing",
        "environment.install.success" => "Installation complete",
        "environment.install.failed" => "Installation failed",
        "environment.install.timed_out" => "Installation timed out",
        "environment.install.elapsed" => "Elapsed",
        "environment.install.executing" => "Running the installer",
        "environment.install.ready" => "The runtime is ready",
        "environment.install.not_completed" => "The installation did not complete",
        "environment.install.timeout_description" => "The installation timed out",
        "environment.install.progress_unknown" => "The installer has not reported a percentage.",
        "environment.install.waiting" => "Waiting for installation output…",
        "environment.install.show_details" => "Show details",
        "environment.install.hide_details" => "Hide details",
        "environment.install.cancel" => "Cancel",
        "environment.install.close" => "Close",
        "environment.install.nodejs_keg_ready" => {
            "Node.js is installed in its dedicated runtime directory and does not need a global link."
        }
        "environment.install.pm2.preparing" => "Preparing the global npm environment…",
        "environment.install.pm2.installing" => {
            "Connecting to the npm registry and installing PM2…"
        }
        "environment.install.pm2.verifying" => {
            "The install command completed. Verifying the PM2 version…"
        }
        "extensions.title" => "Extensions",
        "extensions.description" => "Manage extensions installed in the current Tavern instance",
        "extensions.install" => "Install Extension",
        "extensions.open_root" => "Open Extensions Folder",
        "extensions.no_instance" => "No Tavern instance selected",
        "extensions.no_instance_hint" => {
            "Select or install a Tavern instance in Version Management first."
        }
        "extensions.go_versions" => "Go to Version Management",
        "extensions.current_instance" => "Current Tavern Instance",
        "extensions.version" => "Current Version",
        "extensions.source.online" => "Online",
        "extensions.source.local" => "Local",
        "extensions.installed" => "Installed Extensions",
        "extensions.items" => "items",
        "extensions.show_system" => "Show System Extensions",
        "extensions.refresh" => "Rescan Extensions",
        "extensions.loading" => "Scanning extensions…",
        "extensions.load_failed" => "Extension scan failed",
        "extensions.empty" => "No extensions found",
        "extensions.empty_hint" => "Installed extensions will appear here.",
        "extensions.scope.global" => "Global",
        "extensions.system" => "System",
        "extensions.enabled" => "Enabled",
        "extensions.disabled" => "Disabled",
        "extensions.system_enabled" => "Managed by Tavern",
        "extensions.manifest_broken" => "Invalid Manifest",
        "extensions.homepage" => "Visit Homepage",
        "extensions.open_directory" => "Open Folder",
        "extensions.repair_git" => "Repair Git",
        "extensions.delete" => "Delete",
        "extensions.auto_update" => "Auto Update",
        "extensions.on" => "On",
        "extensions.off" => "Off",
        "extensions.install.title" => "Install Extension",
        "extensions.install.description" => {
            "Install a third-party extension from Git or an offline ZIP package."
        }
        "extensions.install.git" => "Git Install",
        "extensions.install.offline" => "Offline Package",
        "extensions.installing" => "Installing…",
        "extensions.install.executing" => "Installing extension files…",
        "extensions.install.failed" => "Installation failed",
        "extensions.install.not_completed" => {
            "The installation did not complete. Check the logs and try again."
        }
        "extensions.install.success_title" => "Installation complete",
        "extensions.install.ready" => "The extension was installed successfully.",
        "extensions.install.elapsed" => "Elapsed",
        "extensions.install.progress_unknown" => "The installer has not reported a percentage.",
        "extensions.install.waiting" => "Waiting for installation output…",
        "extensions.install.show_logs" => "Show logs",
        "extensions.install.hide_logs" => "Hide logs",
        "extensions.install.success" => "Installation completed and the list was rescanned.",
        "extensions.close" => "Close",
        "extensions.cancel" => "Cancel",
        "extensions.git_url" => "Git Repository URL",
        "extensions.git_url.placeholder" => "https://github.com/owner/repository.git",
        "extensions.detect" => "Detect Repository",
        "extensions.detecting" => "Detecting…",
        "extensions.branch" => "Branch",
        "extensions.branch.placeholder" => "Detect the repository first",
        "extensions.choose_zip" => "Choose ZIP Packages",
        "extensions.checking" => "Checking…",
        "extensions.offline.empty" => "No offline package selected.",
        "extensions.remove" => "Remove",
        "extensions.overwrite" => "Overwrite",
        "extensions.confirm.delete.title" => "Delete extension?",
        "extensions.confirm.delete.description" => {
            "This will permanently delete the third-party extension:"
        }
        "extensions.confirm.overwrite.title" => "Overwrite existing extension?",
        "extensions.confirm.overwrite.description" => {
            "An extension with the same name already exists. Continuing will replace its directory."
        }
        "extensions.confirm.repair.title" => "Repair Git metadata?",
        "extensions.confirm.repair.description" => {
            "This initializes Git and configures the homepage as origin for:"
        }
        "extensions.notice.stop_required" => {
            "Stop the current Tavern service before modifying extensions."
        }
        "extensions.log.proxy_fallback" => {
            "The GitHub accelerator failed. Retrying with the original URL."
        }
        "extensions.error.root_missing" => "The selected instance has no extensions directory",
        "extensions.error.scan_failed" => "Failed to read the extensions directory",
        "extensions.error.invalid_repository" => "Invalid Git repository URL",
        "extensions.error.branch_fetch_failed" => "Failed to fetch Git branches",
        "extensions.error.branch_fetch_timeout" => {
            "Git branch detection timed out. Check the network and try again"
        }
        "extensions.error.git_unavailable" => "Git could not be executed",
        "extensions.error.no_branches" => "The repository has no installable branches",
        "extensions.error.select_branch" => "Detect the repository and select a branch first",
        "extensions.error.conflict" => {
            "The extension already exists and requires overwrite confirmation"
        }
        "extensions.error.clone_failed" => "Failed to clone the extension repository",
        "extensions.error.cancelled" => "Operation cancelled",
        "extensions.error.offline_invalid" => "Invalid offline extension package",
        "extensions.error.duplicate_package" => {
            "The selection contains duplicate extension packages"
        }
        "extensions.error.task_disconnected" => "The extension background task ended unexpectedly",
        "extensions.error.offline_open_failed" => "Could not open the offline package",
        "extensions.error.offline_write_failed" => "Failed to write offline extension files",
        "extensions.error.manifest_count" => "The package must contain exactly one manifest.json",
        "extensions.error.manifest_missing" => "The extension has no manifest.json",
        "extensions.error.manifest_invalid" => "The extension manifest is invalid",
        "extensions.error.archive_path" => "The archive contains an unsafe path",
        "extensions.error.archive_symlink" => "The archive contains an unsupported symbolic link",
        "extensions.error.invalid_extension_id" => {
            "Could not determine a safe extension directory name"
        }
        "extensions.error.invalid_target" => "Invalid extension target directory",
        "extensions.error.path_outside_root" => {
            "The extension path is outside the third-party root"
        }
        "extensions.error.create_directory" => "Failed to create the extension directory",
        "extensions.error.cleanup_failed" => "Failed to clean an extension temporary directory",
        "extensions.error.replace_failed" => {
            "Failed to replace the existing extension; restoration was attempted"
        }
        "extensions.error.toggle_failed" => "Failed to change the extension state",
        "extensions.error.delete_failed" => "Failed to delete the extension",
        "extensions.error.git_repair_unsupported" => "The extension has no repairable GitHub URL",
        "extensions.error.git_repair_failed" => "Failed to repair Git metadata",
        "extensions.error.remote_conflict" => {
            "The existing origin differs from the extension homepage"
        }
        "extensions.error.open_failed" => "Failed to open the target",
        _ => return None,
    })
}

fn translate_dynamic(content: &str) -> String {
    for (source, translated) in [
        ("资源目录已重新扫描。", "Resource folders rescanned."),
        (
            "历史对话请通过资源迁移或直接放入角色对应目录。",
            "Move chat history through resource migration or place it in the matching character folder.",
        ),
        (
            "在线版本请求失败，当前使用旧缓存。",
            "The online request failed. Showing the previous cache.",
        ),
        (
            "在线版本列表已更新。",
            "The online version list was updated.",
        ),
        (
            "当前正在使用的版本不能删除。",
            "The active version cannot be deleted.",
        ),
        (
            "已有在线酒馆安装任务正在执行。",
            "An online Tavern installation is already running.",
        ),
        ("已打开系统登录项设置。", "Opened Login Items settings."),
        (
            "已更新酒馆资源保存位置。",
            "Updated the Tavern resource folder.",
        ),
        (
            "已更新全局数据存放位置。",
            "Updated the shared data folder.",
        ),
        (
            "下载渠道测速已刷新。",
            "Download channel testing was refreshed.",
        ),
        (
            "GitHub 连通性测试服务待接入。",
            "GitHub connectivity testing is not connected yet.",
        ),
        (
            "启动器更新检查服务待接入。",
            "Launcher update checking is not connected yet.",
        ),
    ] {
        if content == source {
            return translated.to_owned();
        }
    }
    for (prefix, translated) in [
        ("预设加载失败：", "Failed to load presets: "),
        (
            "无法创建资源目录：",
            "Could not create the resource folder: ",
        ),
        ("无法打开资源目录：", "Could not open the resource folder: "),
        ("删除失败：", "Delete failed: "),
        ("开始切换到 ", "Switching to "),
        ("未找到在线版本 v", "Online version v"),
        ("开始安装在线版本 v", "Installing online version v"),
        ("已切换到在线安装版本 v", "Switched to online version v"),
        ("已删除在线安装版本 v", "Deleted online version v"),
    ] {
        if let Some(value) = content.strip_prefix(prefix) {
            return match prefix {
                "开始切换到 " => format!("{translated}{}.", value.trim_end_matches(" 分支。")),
                "未找到在线版本 v" => format!(
                    "{translated}{} was not found.",
                    value.trim_end_matches('。')
                ),
                "开始安装在线版本 v" => {
                    format!("{translated}{}.", value.trim_end_matches('。'))
                }
                "已切换到在线安装版本 v" => {
                    format!("{translated}{}.", value.trim_end_matches('。'))
                }
                "已删除在线安装版本 v" => {
                    format!("{translated}{}.", value.trim_end_matches('。'))
                }
                _ => format!("{translated}{value}"),
            };
        }
    }
    if let Some(value) = content
        .strip_prefix("已打开 ")
        .and_then(|value| value.strip_suffix(" 目录。"))
    {
        let resource = match value {
            "角色卡" => "character cards",
            "世界书" => "world books",
            "历史对话" => "chat history",
            "预设" => "presets",
            _ => value,
        };
        return format!("Opened the {resource} folder.");
    }
    if let Some(value) = content
        .strip_prefix("已删除“")
        .and_then(|value| value.strip_suffix("”。"))
    {
        return format!("Deleted “{value}”.");
    }
    if let Some(value) = content.strip_prefix("已导入 ")
        && let Some((imported, failed)) = value.trim_end_matches('。').split_once(" 个文件，")
        && let Some(failed) = failed.strip_suffix(" 个失败")
    {
        return format!("Imported {imported} files; {failed} failed.");
    }
    if let Some(value) = content.strip_prefix("已导入 ") {
        let value = value.trim_end_matches('。');
        if let Some((count, resource)) = value.split_once(" 个") {
            let resource = match resource {
                "角色卡" => "character cards",
                "世界书" => "world books",
                "预设" => "presets",
                _ => resource,
            };
            return format!("Imported {count} {resource}.");
        }
        return format!("Imported {value}.");
    }
    if let Some(value) = content
        .strip_prefix("已添加 ")
        .and_then(|value| value.strip_suffix(" 个本地实例"))
    {
        return format!("{value} local instances added");
    }
    if let Some(value) = content
        .strip_prefix("正在执行 ")
        .and_then(|value| value.strip_suffix('…'))
    {
        return format!("Running {value}…");
    }
    for (prefix, translated) in [
        ("无法启动命令：", "Unable to start command: "),
        ("命令退出码：", "Command exit code: "),
        ("等待命令结束失败：", "Unable to wait for command: "),
        ("读取命令日志失败：", "Unable to read command logs: "),
    ] {
        if let Some(value) = content.strip_prefix(prefix) {
            return format!("{translated}{value}");
        }
    }
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
    if content == "正在测速下载渠道…" {
        return "Testing download channels…".into();
    }
    if content == "渠道测速超时，请稍后重试。" {
        return "Download channel test timed out. Please try again later.".into();
    }
    if content == "所有渠道测速失败，已回退到官方渠道。" {
        return "All channel tests failed; falling back to the official channel.".into();
    }
    if content == "等待测试…" {
        return "Waiting…".into();
    }
    if content == "准备克隆" {
        return "Preparing clone".into();
    }
    if content == "进行中" {
        return "In progress".into();
    }
    if let Some(value) = content.strip_prefix("测速成功 · ") {
        return format!("Test succeeded · {value}");
    }
    if content == "测速成功" {
        return "Test succeeded".into();
    }
    if content == "测速失败" {
        return "Test failed".into();
    }
    if let Some(value) = content.strip_prefix("最快渠道：") {
        let channel = match value {
            "自动" => "Automatic",
            "镜像 1" => "Mirror 1",
            "镜像 2" => "Mirror 2",
            "官方" => "Official",
            _ => value,
        };
        return format!("Fastest channel: {channel}");
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

/// 本地实例的静态标题同时供 Toast 与普通文本使用。
fn translate_local(key: &str) -> Option<&'static str> {
    Some(match key {
        "正在检查扫描目录访问权限…" => "Checking access to scan locations…",
        "扫描已完成，但结果不完整。" => "Scan finished with incomplete results.",
        "正在快速扫描用户主目录…" => "Quick-scanning your home directory…",
        "快速扫描完成，仅扫描用户主目录。" => {
            "Quick scan complete. Only your home directory was scanned."
        }
        "快速扫描完成，仅扫描用户主目录；部分位置不可访问。" => {
            "Home-directory quick scan complete. Some locations were inaccessible."
        }
        "扫描范围：已挂载的本地物理磁盘" => "Scope: mounted local physical disks",
        "已检查清单" => "Manifests checked",
        "当前用户主目录" => "Current user's home directory",
        "取消扫描" => "Cancel Scan",
        "扫描正在停止，请稍后重试。" => {
            "The scan is stopping. Please try again shortly."
        }
        "快速扫描输出异常，已停止。" => {
            "Quick scan stopped because its output was invalid."
        }
        "快速扫描命令执行失败。" => "The quick-scan command failed.",
        "无法等待快速扫描进程。" => "Unable to monitor the quick-scan process.",
        "快速扫描跳过或遇到错误。" => {
            "Quick scan skipped a location or encountered an error."
        }
        "无法读取快速扫描输出。" => "Unable to read quick-scan output.",
        "无法启动快速扫描命令。" => "Unable to start the quick-scan command.",
        "无法确定用户主目录，快速扫描未启动。" => {
            "Cannot determine the home directory. Quick scan was not started."
        }
        "用户主目录不可访问，快速扫描无法继续。" => {
            "The home directory is inaccessible. Quick scan cannot continue."
        }
        "实例路径无法保存为文本，已跳过。" => {
            "An instance path cannot be saved as text and was skipped."
        }
        "扫描进度" => "Scan Progress",
        "扫描完成" => "Scan Complete",
        "警告" => "Warning",
        "确定" => "Confirm",
        "确定要取消当前的扫描吗？已经扫描到的实例会保留。" => {
            "Cancel the current scan? Instances already found will be kept."
        }
        "在用户主目录中查找酒馆实例。" => {
            "Looking for SillyTavern instances in your home directory."
        }
        "收起详细日志" => "Hide Details",
        "查看详细日志" => "Show Details",
        "选择酒馆的 package.json" => "Select SillyTavern's package.json",
        "选择的不是酒馆实例，请重新选择。" => {
            "This is not a SillyTavern instance. Please select another file."
        }
        "请不要添加在线实例。" => "Please do not add the online instance.",
        "无法读取实例文件。" => "Unable to read the instance file.",
        "未知版本" => "Unknown version",
        "无法加载本地实例列表。" => "Unable to load local instances.",
        "本地实例列表损坏，已停止覆盖原文件。" => {
            "The instance list is invalid. The original file will not be overwritten."
        }
        "本地实例列表不可写，请修复后重新启动。" => {
            "The instance list cannot be saved. Repair it and restart the app."
        }
        "无法启动本地实例任务。" => "Unable to start the local instance task.",
        "无法读取任务输出。" => "Unable to read task output.",
        "本地实例任务已取消。" => "The local instance task was cancelled.",
        "本地实例任务超时。" => "The local instance task timed out.",
        "任务输出过大，无法确认结果。" => {
            "Task output is too large to verify the result."
        }
        "无法解析运行依赖检测结果。" => {
            "Unable to parse the runtime dependency check."
        }
        "运行依赖检测失败。" => "The runtime dependency check failed.",
        "安装依赖失败，请查看日志后重试。" => {
            "Dependency installation failed. Review the logs and retry."
        }
        "安装已结束，但运行依赖仍不完整。" => {
            "Installation ended, but runtime dependencies are still incomplete."
        }
        "请先安装可用的 Node.js 和 npm。" => {
            "Install a working Node.js and npm runtime first."
        }
        "无法枚举本地物理磁盘。" => "Unable to enumerate local physical disks.",
        "无法解析磁盘信息。" => "Unable to parse disk information.",
        "没有可扫描的已挂载物理磁盘。" => {
            "No mounted physical disks are available to scan."
        }
        "扫描权限不足，请授权后重试。" => {
            "Disk access was denied. Grant access and try again."
        }
        "无法确认完全磁盘访问权限，请授权后重试。" => {
            "Full Disk Access could not be verified. Grant access and try again."
        }
        "无法打开系统权限设置。" => "Unable to open system privacy settings.",
        "无法访问扫描磁盘。" => "Unable to access the disk being scanned.",
        "扫描已取消。" => "Scan cancelled.",
        "扫描遇到权限拒绝，已终止。" => "The scan stopped because access was denied.",
        "扫描路径不可用。" => "A scan location is unavailable.",
        "扫描磁盘已断开，结果不完整。" => {
            "A disk was disconnected. Scan results are incomplete."
        }
        "无法读取扫描目录。" => "Unable to read a scan directory.",
        "扫描结束，但部分路径不可访问，结果不完整。" => {
            "The scan ended with inaccessible locations. Results are incomplete."
        }
        "尚未开始扫描。" => "No scan has been started.",
        "正在检查扫描权限…" => "Checking disk access…",
        "请授予完全磁盘访问权限。" => "Please grant Full Disk Access.",
        "正在扫描本地实例…" => "Scanning for local instances…",
        "扫描完成。" => "Scan complete.",
        "扫描已终止，结果可能不完整。" => "Scan stopped. Results may be incomplete.",
        "扫描授权已取消，未开始扫描。" => {
            "Permission request cancelled. No scan was started."
        }
        "安装本地实例依赖" => "Install Local Dependencies",
        "正在安装并检查运行依赖…" => "Installing and verifying runtime dependencies…",
        "运行依赖已安装完成，请手动切换版本。" => {
            "Runtime dependencies are ready. Switch to this instance when needed."
        }
        "扫描本地实例" => "Scan Local Instances",
        "请在系统设置中允许本应用完全磁盘访问，然后返回重新检测。若仍无效，请重新启动应用后重试。" => {
            "Allow Full Disk Access for this app in System Settings, then return to check again. If access is still denied, restart the app and retry."
        }
        "前往系统设置" => "Open System Settings",
        "已授权，重新检测" => "Access Granted, Check Again",
        "扫描目录" => "Directories",
        "发现实例" => "Found",
        "新增实例" => "Added",
        "重复实例" => "Duplicates",
        "耗时（秒）" => "Elapsed (s)",
        "关闭窗口不影响已开始的后台扫描；日志保留最近 600 行。" => {
            "Closing this dialog keeps the scan running. The latest 600 log lines are retained."
        }
        "已有安装任务正在执行，请稍后再试。" => {
            "An installation is already running. Please wait."
        }
        "保存本地实例列表失败。" => "Unable to save local instances.",
        "已导入本地实例。" => "Local instance imported.",
        "该本地实例已在列表中，已忽略。" => {
            "This instance is already in the list and was skipped."
        }
        "扫描完成，已自动添加发现的本地实例。" => {
            "Scan complete. Discovered local instances were added automatically."
        }
        "已切换到本地实例。" => "Switched to the local instance.",
        "已切换到在线实例。" => "Switched to the online instance.",
        "在线实例尚未就绪，请重新选择版本。" => {
            "The online instance is not ready. Please select a version again."
        }
        "未找到要切换的在线版本，请刷新列表。" => {
            "The requested online version was not found. Please refresh the list."
        }
        "运行依赖不完整，请先安装依赖。" => {
            "Runtime dependencies are incomplete. Install them before switching."
        }
        "当前正在使用的实例不能从列表中移除。" => {
            "The active instance cannot be removed from the list."
        }
        "已从本地实例列表移除。" => "Removed from the local instance list.",
        "正在保存本地实例，请稍后关闭。" => {
            "Saving local instances. Please wait before closing."
        }
        "重试安装" => "Retry Installation",
        "重试检测" => "Retry Check",
        "依赖检测失败，请点击重试查看原因。" => {
            "Dependency check failed. Retry to view the reason."
        }
        "检测中…" => "Checking…",
        "安装中…" => "Installing…",
        "开始扫描" => "Start Scan",
        "正在加载本地实例…" => "Loading local instances…",
        _ => return None,
    })
}

/// 酒馆配置同步、导入和字段校验共用的中英文文案。
fn translate_config(key: &str) -> Option<&'static str> {
    Some(match key {
        "此地址由酒馆服务模式自动管理。" => {
            "This address is managed automatically by the SillyTavern service mode."
        }
        "系统保留的白名单地址不能删除。" => {
            "System-reserved whitelist addresses cannot be removed."
        }
        "由服务模式自动管理" => "Managed by service mode",
        "未支持的值（保留原值）" => "Unsupported value (preserved)",
        "配置后台任务失败，请重试。" => {
            "The configuration background task failed. Please retry."
        }
        "无法同步配置到界面。" => {
            "Unable to synchronize configuration with the interface."
        }
        "下载配置模板失败。" => "Unable to download the configuration template.",
        "导入源文件已变化，请重新确认。" => {
            "The import file has changed. Please confirm again."
        }
        "所有配置模板下载地址均失败。" => {
            "All configuration template download sources failed."
        }
        "无法创建配置模板下载请求。" => {
            "Unable to create the template download request."
        }
        "无法在 Finder 中定位配置文件。" => {
            "Unable to reveal the configuration file in Finder."
        }
        "无法构造 YAML 配置值。" => "Unable to construct the YAML configuration value.",
        "无法读写酒馆配置文件。" => {
            "Unable to read or write the SillyTavern configuration file."
        }
        "未知配置字段。" => "Unknown configuration field.",
        "此配置字段的 YAML 类型不受支持。" => {
            "This configuration field uses an unsupported YAML type."
        }
        "配置 YAML 无效，请修复文件后重试。" => {
            "Invalid configuration YAML. Repair the file and try again."
        }
        "配置字段的父级类型不正确。" => {
            "A configuration field has an invalid parent type."
        }
        "配置必须包含一个 YAML 映射文档。" => {
            "Configuration must contain exactly one YAML mapping document."
        }
        "配置文件已存在，已停止覆盖。" => {
            "The configuration file already exists and was not overwritten."
        }
        "配置文件已被外部修改。" => "The configuration file was modified externally.",
        "配置模板下载内容异常。" => "The downloaded configuration template is invalid.",
        "配置目标已变化，请重新加载。" => {
            "The configuration target changed. Please reload."
        }
        "配置目标路径无效。" => "The configuration target path is invalid.",
        "配置键必须是文本且不能重复。" => {
            "Configuration keys must be unique text values."
        }
        "列表项必须是文本。" => "List entries must be text.",
        "数值超出允许范围。" => "The value is outside the allowed range.",
        "白名单需填写 IP 地址或 CIDR 网段。" => {
            "Enter an IP address or CIDR range in the whitelist."
        }
        "请补全或删除空白列表项。" => "Complete or remove the empty list entry.",
        "请输入有效 IPv4 地址。" => "Enter a valid IPv4 address.",
        "请输入有效 IPv6 地址。" => "Enter a valid IPv6 address.",
        "请输入有效整数。" => "Enter a valid integer.",
        "请选择支持的配置值。" => "Select a supported configuration value.",
        "配置值类型不正确。" => "The configuration value has an invalid type.",
        "当前酒馆配置尚未就绪。" => {
            "The current SillyTavern configuration is not ready."
        }
        "请先选择酒馆实例。" => "Select a SillyTavern instance first.",
        "配置已导入，原文件已备份。" => {
            "Configuration imported. The original file was backed up."
        }
        "配置已生成，已保留模板默认值。" => {
            "Configuration generated with the template defaults preserved."
        }
        "仍有未保存的配置" => "Unsaved Configuration",
        "以下字段同时在界面和文件中修改，尚未覆盖任何一方。" => {
            "These fields were changed both here and in the file. Neither change has been overwritten."
        }
        "保存失败" => "Save Failed",
        "保留我的修改" => "Keep My Changes",
        "原文件不会被覆盖，请修复后重新加载。" => {
            "The original file will not be overwritten. Repair it and reload."
        }
        "存在无效输入、文件冲突或保存失败。继续编辑，或放弃尚未保存的修改并退出？" => {
            "Some inputs are invalid, conflicting, or could not be saved. Continue editing or discard unsaved changes and quit?"
        }
        "导入源文件" => "Import Source",
        "导入配置文件" => "Import Configuration",
        "尚未选择实例" => "No Instance Selected",
        "就绪" => "Ready",
        "已保存" => "Saved",
        "待保存" => "Pending Save",
        "打开配置文件" => "Reveal Configuration",
        "放弃并退出" => "Discard and Quit",
        "是否覆盖已有配置项？导入字段优先，缺失字段保留并由模板补全。" => {
            "Overwrite existing settings? Imported values take priority; missing settings are retained or filled from the template."
        }
        "正在保存…" => "Saving…",
        "正在准备导入…" => "Preparing Import…",
        "正在加载配置…" => "Loading Configuration…",
        "正在处理当前目标，请稍候。不会自动修改网络访问设置。" => {
            "Processing this target. Network access settings will not be changed automatically."
        }
        "正在生成配置…" => "Generating Configuration…",
        "源文件或目标文件已变化，请再次确认。" => {
            "The source or target file changed. Please confirm again."
        }
        "目标配置文件" => "Target Configuration",
        "目标配置文件不存在，请点击立即生成。生成前不会写入页面默认值。" => {
            "The configuration file is missing. Click Generate Now. No interface defaults will be written beforehand."
        }
        "确认后会先备份原配置，列表字段整体替换。" => {
            "The original configuration will be backed up first. Imported lists replace existing lists."
        }
        "确认覆盖并导入" => "Confirm and Import",
        "立即生成" => "Generate Now",
        "继续编辑" => "Continue Editing",
        "请先在版本管理中选择一个酒馆实例。" => {
            "Select a SillyTavern instance in Versions first."
        }
        "请检查输入" => "Check Inputs",
        "配置存在冲突" => "Configuration Conflict",
        "配置文件不存在" => "Configuration File Missing",
        "配置文件无效" => "Invalid Configuration File",
        "配置读取失败" => "Configuration Read Failed",
        "采用文件内容" => "Use File Contents",
        "重新加载" => "Reload",
        _ => return None,
    })
}
