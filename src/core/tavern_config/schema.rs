//! 旧版酒馆配置字段映射；界面草稿与磁盘类型分离，输入中间态不能写入 YAML。

use serde_json::Value;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Bool,
    Text,
    Ipv4,
    Ipv6,
    Integer(i64, i64),
    NullableInteger,
    List,
    Whitelist,
    Browser,
    Format,
    Log,
}
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub key: &'static str,
    pub path: &'static str,
    pub kind: Kind,
}
macro_rules! fields {
    ($($key:ident => $path:literal, $kind:expr;)+) => {
        pub const FIELDS: &[Field] = &[$(Field { key: stringify!($key), path: $path, kind: $kind },)+];
    };
}
use Kind::*;
fields! {
    port => "port", Integer(1, 65535);
    listen => "listen", Bool;
    listen_ipv4 => "listenAddress.ipv4", Ipv4;
    listen_ipv6 => "listenAddress.ipv6", Ipv6;
    protocol_ipv4 => "protocol.ipv4", Bool;
    protocol_ipv6 => "protocol.ipv6", Bool;
    basic_auth_mode => "basicAuthMode", Bool;
    enable_user_accounts => "enableUserAccounts", Bool;
    enable_discreet_login => "enableDiscreetLogin", Bool;
    per_user_basic_auth => "perUserBasicAuth", Bool;
    basic_auth_username => "basicAuthUser.username", Text;
    basic_auth_password => "basicAuthUser.password", Text;
    whitelist_mode => "whitelistMode", Bool;
    whitelist => "whitelist", Whitelist;
    cors_enabled => "cors.enabled", Bool;
    cors_origins => "cors.origin", List;
    cors_methods => "cors.methods", List;
    cors_allowed_headers => "cors.allowedHeaders", List;
    cors_exposed_headers => "cors.exposedHeaders", List;
    cors_credentials => "cors.credentials", Bool;
    cors_max_age => "cors.maxAge", NullableInteger;
    request_proxy_enabled => "requestProxy.enabled", Bool;
    request_proxy_url => "requestProxy.url", Text;
    proxy_bypass => "requestProxy.bypass", List;
    common_backups => "backups.common.numberOfBackups", Integer(-1, i64::MAX);
    chat_backups_enabled => "backups.chat.enabled", Bool;
    chat_backups_check_integrity => "backups.chat.checkIntegrity", Bool;
    chat_max_backups => "backups.chat.maxTotalBackups", Integer(-1, i64::MAX);
    chat_throttle_interval => "backups.chat.throttleInterval", Integer(0, i64::MAX);
    thumbnails_enabled => "thumbnails.enabled", Bool;
    thumbnail_format => "thumbnails.format", Format;
    thumbnail_quality => "thumbnails.quality", Integer(0, 100);
    background_width => "thumbnails.dimensions.bg.0", Integer(1, u32::MAX as i64);
    background_height => "thumbnails.dimensions.bg.1", Integer(1, u32::MAX as i64);
    avatar_width => "thumbnails.dimensions.avatar.0", Integer(1, u32::MAX as i64);
    avatar_height => "thumbnails.dimensions.avatar.1", Integer(1, u32::MAX as i64);
    persona_width => "thumbnails.dimensions.persona.0", Integer(1, u32::MAX as i64);
    persona_height => "thumbnails.dimensions.persona.1", Integer(1, u32::MAX as i64);
    browser_launch_enabled => "browserLaunch.enabled", Bool;
    browser_type => "browserLaunch.browser", Browser;
    ssl_enabled => "ssl.enabled", Bool;
    ssl_cert_path => "ssl.certPath", Text;
    ssl_key_path => "ssl.keyPath", Text;
    ssl_key_passphrase => "ssl.keyPassphrase", Text;
    dns_prefer_ipv6 => "dnsPreferIPv6", Bool;
    heartbeat_interval => "heartbeatInterval", Integer(0, i64::MAX);
    host_whitelist_enabled => "hostWhitelist.enabled", Bool;
    host_whitelist_scan => "hostWhitelist.scan", Bool;
    host_whitelist => "hostWhitelist.hosts", List;
    import_domains => "whitelistImportDomains", List;
    session_timeout => "sessionTimeout", Integer(-1, i64::MAX);
    disable_csrf_protection => "disableCsrfProtection", Bool;
    security_override => "securityOverride", Bool;
    allow_keys_exposure => "allowKeysExposure", Bool;
    skip_content_check => "skipContentCheck", Bool;
    enable_access_log => "logging.enableAccessLog", Bool;
    min_log_level => "logging.minLogLevel", Log;
    lazy_load_characters => "performance.lazyLoadCharacters", Bool;
    memory_cache_capacity => "performance.memoryCacheCapacity", Text;
    use_disk_cache => "performance.useDiskCache", Bool;
    cache_buster_enabled => "cacheBuster.enabled", Bool;
    cache_buster_pattern => "cacheBuster.userAgentPattern", Text;
    authelia_auth => "sso.autheliaAuth", Bool;
    authentik_auth => "sso.authentikAuth", Bool;
    extensions_enabled => "extensions.enabled", Bool;
    extensions_auto_update => "extensions.autoUpdate", Bool;
    enable_server_plugins => "enableServerPlugins", Bool;
    enable_server_plugins_auto_update => "enableServerPluginsAutoUpdate", Bool;
    enable_cors_proxy => "enableCorsProxy", Bool;
    prompt_placeholder => "promptPlaceholder", Text;
    enable_downloadable_tokenizers => "enableDownloadableTokenizers", Bool;
}

pub fn field(key: &str) -> Option<&'static Field> {
    FIELDS.iter().find(|field| field.key == key)
}

fn valid_ip_range(value: &str) -> bool {
    let (address, prefix) = value
        .split_once('/')
        .map_or((value, None), |(ip, bits)| (ip, Some(bits)));
    let max = if address.parse::<Ipv4Addr>().is_ok() {
        32
    } else if address.parse::<Ipv6Addr>().is_ok() {
        128
    } else {
        return false;
    };
    prefix.is_none_or(|bits| bits.parse::<u8>().is_ok_and(|bits| bits <= max))
}

/// 返回磁盘值；错误只含规则，不包含可能敏感的用户输入。
pub fn encode(field: &Field, ui: &Value) -> Result<Value, &'static str> {
    let text = || ui.as_str().ok_or("tavern.field_error.type_invalid");
    let integer = |min: i64, max: i64| -> Result<Value, &'static str> {
        let number = text()?
            .trim()
            .parse::<i64>()
            .map_err(|_| "tavern.field_error.invalid_integer")?;
        if !(min..=max).contains(&number) {
            return Err("tavern.field_error.out_of_range");
        }
        Ok(number.into())
    };
    Ok(match field.kind {
        Bool => Value::Bool(ui.as_bool().ok_or("tavern.field_error.type_invalid")?),
        Text => Value::String(text()?.to_owned()),
        Ipv4 => {
            text()?
                .trim()
                .parse::<Ipv4Addr>()
                .map_err(|_| "tavern.field_error.invalid_ipv4")?;
            Value::String(text()?.trim().to_owned())
        }
        Ipv6 => {
            text()?
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<Ipv6Addr>()
                .map_err(|_| "tavern.field_error.invalid_ipv6")?;
            Value::String(text()?.trim().to_owned())
        }
        Integer(min, max) => integer(min, max)?,
        NullableInteger => {
            if text()?.trim().is_empty() {
                Value::Null
            } else {
                integer(0, i64::MAX)?
            }
        }
        List | Whitelist => {
            let items = ui.as_array().ok_or("tavern.field_error.type_invalid")?;
            let mut values = Vec::new();
            for item in items {
                let item = item.as_str().ok_or("tavern.field_error.list_item_text")?.trim();
                if item.is_empty() {
                    return Err("tavern.field_error.list_item_empty");
                }
                if matches!(field.kind, Whitelist) && !valid_ip_range(item) {
                    return Err("tavern.field_error.whitelist_format");
                }
                values.push(Value::String(item.to_owned()));
            }
            Value::Array(values)
        }
        Browser => Value::String(
            match text()? {
                "System" => "default",
                "Chrome" => "chrome",
                "Firefox" => "firefox",
                "Edge" => "edge",
                "Safari" => "safari",
                _ => return Err("tavern.field_error.unsupported_value"),
            }
            .into(),
        ),
        Format => Value::String(
            match text()? {
                "Jpeg" => "jpg",
                "Png" => "png",
                "Webp" => "webp",
                _ => return Err("tavern.field_error.unsupported_value"),
            }
            .into(),
        ),
        Log => Value::from(match text()? {
            "Debug" => 0,
            "Info" => 1,
            "Warn" => 2,
            "Error" => 3,
            _ => return Err("tavern.field_error.unsupported_value"),
        }),
    })
}

/// 未支持的枚举保留在磁盘，只用 Unknown 标记提醒界面；未编辑时绝不覆盖它。
pub fn decode(field: &Field, raw: &Value) -> Result<Value, &'static str> {
    let result = match field.kind {
        Bool => Value::Bool(raw.as_bool().ok_or("tavern.field_error.type_invalid")?),
        Integer(_, _) => Value::String(raw.as_i64().ok_or("tavern.field_error.type_invalid")?.to_string()),
        NullableInteger if raw.is_null() => Value::String(String::new()),
        NullableInteger => Value::String(raw.as_i64().ok_or("tavern.field_error.type_invalid")?.to_string()),
        Browser => Value::String(
            match raw.as_str().ok_or("tavern.field_error.type_invalid")? {
                "default" => "System",
                "chrome" => "Chrome",
                "firefox" => "Firefox",
                "edge" => "Edge",
                "safari" => "Safari",
                _ => "Unknown",
            }
            .into(),
        ),
        Format => Value::String(
            match raw.as_str().ok_or("tavern.field_error.type_invalid")? {
                "jpg" | "jpeg" => "Jpeg",
                "png" => "Png",
                "webp" => "Webp",
                _ => "Unknown",
            }
            .into(),
        ),
        Log => Value::String(
            match raw.as_i64().ok_or("tavern.field_error.type_invalid")? {
                0 => "Debug",
                1 => "Info",
                2 => "Warn",
                3 => "Error",
                _ => "Unknown",
            }
            .into(),
        ),
        List | Whitelist => {
            if !raw
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string))
            {
                return Err("tavern.field_error.list_item_text");
            }
            raw.clone()
        }
        _ => Value::String(raw.as_str().ok_or("tavern.field_error.type_invalid")?.to_owned()),
    };
    if !matches!(field.kind, Browser | Format | Log) {
        encode(field, &result)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn inputs_accept_intermediate_ui_but_only_valid_values_encode() {
        for value in ["", "abc", "0", "65536", "99999999999999999999999999"] {
            assert!(encode(field("port").unwrap(), &json!(value)).is_err());
        }
        assert_eq!(
            encode(field("port").unwrap(), &json!("9000")).unwrap(),
            json!(9000)
        );
        assert!(encode(field("listen_ipv4").unwrap(), &json!("127.")).is_err());
        assert!(encode(field("listen_ipv6").unwrap(), &json!("[::]")).is_ok());
        assert_eq!(
            encode(field("cors_max_age").unwrap(), &json!("")).unwrap(),
            Value::Null
        );
        assert!(encode(field("session_timeout").unwrap(), &json!("-1")).is_ok());
        assert!(
            encode(
                field("whitelist").unwrap(),
                &json!(["192.168.0.0/16", "::1"])
            )
            .is_ok()
        );
        assert!(encode(field("whitelist").unwrap(), &json!(["127.0.0.1/40"])).is_err());
        assert!(encode(field("import_domains").unwrap(), &json!([""])).is_err());
    }
    #[test]
    fn mapping_uses_old_browser_log_and_format_values() {
        assert_eq!(
            encode(field("browser_type").unwrap(), &json!("System")).unwrap(),
            json!("default")
        );
        assert_eq!(
            encode(field("thumbnail_format").unwrap(), &json!("Jpeg")).unwrap(),
            json!("jpg")
        );
        assert_eq!(
            encode(field("min_log_level").unwrap(), &json!("Warn")).unwrap(),
            json!(2)
        );
        assert!(decode(field("port").unwrap(), &json!("9000")).is_err());
        assert_eq!(
            decode(field("thumbnail_format").unwrap(), &json!("new-format")).unwrap(),
            json!("Unknown")
        );
    }
}
