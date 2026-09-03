//! 酒馆配置页面：提供面向新手的常用配置，以及与旧版一致的完整高级配置。

use std::fmt;

use crate::lang::text;
use iced::widget::{button, column, container, pick_list, row, scrollable, space, text_input};
use iced::{Alignment, Background, Border, Color, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use crate::theme::{button_style, pick_list_menu_style, pick_list_style, text_input_style};
use astra_ui::{
    BLUE_600, ButtonVariant, CYAN_500, DANGER, INK_MUTED, SUCCESS, WARNING, WHITE, fonts, icons,
    pick_list_handle,
};

const CONTROL_WIDTH: f32 = 270.0;
const LIST_WIDTH: f32 = 330.0;
const PAGE_WIDTH: f32 = 840.0;

macro_rules! enum_text {
    ($ty:ident, $([$variant:ident, $label:literal]),+ $(,)?) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&crate::lang::display_label(match self { $(Self::$variant => $label,)+ }))
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BrowserType {
    #[default]
    System,
    Chrome,
    Firefox,
    Edge,
    Safari,
}
impl BrowserType {
    pub(crate) const ALL: [Self; 5] = [
        Self::System,
        Self::Chrome,
        Self::Firefox,
        Self::Edge,
        Self::Safari,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::System => "系统默认",
            Self::Chrome => "Chrome",
            Self::Firefox => "Firefox",
            Self::Edge => "Edge",
            Self::Safari => "Safari",
        }
    }
}
enum_text!(
    BrowserType,
    [System, "系统默认"],
    [Chrome, "Chrome"],
    [Firefox, "Firefox"],
    [Edge, "Edge"],
    [Safari, "Safari"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ThumbnailFormat {
    #[default]
    Jpeg,
    Png,
    Webp,
}
impl ThumbnailFormat {
    const ALL: [Self; 3] = [Self::Jpeg, Self::Png, Self::Webp];
}
enum_text!(
    ThumbnailFormat,
    [Jpeg, "JPEG（默认）"],
    [Png, "PNG"],
    [Webp, "WebP（推荐）"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LogLevel {
    #[default]
    Debug,
    Info,
    Warn,
    Error,
}
impl LogLevel {
    const ALL: [Self; 4] = [Self::Debug, Self::Info, Self::Warn, Self::Error];
}
enum_text!(
    LogLevel,
    [Debug, "0 - Debug（最详细）"],
    [Info, "1 - Info"],
    [Warn, "2 - Warn"],
    [Error, "3 - Error"]
);

/// 所有布尔配置的稳定标识，供通用开关消息使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolField {
    Listen,
    ProtocolIpv4,
    ProtocolIpv6,
    DnsPreferIpv6,
    BrowserLaunchEnabled,
    BasicAuthMode,
    EnableUserAccounts,
    EnableDiscreetLogin,
    PerUserBasicAuth,
    WhitelistMode,
    HostWhitelistEnabled,
    HostWhitelistScan,
    SslEnabled,
    CorsEnabled,
    CorsCredentials,
    RequestProxyEnabled,
    ChatBackupsEnabled,
    ChatBackupsCheckIntegrity,
    ThumbnailsEnabled,
    LazyLoadCharacters,
    UseDiskCache,
    EnableAccessLog,
    DisableCsrfProtection,
    SecurityOverride,
    AllowKeysExposure,
    SkipContentCheck,
    ExtensionsEnabled,
    ExtensionsAutoUpdate,
    EnableServerPlugins,
    EnableServerPluginsAutoUpdate,
    AutheliaAuth,
    AuthentikAuth,
    CacheBusterEnabled,
    EnableCorsProxy,
    EnableDownloadableTokenizers,
}

/// 所有文本及数字输入项的稳定标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextField {
    Port,
    ListenIpv4,
    ListenIpv6,
    HeartbeatInterval,
    BasicAuthUsername,
    BasicAuthPassword,
    SslCertPath,
    SslKeyPath,
    SslKeyPassphrase,
    CorsMaxAge,
    RequestProxyUrl,
    CommonBackups,
    ChatMaxBackups,
    ChatThrottleInterval,
    ThumbnailQuality,
    BackgroundWidth,
    BackgroundHeight,
    AvatarWidth,
    AvatarHeight,
    PersonaWidth,
    PersonaHeight,
    MemoryCacheCapacity,
    PromptPlaceholder,
    SessionTimeout,
    CacheBusterPattern,
}

/// 可增删列表配置的稳定标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListField {
    Whitelist,
    HostWhitelist,
    ImportDomains,
    CorsOrigins,
    CorsMethods,
    CorsAllowedHeaders,
    CorsExposedHeaders,
    ProxyBypass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TavernAction {
    OpenConfigFile,
    MigrateConfig,
}

/// 酒馆配置页的交互消息。
#[derive(Debug, Clone)]
pub(crate) enum TavernMessage {
    ToggleAdvancedSection(usize),
    Toggle(BoolField, bool),
    Edit(TextField, String),
    EditList(ListField, usize, String),
    AddListItem(ListField),
    RemoveListItem(ListField, usize),
    SelectBrowser(BrowserType),
    SelectThumbnailFormat(ThumbnailFormat),
    SelectLogLevel(LogLevel),
    Action(TavernAction),
    /// 恢复默认入口将在配置持久化服务接入后启用。
    #[allow(dead_code)]
    RestoreDefaults,
}

/// 与旧版 Tavern.vue 字段一一对应的本地配置草稿。
#[derive(Debug, Clone)]
struct TavernConfig {
    port: String,
    listen: bool,
    listen_ipv4: String,
    listen_ipv6: String,
    protocol_ipv4: bool,
    protocol_ipv6: bool,
    basic_auth_mode: bool,
    enable_user_accounts: bool,
    enable_discreet_login: bool,
    per_user_basic_auth: bool,
    basic_auth_username: String,
    basic_auth_password: String,
    whitelist_mode: bool,
    whitelist: Vec<String>,
    cors_enabled: bool,
    cors_origins: Vec<String>,
    cors_methods: Vec<String>,
    cors_allowed_headers: Vec<String>,
    cors_exposed_headers: Vec<String>,
    cors_credentials: bool,
    cors_max_age: String,
    request_proxy_enabled: bool,
    request_proxy_url: String,
    proxy_bypass: Vec<String>,
    common_backups: String,
    chat_backups_enabled: bool,
    chat_backups_check_integrity: bool,
    chat_max_backups: String,
    chat_throttle_interval: String,
    thumbnails_enabled: bool,
    thumbnail_format: ThumbnailFormat,
    thumbnail_quality: String,
    background_width: String,
    background_height: String,
    avatar_width: String,
    avatar_height: String,
    persona_width: String,
    persona_height: String,
    browser_launch_enabled: bool,
    browser_type: BrowserType,
    ssl_enabled: bool,
    ssl_cert_path: String,
    ssl_key_path: String,
    ssl_key_passphrase: String,
    dns_prefer_ipv6: bool,
    heartbeat_interval: String,
    host_whitelist_enabled: bool,
    host_whitelist_scan: bool,
    host_whitelist: Vec<String>,
    import_domains: Vec<String>,
    session_timeout: String,
    disable_csrf_protection: bool,
    security_override: bool,
    allow_keys_exposure: bool,
    skip_content_check: bool,
    enable_access_log: bool,
    min_log_level: LogLevel,
    lazy_load_characters: bool,
    memory_cache_capacity: String,
    use_disk_cache: bool,
    cache_buster_enabled: bool,
    cache_buster_pattern: String,
    authelia_auth: bool,
    authentik_auth: bool,
    extensions_enabled: bool,
    extensions_auto_update: bool,
    enable_server_plugins: bool,
    enable_server_plugins_auto_update: bool,
    enable_cors_proxy: bool,
    prompt_placeholder: String,
    enable_downloadable_tokenizers: bool,
}

impl Default for TavernConfig {
    fn default() -> Self {
        Self {
            port: "8000".into(),
            listen: false,
            listen_ipv4: "0.0.0.0".into(),
            listen_ipv6: "[::]".into(),
            protocol_ipv4: true,
            protocol_ipv6: false,
            basic_auth_mode: false,
            enable_user_accounts: false,
            enable_discreet_login: false,
            per_user_basic_auth: false,
            basic_auth_username: "user".into(),
            basic_auth_password: "password".into(),
            whitelist_mode: true,
            whitelist: vec!["::1".into(), "127.0.0.1".into()],
            cors_enabled: true,
            cors_origins: vec!["null".into()],
            cors_methods: vec!["OPTIONS".into()],
            cors_allowed_headers: Vec::new(),
            cors_exposed_headers: Vec::new(),
            cors_credentials: false,
            cors_max_age: String::new(),
            request_proxy_enabled: false,
            request_proxy_url: String::new(),
            proxy_bypass: Vec::new(),
            common_backups: "50".into(),
            chat_backups_enabled: true,
            chat_backups_check_integrity: true,
            chat_max_backups: "-1".into(),
            chat_throttle_interval: "10000".into(),
            thumbnails_enabled: true,
            thumbnail_format: ThumbnailFormat::Jpeg,
            thumbnail_quality: "95".into(),
            background_width: "160".into(),
            background_height: "90".into(),
            avatar_width: "96".into(),
            avatar_height: "144".into(),
            persona_width: "96".into(),
            persona_height: "144".into(),
            browser_launch_enabled: true,
            browser_type: BrowserType::System,
            ssl_enabled: false,
            ssl_cert_path: "./certs/cert.pem".into(),
            ssl_key_path: "./certs/privkey.pem".into(),
            ssl_key_passphrase: String::new(),
            dns_prefer_ipv6: false,
            heartbeat_interval: "0".into(),
            host_whitelist_enabled: false,
            host_whitelist_scan: true,
            host_whitelist: Vec::new(),
            import_domains: Vec::new(),
            session_timeout: "-1".into(),
            disable_csrf_protection: false,
            security_override: false,
            allow_keys_exposure: false,
            skip_content_check: false,
            enable_access_log: true,
            min_log_level: LogLevel::Debug,
            lazy_load_characters: false,
            memory_cache_capacity: "100".into(),
            use_disk_cache: true,
            cache_buster_enabled: false,
            cache_buster_pattern: String::new(),
            authelia_auth: false,
            authentik_auth: false,
            extensions_enabled: true,
            extensions_auto_update: true,
            enable_server_plugins: false,
            enable_server_plugins_auto_update: true,
            enable_cors_proxy: false,
            prompt_placeholder: "[Start a new chat]".into(),
            enable_downloadable_tokenizers: true,
        }
    }
}

/// 酒馆配置页状态。服务层接入后可直接用 `config` 做序列化映射。
#[derive(Debug, Clone)]
pub(crate) struct TavernState {
    config: TavernConfig,
    advanced_expanded: [bool; 9],
    last_action: Option<TavernAction>,
}

impl Default for TavernState {
    fn default() -> Self {
        Self {
            config: TavernConfig::default(),
            // 与旧版一致：首次进入页面只展开网络基础配置。
            advanced_expanded: [true, false, false, false, false, false, false, false, false],
            last_action: None,
        }
    }
}

impl TavernState {
    pub(crate) fn browser_type(&self) -> BrowserType {
        self.config.browser_type
    }

    pub(crate) fn update(&mut self, message: TavernMessage) {
        match message {
            TavernMessage::ToggleAdvancedSection(index) => {
                if let Some(expanded) = self.advanced_expanded.get_mut(index) {
                    *expanded = !*expanded;
                }
            }
            TavernMessage::Toggle(field, value) => self.set_bool(field, value),
            TavernMessage::Edit(field, value) => self.set_text(field, value),
            TavernMessage::EditList(field, index, value) => {
                if let Some(item) = self.list_mut(field).get_mut(index) {
                    *item = value;
                }
            }
            TavernMessage::AddListItem(field) => self.list_mut(field).push(String::new()),
            TavernMessage::RemoveListItem(field, index) => {
                let list = self.list_mut(field);
                if index < list.len() {
                    list.remove(index);
                }
            }
            TavernMessage::SelectBrowser(value) => self.config.browser_type = value,
            TavernMessage::SelectThumbnailFormat(value) => self.config.thumbnail_format = value,
            TavernMessage::SelectLogLevel(value) => self.config.min_log_level = value,
            TavernMessage::Action(action) => self.last_action = Some(action),
            TavernMessage::RestoreDefaults => *self = Self::default(),
        }
    }

    fn set_bool(&mut self, field: BoolField, value: bool) {
        let config = &mut self.config;
        match field {
            BoolField::Listen => config.listen = value,
            BoolField::ProtocolIpv4 => config.protocol_ipv4 = value,
            BoolField::ProtocolIpv6 => config.protocol_ipv6 = value,
            BoolField::DnsPreferIpv6 => config.dns_prefer_ipv6 = value,
            BoolField::BrowserLaunchEnabled => config.browser_launch_enabled = value,
            BoolField::BasicAuthMode => config.basic_auth_mode = value,
            BoolField::EnableUserAccounts => config.enable_user_accounts = value,
            BoolField::EnableDiscreetLogin => config.enable_discreet_login = value,
            BoolField::PerUserBasicAuth => config.per_user_basic_auth = value,
            BoolField::WhitelistMode => config.whitelist_mode = value,
            BoolField::HostWhitelistEnabled => config.host_whitelist_enabled = value,
            BoolField::HostWhitelistScan => config.host_whitelist_scan = value,
            BoolField::SslEnabled => config.ssl_enabled = value,
            BoolField::CorsEnabled => config.cors_enabled = value,
            BoolField::CorsCredentials => config.cors_credentials = value,
            BoolField::RequestProxyEnabled => config.request_proxy_enabled = value,
            BoolField::ChatBackupsEnabled => config.chat_backups_enabled = value,
            BoolField::ChatBackupsCheckIntegrity => config.chat_backups_check_integrity = value,
            BoolField::ThumbnailsEnabled => config.thumbnails_enabled = value,
            BoolField::LazyLoadCharacters => config.lazy_load_characters = value,
            BoolField::UseDiskCache => config.use_disk_cache = value,
            BoolField::EnableAccessLog => config.enable_access_log = value,
            BoolField::DisableCsrfProtection => config.disable_csrf_protection = value,
            BoolField::SecurityOverride => config.security_override = value,
            BoolField::AllowKeysExposure => config.allow_keys_exposure = value,
            BoolField::SkipContentCheck => config.skip_content_check = value,
            BoolField::ExtensionsEnabled => config.extensions_enabled = value,
            BoolField::ExtensionsAutoUpdate => config.extensions_auto_update = value,
            BoolField::EnableServerPlugins => config.enable_server_plugins = value,
            BoolField::EnableServerPluginsAutoUpdate => {
                config.enable_server_plugins_auto_update = value
            }
            BoolField::AutheliaAuth => config.authelia_auth = value,
            BoolField::AuthentikAuth => config.authentik_auth = value,
            BoolField::CacheBusterEnabled => config.cache_buster_enabled = value,
            BoolField::EnableCorsProxy => config.enable_cors_proxy = value,
            BoolField::EnableDownloadableTokenizers => {
                config.enable_downloadable_tokenizers = value
            }
        }
    }

    fn set_text(&mut self, field: TextField, value: String) {
        let config = &mut self.config;
        *match field {
            TextField::Port => &mut config.port,
            TextField::ListenIpv4 => &mut config.listen_ipv4,
            TextField::ListenIpv6 => &mut config.listen_ipv6,
            TextField::HeartbeatInterval => &mut config.heartbeat_interval,
            TextField::BasicAuthUsername => &mut config.basic_auth_username,
            TextField::BasicAuthPassword => &mut config.basic_auth_password,
            TextField::SslCertPath => &mut config.ssl_cert_path,
            TextField::SslKeyPath => &mut config.ssl_key_path,
            TextField::SslKeyPassphrase => &mut config.ssl_key_passphrase,
            TextField::CorsMaxAge => &mut config.cors_max_age,
            TextField::RequestProxyUrl => &mut config.request_proxy_url,
            TextField::CommonBackups => &mut config.common_backups,
            TextField::ChatMaxBackups => &mut config.chat_max_backups,
            TextField::ChatThrottleInterval => &mut config.chat_throttle_interval,
            TextField::ThumbnailQuality => &mut config.thumbnail_quality,
            TextField::BackgroundWidth => &mut config.background_width,
            TextField::BackgroundHeight => &mut config.background_height,
            TextField::AvatarWidth => &mut config.avatar_width,
            TextField::AvatarHeight => &mut config.avatar_height,
            TextField::PersonaWidth => &mut config.persona_width,
            TextField::PersonaHeight => &mut config.persona_height,
            TextField::MemoryCacheCapacity => &mut config.memory_cache_capacity,
            TextField::PromptPlaceholder => &mut config.prompt_placeholder,
            TextField::SessionTimeout => &mut config.session_timeout,
            TextField::CacheBusterPattern => &mut config.cache_buster_pattern,
        } = value;
    }

    fn list_mut(&mut self, field: ListField) -> &mut Vec<String> {
        match field {
            ListField::Whitelist => &mut self.config.whitelist,
            ListField::HostWhitelist => &mut self.config.host_whitelist,
            ListField::ImportDomains => &mut self.config.import_domains,
            ListField::CorsOrigins => &mut self.config.cors_origins,
            ListField::CorsMethods => &mut self.config.cors_methods,
            ListField::CorsAllowedHeaders => &mut self.config.cors_allowed_headers,
            ListField::CorsExposedHeaders => &mut self.config.cors_exposed_headers,
            ListField::ProxyBypass => &mut self.config.proxy_bypass,
        }
    }
}

/// 渲染酒馆配置页，结构与旧版 Tavern.vue 保持一致。
pub(crate) fn tavern_view(state: &TavernState) -> Element<'_, TavernMessage> {
    let header = row![
        container(icons::icon(Icon::List, 21, WHITE))
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(page_icon_style),
        column![
            text("酒馆配置").size(19).font(fonts::MEDIUM),
            row![
                crate::theme::muted_icon(Icon::Settings, 10),
                text("管理当前版本的 config.yaml 选项")
                    .size(10)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style)
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        .spacing(4),
        space::horizontal(),
        status_badge(state.last_action),
        header_button(
            "打开配置文件",
            Icon::FolderOpen,
            TavernAction::OpenConfigFile
        ),
        header_button("配置迁移", Icon::ArrowDownUp, TavernAction::MigrateConfig),
    ]
    .spacing(11)
    .align_y(Alignment::Center)
    .width(Fill);

    let header = column![header, crate::theme::separator()]
        .spacing(20)
        .width(Fill);

    let groups = config_groups(state);
    let groups = container(groups)
        .width(Fill)
        .max_width(PAGE_WIDTH)
        .padding([4, 0]);
    let scroller = scrollable(container(groups).width(Fill).align_x(Alignment::Center))
        .width(Fill)
        .height(Fill);

    let content = column![
        container(header)
            .width(Fill)
            .max_width(PAGE_WIDTH)
            .align_x(Alignment::Center),
        scroller,
    ]
    .spacing(18)
    .width(Fill)
    .height(Fill);

    container(content)
        .width(Fill)
        .height(Fill)
        .padding([26, 32])
        .align_x(Alignment::Center)
        .style(crate::theme::canvas_style)
        .into()
}

fn config_groups(state: &TavernState) -> Element<'_, TavernMessage> {
    let config = &state.config;
    column![
        network_section(config, state.advanced_expanded[0]),
        security_section(config, state.advanced_expanded[1]),
        ssl_section(config, state.advanced_expanded[2]),
        cors_section(config, state.advanced_expanded[3]),
        proxy_backup_section(config, state.advanced_expanded[4]),
        thumbnail_section(config, state.advanced_expanded[5]),
        performance_section(config, state.advanced_expanded[6]),
        logging_section(config, state.advanced_expanded[7]),
        session_security_section(config, state.advanced_expanded[8]),
    ]
    .spacing(14)
    .width(Fill)
    .into()
}

fn network_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let port = row![
        container(stacked_field(
            "服务端口",
            wide_text_control("8000", &config.port, TextField::Port, false),
            None,
        ))
        .width(Length::FillPortion(1)),
        space::horizontal().width(Length::FillPortion(1)),
    ]
    .spacing(20)
    .width(Fill);

    let listen_addresses = row![
        container(stacked_field(
            "IPV4 监听地址",
            wide_text_control("0.0.0.0", &config.listen_ipv4, TextField::ListenIpv4, false,),
            None,
        ))
        .width(Length::FillPortion(1)),
        container(stacked_field(
            "IPV6 监听地址",
            wide_text_control("[::]", &config.listen_ipv6, TextField::ListenIpv6, false,),
            None,
        ))
        .width(Length::FillPortion(1)),
    ]
    .spacing(20)
    .width(Fill);

    let protocol_options = row![
        pill_toggle("允许局域网访问", BoolField::Listen, config.listen),
        pill_toggle("启用 IPv4", BoolField::ProtocolIpv4, config.protocol_ipv4),
        pill_toggle("启用 IPv6", BoolField::ProtocolIpv6, config.protocol_ipv6),
        pill_toggle(
            "DNS IPv6 优先",
            BoolField::DnsPreferIpv6,
            config.dns_prefer_ipv6,
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let launch_options = row![
        container(stacked_field(
            "心跳间隔（秒）",
            wide_text_control(
                "0",
                &config.heartbeat_interval,
                TextField::HeartbeatInterval,
                false,
            ),
            Some("服务器心跳检测间隔"),
        ))
        .width(Length::FillPortion(1)),
        container(stacked_field(
            "浏览器类型",
            wide_select_control(
                &BrowserType::ALL,
                config.browser_type,
                TavernMessage::SelectBrowser,
            ),
            None,
        ))
        .width(Length::FillPortion(1)),
    ]
    .spacing(20)
    .width(Fill);

    let content = column![
        port,
        crate::theme::separator(),
        listen_addresses,
        protocol_options,
        crate::theme::separator(),
        launch_options,
        pill_toggle(
            "自动启动浏览器",
            BoolField::BrowserLaunchEnabled,
            config.browser_launch_enabled,
        ),
    ]
    .spacing(18)
    .padding(20)
    .width(Fill);

    advanced_group(
        0,
        expanded,
        Icon::Globe,
        BLUE_600,
        "网络与访问",
        "端口、监听地址与基础协议",
        content.into(),
    )
}

fn security_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let mut rows = vec![
        field_row(
            "启用基础认证",
            "访问酒馆前要求输入全局用户名和密码。",
            toggle_control(BoolField::BasicAuthMode, config.basic_auth_mode),
        ),
        field_row(
            "启用用户账户",
            "允许使用独立的酒馆用户账户。",
            toggle_control(BoolField::EnableUserAccounts, config.enable_user_accounts),
        ),
    ];
    if config.basic_auth_mode {
        rows.extend([
            field_row(
                "全局用户名",
                "Basic Auth 使用的用户名。",
                text_control(
                    "user",
                    &config.basic_auth_username,
                    TextField::BasicAuthUsername,
                    false,
                ),
            ),
            field_row(
                "全局密码",
                "Basic Auth 使用的密码。",
                text_control(
                    "password",
                    &config.basic_auth_password,
                    TextField::BasicAuthPassword,
                    true,
                ),
            ),
        ]);
    }
    rows.extend([
        field_row(
            "低调登录模式",
            "隐藏显式登录入口。",
            toggle_control(BoolField::EnableDiscreetLogin, config.enable_discreet_login),
        ),
        field_row(
            "按用户基础认证",
            "每个用户使用独立的基础认证。",
            toggle_control(BoolField::PerUserBasicAuth, config.per_user_basic_auth),
        ),
        field_row(
            "启用 IP 白名单",
            "仅允许白名单中的 IP 地址访问。",
            toggle_control(BoolField::WhitelistMode, config.whitelist_mode),
        ),
    ]);
    if config.whitelist_mode {
        rows.push(field_row(
            "白名单 IP 列表",
            "支持 IPv4 与 IPv6 地址。",
            list_control(
                &config.whitelist,
                ListField::Whitelist,
                "例如：192.168.1.100",
            ),
        ));
    }
    rows.extend([
        field_row(
            "启用主机白名单",
            "限制 Host 请求头中的主机名。",
            toggle_control(
                BoolField::HostWhitelistEnabled,
                config.host_whitelist_enabled,
            ),
        ),
        field_row(
            "扫描主机",
            "自动扫描并识别可用主机名。",
            toggle_control(BoolField::HostWhitelistScan, config.host_whitelist_scan),
        ),
    ]);
    if config.host_whitelist_enabled {
        rows.push(field_row(
            "主机白名单",
            "允许访问酒馆的主机名。",
            list_control(
                &config.host_whitelist,
                ListField::HostWhitelist,
                "例如：localhost",
            ),
        ));
    }
    rows.push(field_row(
        "导入域名白名单",
        "允许从指定域名导入内容。",
        list_control(
            &config.import_domains,
            ListField::ImportDomains,
            "例如：example.com",
        ),
    ));

    advanced_group(
        1,
        expanded,
        Icon::ShieldCheck,
        Color::from_rgb8(124, 58, 237),
        "安全与账户",
        "账户系统、Basic Auth、IP 与主机白名单。",
        section_rows(rows),
    )
}

fn ssl_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let mut rows = vec![field_row(
        "启用 HTTPS",
        "使用 SSL/TLS 证书加密连接。",
        toggle_control(BoolField::SslEnabled, config.ssl_enabled),
    )];
    if config.ssl_enabled {
        rows.extend([
            field_row(
                "证书文件路径",
                "支持 .pem、.crt 与 .cer 证书文件。",
                text_control(
                    "./certs/cert.pem",
                    &config.ssl_cert_path,
                    TextField::SslCertPath,
                    false,
                ),
            ),
            field_row(
                "私钥文件路径",
                "支持 .pem 与 .key 私钥文件。",
                text_control(
                    "./certs/privkey.pem",
                    &config.ssl_key_path,
                    TextField::SslKeyPath,
                    false,
                ),
            ),
            field_row(
                "私钥密码短语",
                "仅在私钥有密码保护时填写。",
                text_control(
                    "密码短语",
                    &config.ssl_key_passphrase,
                    TextField::SslKeyPassphrase,
                    true,
                ),
            ),
        ]);
    }
    advanced_group(
        2,
        expanded,
        Icon::LockKeyhole,
        SUCCESS,
        "HTTPS / SSL",
        "配置证书以支持加密访问。",
        section_rows(rows),
    )
}

fn cors_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let mut rows = vec![field_row(
        "启用 CORS",
        "控制 API 的跨域访问权限。",
        toggle_control(BoolField::CorsEnabled, config.cors_enabled),
    )];
    if config.cors_enabled {
        rows.extend([
            field_row(
                "允许来源 Origin",
                "允许发起跨域请求的来源。",
                list_control(
                    &config.cors_origins,
                    ListField::CorsOrigins,
                    "例如：https://example.com",
                ),
            ),
            field_row(
                "允许方法 Methods",
                "允许的 HTTP 方法。",
                list_control(
                    &config.cors_methods,
                    ListField::CorsMethods,
                    "例如：OPTIONS",
                ),
            ),
            field_row(
                "预检缓存时间",
                "CORS 预检结果缓存时间，单位为秒。",
                text_control("留空", &config.cors_max_age, TextField::CorsMaxAge, false),
            ),
            field_row(
                "允许请求头",
                "跨域请求可携带的请求头。",
                list_control(
                    &config.cors_allowed_headers,
                    ListField::CorsAllowedHeaders,
                    "例如：Content-Type",
                ),
            ),
            field_row(
                "暴露响应头",
                "允许浏览器读取的响应头。",
                list_control(
                    &config.cors_exposed_headers,
                    ListField::CorsExposedHeaders,
                    "例如：X-Trace-Id",
                ),
            ),
            field_row(
                "允许携带凭证",
                "允许跨域请求携带 Cookie 等凭证。",
                toggle_control(BoolField::CorsCredentials, config.cors_credentials),
            ),
        ]);
    }
    advanced_group(
        3,
        expanded,
        Icon::PlugZap,
        CYAN_500,
        "跨域资源共享（CORS）",
        "控制 API 的跨域来源、方法、请求头与凭证。",
        section_rows(rows),
    )
}

fn proxy_backup_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let mut rows = vec![field_row(
        "启用请求代理",
        "为酒馆发出的外部请求配置代理。",
        toggle_control(BoolField::RequestProxyEnabled, config.request_proxy_enabled),
    )];
    if config.request_proxy_enabled {
        rows.extend([
            field_row(
                "代理地址",
                "外部请求使用的 HTTP 代理。",
                text_control(
                    "http://proxy.example.com:8080",
                    &config.request_proxy_url,
                    TextField::RequestProxyUrl,
                    false,
                ),
            ),
            field_row(
                "代理绕过列表",
                "不经过代理的主机或地址。",
                list_control(
                    &config.proxy_bypass,
                    ListField::ProxyBypass,
                    "例如：localhost",
                ),
            ),
        ]);
    }
    rows.extend([
        field_row(
            "通用备份数量",
            "保留的通用配置备份数量。",
            text_control(
                "50",
                &config.common_backups,
                TextField::CommonBackups,
                false,
            ),
        ),
        field_row(
            "启用聊天备份",
            "自动为聊天记录建立备份。",
            toggle_control(BoolField::ChatBackupsEnabled, config.chat_backups_enabled),
        ),
    ]);
    if config.chat_backups_enabled {
        rows.extend([
            field_row(
                "检查聊天备份完整性",
                "保存备份时检查聊天数据是否完整。",
                toggle_control(
                    BoolField::ChatBackupsCheckIntegrity,
                    config.chat_backups_check_integrity,
                ),
            ),
            field_row(
                "最大聊天备份数",
                "-1 表示不限制总数。",
                text_control(
                    "-1",
                    &config.chat_max_backups,
                    TextField::ChatMaxBackups,
                    false,
                ),
            ),
            field_row(
                "备份节流间隔",
                "两次聊天备份之间的最短间隔，单位为毫秒。",
                text_control(
                    "10000",
                    &config.chat_throttle_interval,
                    TextField::ChatThrottleInterval,
                    false,
                ),
            ),
        ]);
    }
    advanced_group(
        4,
        expanded,
        Icon::DatabaseBackup,
        Color::from_rgb8(234, 88, 12),
        "代理与备份",
        "外部请求代理与自动备份策略。",
        section_rows(rows),
    )
}

fn thumbnail_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    let mut rows = vec![field_row(
        "启用自动缩略图",
        "为角色卡、背景与人设图生成缩略图。",
        toggle_control(BoolField::ThumbnailsEnabled, config.thumbnails_enabled),
    )];
    if config.thumbnails_enabled {
        rows.extend([
            field_row(
                "图像格式",
                "选择生成缩略图时使用的文件格式。",
                select_control(
                    &ThumbnailFormat::ALL,
                    config.thumbnail_format,
                    TavernMessage::SelectThumbnailFormat,
                ),
            ),
            field_row(
                "压缩质量",
                "取值范围为 1 到 100。",
                text_control(
                    "95",
                    &config.thumbnail_quality,
                    TextField::ThumbnailQuality,
                    false,
                ),
            ),
            dimension_row(
                "背景尺寸",
                &config.background_width,
                &config.background_height,
                TextField::BackgroundWidth,
                TextField::BackgroundHeight,
            ),
            dimension_row(
                "头像尺寸",
                &config.avatar_width,
                &config.avatar_height,
                TextField::AvatarWidth,
                TextField::AvatarHeight,
            ),
            dimension_row(
                "人设尺寸",
                &config.persona_width,
                &config.persona_height,
                TextField::PersonaWidth,
                TextField::PersonaHeight,
            ),
        ]);
    }
    advanced_group(
        5,
        expanded,
        Icon::Image,
        Color::from_rgb8(219, 39, 119),
        "缩略图优化",
        "角色卡与背景图的压缩格式、质量和尺寸。",
        section_rows(rows),
    )
}

fn performance_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    advanced_group(
        6,
        expanded,
        Icon::Cpu,
        WARNING,
        "性能优化",
        "缓存管理与角色卡加载策略。",
        section_rows(vec![
            field_row(
                "延迟加载角色卡",
                "按需加载角色卡，加快首次打开速度。",
                toggle_control(BoolField::LazyLoadCharacters, config.lazy_load_characters),
            ),
            field_row(
                "使用磁盘缓存",
                "使用磁盘空间降低运行时内存占用。",
                toggle_control(BoolField::UseDiskCache, config.use_disk_cache),
            ),
            field_row(
                "内存缓存容量",
                "缓存容量上限，单位为 MB。",
                text_control(
                    "100",
                    &config.memory_cache_capacity,
                    TextField::MemoryCacheCapacity,
                    false,
                ),
            ),
        ]),
    )
}

fn logging_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    advanced_group(
        7,
        expanded,
        Icon::Activity,
        INK_MUTED,
        "日志与调试",
        "运行日志与访问监控。",
        section_rows(vec![
            field_row(
                "启用访问日志",
                "记录对酒馆服务的访问请求。",
                toggle_control(BoolField::EnableAccessLog, config.enable_access_log),
            ),
            field_row(
                "最低日志等级",
                "只输出该等级及以上的日志。",
                select_control(
                    &LogLevel::ALL,
                    config.min_log_level,
                    TavernMessage::SelectLogLevel,
                ),
            ),
        ]),
    )
}

fn session_security_section(config: &TavernConfig, expanded: bool) -> Element<'_, TavernMessage> {
    advanced_group(
        8,
        expanded,
        Icon::ListChecks,
        DANGER,
        "会话与安全",
        "会话、安全选项、扩展、插件、SSO 与缓存清除。",
        section_rows(vec![
            field_row(
                "提示词占位符",
                "新对话输入框显示的默认文本。",
                text_control(
                    "[Start a new chat]",
                    &config.prompt_placeholder,
                    TextField::PromptPlaceholder,
                    false,
                ),
            ),
            field_row(
                "会话超时时间",
                "会话有效期，单位为秒；-1 表示永不过期。",
                text_control(
                    "-1",
                    &config.session_timeout,
                    TextField::SessionTimeout,
                    false,
                ),
            ),
            field_row(
                "禁用 CSRF 保护",
                "关闭跨站请求伪造保护，不推荐启用。",
                toggle_control(
                    BoolField::DisableCsrfProtection,
                    config.disable_csrf_protection,
                ),
            ),
            field_row(
                "安全覆盖",
                "覆盖部分内置安全检查。",
                toggle_control(BoolField::SecurityOverride, config.security_override),
            ),
            field_row(
                "允许密钥暴露",
                "允许界面读取密钥，不推荐启用。",
                toggle_control(BoolField::AllowKeysExposure, config.allow_keys_exposure),
            ),
            field_row(
                "跳过内容检查",
                "跳过导入内容的安全检查。",
                toggle_control(BoolField::SkipContentCheck, config.skip_content_check),
            ),
            field_row(
                "启用扩展",
                "允许加载酒馆前端扩展。",
                toggle_control(BoolField::ExtensionsEnabled, config.extensions_enabled),
            ),
            field_row(
                "扩展自动更新",
                "自动更新已安装的酒馆扩展。",
                toggle_control(
                    BoolField::ExtensionsAutoUpdate,
                    config.extensions_auto_update,
                ),
            ),
            field_row(
                "启用服务器插件",
                "允许加载酒馆服务端插件。",
                toggle_control(BoolField::EnableServerPlugins, config.enable_server_plugins),
            ),
            field_row(
                "服务器插件自动更新",
                "自动更新已安装的服务端插件。",
                toggle_control(
                    BoolField::EnableServerPluginsAutoUpdate,
                    config.enable_server_plugins_auto_update,
                ),
            ),
            field_row(
                "Authelia SSO",
                "启用 Authelia 单点登录认证。",
                toggle_control(BoolField::AutheliaAuth, config.authelia_auth),
            ),
            field_row(
                "Authentik SSO",
                "启用 Authentik 单点登录认证。",
                toggle_control(BoolField::AuthentikAuth, config.authentik_auth),
            ),
            field_row(
                "启用缓存清除",
                "按 User-Agent 匹配并清除浏览器缓存。",
                toggle_control(BoolField::CacheBusterEnabled, config.cache_buster_enabled),
            ),
            field_row(
                "User-Agent 匹配模式",
                "匹配需要清除缓存的浏览器。",
                text_control(
                    "例如：Chrome.*",
                    &config.cache_buster_pattern,
                    TextField::CacheBusterPattern,
                    false,
                ),
            ),
            field_row(
                "启用 CORS 代理",
                "启用 SillyTavern 内置的 CORS 代理。",
                toggle_control(BoolField::EnableCorsProxy, config.enable_cors_proxy),
            ),
            field_row(
                "可下载分词器",
                "允许按需下载 AI 模型分词器。",
                toggle_control(
                    BoolField::EnableDownloadableTokenizers,
                    config.enable_downloadable_tokenizers,
                ),
            ),
        ]),
    )
}

fn status_badge(last_action: Option<TavernAction>) -> Element<'static, TavernMessage> {
    let (label, color) = if last_action.is_some() {
        ("操作待接入", BLUE_600)
    } else {
        ("就绪", SUCCESS)
    };
    container(
        row![
            icons::icon(Icon::CircleCheck, 13, color),
            text(label).size(10).font(fonts::MEDIUM).color(color)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([7, 9])
    .style(status_badge_style(color))
    .into()
}

fn header_button(
    label: &'static str,
    icon: Icon,
    action: TavernAction,
) -> Element<'static, TavernMessage> {
    button(
        row![
            crate::theme::muted_icon(icon, 14),
            text(label).size(11).font(fonts::MEDIUM)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(TavernMessage::Action(action))
    .height(36)
    .padding([7, 11])
    .style(button_style(ButtonVariant::Outline))
    .into()
}

/// 配置分组沿用旧版的独立折叠卡片交互。
fn advanced_group<'a>(
    index: usize,
    expanded: bool,
    icon: Icon,
    accent: Color,
    title: &'static str,
    description: &'static str,
    content: Element<'a, TavernMessage>,
) -> Element<'a, TavernMessage> {
    let header = button(
        row![
            container(icons::icon(icon, 18, accent))
                .width(40)
                .height(40)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(section_icon_style(accent)),
            column![
                text(title)
                    .size(15)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
                text(description)
                    .size(11)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style)
            ]
            .spacing(3)
            .width(Fill),
            icons::icon(
                if expanded {
                    Icon::ChevronUp
                } else {
                    Icon::ChevronDown
                },
                16,
                INK_MUTED,
            )
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .on_press(TavernMessage::ToggleAdvancedSection(index))
    .width(Fill)
    .padding(16)
    .style(button_style(ButtonVariant::Ghost));

    let mut group = column![header].width(Fill);
    if expanded {
        group = group.push(crate::theme::separator()).push(content);
    }

    container(group).width(Fill).style(config_card_style).into()
}

fn section_rows<'a>(rows: Vec<Element<'a, TavernMessage>>) -> Element<'a, TavernMessage> {
    column(rows).spacing(10).padding(20).width(Fill).into()
}

fn field_row<'a>(
    title: &'static str,
    description: &'static str,
    control: Element<'a, TavernMessage>,
) -> Element<'a, TavernMessage> {
    container(
        row![
            column![
                text(title)
                    .size(13)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
                text(description)
                    .size(11)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style)
            ]
            .spacing(3)
            .width(Fill),
            control
        ]
        .spacing(18)
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .padding([13, 15])
    .style(setting_row_style)
    .width(Fill)
    .into()
}

fn dimension_row<'a>(
    title: &'static str,
    width: &'a str,
    height: &'a str,
    width_field: TextField,
    height_field: TextField,
) -> Element<'a, TavernMessage> {
    field_row(
        title,
        "宽度 × 高度，单位为像素。",
        row![
            compact_text_control("宽", width, width_field),
            text("×").size(12).style(crate::theme::muted_text_style),
            compact_text_control("高", height, height_field),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
        .width(CONTROL_WIDTH)
        .into(),
    )
}

fn compact_text_control<'a>(
    placeholder: &'static str,
    value: &'a str,
    field: TextField,
) -> Element<'a, TavernMessage> {
    text_input(placeholder, value)
        .on_input(move |value| TavernMessage::Edit(field, value))
        .width(112)
        .padding([8, 11])
        .size(12)
        .style(text_input_style)
        .into()
}

fn toggle_control(field: BoolField, value: bool) -> Element<'static, TavernMessage> {
    crate::theme::switch("", value, move |enabled| {
        TavernMessage::Toggle(field, enabled)
    })
}

fn pill_toggle(
    label: &'static str,
    field: BoolField,
    value: bool,
) -> Element<'static, TavernMessage> {
    button(
        row![
            if value {
                icons::icon(Icon::CircleCheck, 13, WHITE)
            } else {
                crate::theme::muted_icon(Icon::Circle, 13)
            },
            text(label)
                .size(11)
                .font(fonts::MEDIUM)
                .style(move |theme| iced::widget::text::Style {
                    color: Some(if value {
                        WHITE
                    } else {
                        crate::theme::text_muted(theme)
                    }),
                }),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .on_press(TavernMessage::Toggle(field, !value))
    .height(38)
    .padding([8, 13])
    .style(pill_toggle_style(value))
    .into()
}

fn stacked_field<'a>(
    label: &'static str,
    control: Element<'a, TavernMessage>,
    help: Option<&'static str>,
) -> Element<'a, TavernMessage> {
    let mut content = column![
        text(label)
            .size(10)
            .font(fonts::MEDIUM)
            .style(crate::theme::muted_text_style),
        control,
    ]
    .spacing(7)
    .width(Fill);
    if let Some(help) = help {
        content = content.push(
            text(help)
                .size(10)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        );
    }
    content.into()
}

fn text_control<'a>(
    placeholder: &'static str,
    value: &'a str,
    field: TextField,
    secure: bool,
) -> Element<'a, TavernMessage> {
    text_input(placeholder, value)
        .on_input(move |value| TavernMessage::Edit(field, value))
        .secure(secure)
        .width(CONTROL_WIDTH)
        .padding([8, 11])
        .size(12)
        .style(text_input_style)
        .into()
}

fn wide_text_control<'a>(
    placeholder: &'static str,
    value: &'a str,
    field: TextField,
    secure: bool,
) -> Element<'a, TavernMessage> {
    text_input(placeholder, value)
        .on_input(move |value| TavernMessage::Edit(field, value))
        .secure(secure)
        .width(Fill)
        .padding([10, 14])
        .size(13)
        .style(text_input_style)
        .into()
}

fn select_control<'a, T: Copy + Eq + fmt::Display + 'a>(
    options: &'a [T],
    selected: T,
    on_selected: fn(T) -> TavernMessage,
) -> Element<'a, TavernMessage> {
    pick_list(options, Some(selected), on_selected)
        .width(CONTROL_WIDTH)
        .padding([8, 11])
        .text_size(12)
        .font(fonts::REGULAR)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style)
        .into()
}

fn wide_select_control<'a, T: Copy + Eq + fmt::Display + 'a>(
    options: &'a [T],
    selected: T,
    on_selected: fn(T) -> TavernMessage,
) -> Element<'a, TavernMessage> {
    pick_list(options, Some(selected), on_selected)
        .width(Fill)
        .padding([10, 14])
        .text_size(13)
        .font(fonts::REGULAR)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style)
        .into()
}

fn list_control<'a>(
    values: &'a [String],
    field: ListField,
    placeholder: &'static str,
) -> Element<'a, TavernMessage> {
    let mut items = column![].spacing(7).width(LIST_WIDTH);
    for (index, value) in values.iter().enumerate() {
        items = items.push(
            row![
                text_input(placeholder, value)
                    .on_input(move |value| TavernMessage::EditList(field, index, value))
                    .width(Fill)
                    .padding([8, 11])
                    .size(12)
                    .style(text_input_style),
                button(container(crate::theme::muted_icon(Icon::Trash2, 14)))
                    .on_press(TavernMessage::RemoveListItem(field, index))
                    .width(34)
                    .height(34)
                    .padding(0)
                    .style(button_style(ButtonVariant::DangerSoft)),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        );
    }
    items = items.push(
        button(
            row![
                icons::icon(Icon::Plus, 14, BLUE_600),
                text("添加").size(11).font(fonts::MEDIUM).color(BLUE_600)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(TavernMessage::AddListItem(field))
        .height(34)
        .padding([7, 11])
        .style(button_style(ButtonVariant::Tertiary)),
    );
    items.into()
}

fn page_icon_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BLUE_600)),
        border: Border {
            radius: 10.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn section_icon_style(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            accent.r, accent.g, accent.b, 0.10,
        ))),
        border: Border {
            radius: 10.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn config_card_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 14.0.into(),
        },
        ..container::Style::default()
    }
}

fn setting_row_style(theme: &Theme) -> container::Style {
    let surface_alt = crate::theme::surface_alt(theme);
    let line = crate::theme::line(theme);
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            surface_alt.r,
            surface_alt.g,
            surface_alt.b,
            0.55,
        ))),
        border: Border {
            color: Color::from_rgba(line.r, line.g, line.b, 0.85),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    }
}

fn pill_toggle_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let hovered = matches!(status, button::Status::Hovered);
        let surface_alt = crate::theme::surface_alt(theme);
        let line = crate::theme::line(theme);
        let background = if active {
            theme.palette().primary
        } else if hovered {
            Color::from_rgba(
                theme.palette().primary.r,
                theme.palette().primary.g,
                theme.palette().primary.b,
                0.16,
            )
        } else {
            surface_alt
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color: if active {
                WHITE
            } else {
                crate::theme::text_muted(theme)
            },
            border: Border {
                color: if active {
                    theme.palette().primary
                } else {
                    line
                },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..button::Style::default()
        }
    }
}

fn status_badge_style(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.08,
        ))),
        border: Border {
            color: Color::from_rgba(color.r, color.g, color.b, 0.22),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{BoolField, ListField, TavernMessage, TavernState, TextField};

    #[test]
    fn edits_update_the_shared_configuration_draft() {
        let mut state = TavernState::default();
        state.update(TavernMessage::Edit(TextField::Port, "9000".into()));

        assert_eq!(state.config.port, "9000");
    }

    #[test]
    fn list_editing_and_defaults_are_stable() {
        let mut state = TavernState::default();
        state.update(TavernMessage::AddListItem(ListField::Whitelist));
        state.update(TavernMessage::EditList(
            ListField::Whitelist,
            2,
            "192.168.1.20".into(),
        ));
        state.update(TavernMessage::Toggle(BoolField::Listen, true));

        assert_eq!(state.config.whitelist[2], "192.168.1.20");
        assert!(state.config.listen);

        state.update(TavernMessage::RestoreDefaults);
        assert_eq!(state.config.port, "8000");
        assert!(!state.config.listen);
        assert_eq!(state.config.whitelist.len(), 2);
    }

    #[test]
    fn advanced_groups_keep_the_old_initial_expansion() {
        let mut state = TavernState::default();
        assert_eq!(
            state.advanced_expanded,
            [true, false, false, false, false, false, false, false, false]
        );

        state.update(TavernMessage::ToggleAdvancedSection(1));
        assert!(state.advanced_expanded[0]);
        assert!(state.advanced_expanded[1]);
    }
}
