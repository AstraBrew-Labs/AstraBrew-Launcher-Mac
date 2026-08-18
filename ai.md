## 规范
这是一个 rust + iced + astra_ui 项目
astra_ui文档：https://docs.rs/iced-astraui/0.0.1
不要使用cargo run启动项目，也不要用于测试
只能使用cargo check检查项目
代码要严格模式，warning要修复，error要修复。
仅MacOS平台，不用考虑其他平台。

要确保程序check通过

需要加上代码注释，注释要中文

软件窗口界面，不能最大化
在16：9比例下，默认要1280x720，固定大小，不能手动调节
在4:3比例下，默认1280x800，固定大小，不能手动调节
要适配高清屏，高分辨率屏，保持界面清晰度
要保证每个缩放都能正常显示
要适配多屏切换
要适配如果软件界面在副屏，但副屏断开，要自动切换到主屏显示

 项目结构如下：
  src
   - core 核心模块
    - ...rs 核心模块拆分
   - utils.rs 工具函数
   - main.rs 主函数

## 目录结构

数据目录结构
~/Library/Application Support/AstraBrew Launcher/ 根目录
├── default 默认数据目录
│   ├── sillytavern 全局统一酒馆数据目录
│   │   ├── settings.json 默认酒馆WebUI配置文件
│   ├── config.yaml 默认酒馆配置文件
├── data 用户数据目录
│   ├── sillytavern 全局统一酒馆数据目录，在线酒馆实例
│   │   ├── settings.json 全局统一酒馆WebUI设置
│   ├── config.yaml 全局统一酒馆配置文件
├── sillytavern 酒馆核心文件目录
├── settings.json 配置文件

缓存数据目录结构
~/Library/Caches/AstraBrew Launcher/
├── github_proxy_cache.json GitHub 加速地址缓存文件
... 其他缓存文件

日志目录结构
~/Library/Logs/AstraBrew Launcher/
├── launcher.latest.log 主程序日志（上一次启动的版本）
├── launcher.log 主程序日志（只保留最新的版本，实时更新）
├── sillytavern.latest.log 酒馆日志（上一次启动的版本）
├── sillytavern.log 酒馆日志（只保留最新的版本，实时更新）
... 其他日志文件

临时目录
/tmp/AstraBrew Launcher/ 用于存放临时文件，程序运行结束后可以清理掉

## 多语言支持
要支持中文和英文的国际化适配
src
 - lang
  - zh.rs 中文
  - en.rs 英文
  - lang.rs 多语言模块