use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use windows::Win32::Networking::WinInet::{
    INTERNET_OPTION_PROXY_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH, InternetSetOptionW,
};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

const INTERNET_SETTINGS_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings";
const CONFIG_DIR_NAME: &str = "ProxyGuard";
const CONFIG_FILE_NAME: &str = "config.json";
const PORTABLE_MARKER_FILE: &str = "proxy_guard.portable";
const PORTABLE_CONFIG_DIR_NAME: &str = "config";
const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE_NAME: &str = "ProxyGuardHelper";
const HELPER_EXE_NAME: &str = "proxy_guard_helper.exe";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppConfig {
    pub managed_rules: Vec<ManagedRule>,
    pub cleanup_scope: CleanupScope,
    pub cleanup_on_login: bool,
    pub auto_start_helper: bool,
    pub meta: ConfigMeta,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            managed_rules: Vec::new(),
            cleanup_scope: CleanupScope::default(),
            cleanup_on_login: false,
            auto_start_helper: false,
            meta: ConfigMeta::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigMeta {
    pub version: u32,
    pub saved_at: DateTime<Utc>,
}

impl Default for ConfigMeta {
    fn default() -> Self {
        Self {
            version: 1,
            saved_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CleanupScope {
    ShutdownOnly,
    ShutdownAndRestart,
    ShutdownRestartAndLogoff,
}

impl CleanupScope {
    pub fn includes_non_logoff(self) -> bool {
        matches!(
            self,
            CleanupScope::ShutdownOnly
                | CleanupScope::ShutdownAndRestart
                | CleanupScope::ShutdownRestartAndLogoff
        )
    }

    pub fn includes_logoff(self) -> bool {
        matches!(self, CleanupScope::ShutdownRestartAndLogoff)
    }

    pub fn options() -> [CleanupScope; 2] {
        [
            CleanupScope::ShutdownAndRestart,
            CleanupScope::ShutdownRestartAndLogoff,
        ]
    }

    pub fn display_name(self) -> &'static str {
        match self {
            CleanupScope::ShutdownOnly => "关机/重启（旧配置兼容项）",
            CleanupScope::ShutdownAndRestart => "关机/重启（非注销）",
            CleanupScope::ShutdownRestartAndLogoff => "关机+重启+注销",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            CleanupScope::ShutdownOnly => "仅在非注销的系统结束事件时清理。Windows 无法可靠区分关机和重启。",
            CleanupScope::ShutdownAndRestart => "默认推荐：在非注销的系统结束事件时清理。",
            CleanupScope::ShutdownRestartAndLogoff => "包括注销在内的所有会话结束事件都清理。",
        }
    }
}

impl Default for CleanupScope {
    fn default() -> Self {
        CleanupScope::ShutdownAndRestart
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManagedRule {
    ManualProxy {
        label: String,
        normalized_proxy_server: String,
        recommended: bool,
    },
    PacUrl {
        label: String,
        normalized_url: String,
        recommended: bool,
    },
}

impl ManagedRule {
    pub fn label(&self) -> &str {
        match self {
            ManagedRule::ManualProxy { label, .. } | ManagedRule::PacUrl { label, .. } => label,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyCandidate {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub recommended: bool,
    pub rule: ManagedRule,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemProxySnapshot {
    pub proxy_enable: bool,
    pub proxy_server: Option<String>,
    pub auto_config_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvent {
    pub is_logoff: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupResult {
    pub cleaned_manual_proxy: bool,
    pub cleaned_pac: bool,
    pub changed: bool,
}

pub trait ProxySettingsStore {
    fn load_snapshot(&self) -> Result<SystemProxySnapshot>;
    fn save_snapshot(&self, snapshot: &SystemProxySnapshot) -> Result<()>;
}

pub struct RegistryProxySettingsStore;

impl RegistryProxySettingsStore {
    pub fn new() -> Self {
        Self
    }

    fn open_key(access: u32) -> Result<RegKey> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS_SUBKEY, access)
            .with_context(|| format!("failed to open registry key {}", INTERNET_SETTINGS_SUBKEY))
    }
}

impl ProxySettingsStore for RegistryProxySettingsStore {
    fn load_snapshot(&self) -> Result<SystemProxySnapshot> {
        let key = Self::open_key(KEY_READ)?;
        let proxy_enable: u32 = key.get_value("ProxyEnable").unwrap_or(0);
        let proxy_server = key.get_value::<String, _>("ProxyServer").ok();
        let auto_config_url = key.get_value::<String, _>("AutoConfigURL").ok();

        Ok(SystemProxySnapshot {
            proxy_enable: proxy_enable != 0,
            proxy_server,
            auto_config_url,
        })
    }

    fn save_snapshot(&self, snapshot: &SystemProxySnapshot) -> Result<()> {
        let key = Self::open_key(KEY_WRITE)?;
        key.set_value("ProxyEnable", &(snapshot.proxy_enable as u32))
            .context("failed to write ProxyEnable")?;

        match snapshot.proxy_server.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(value) => key
                .set_value("ProxyServer", &value)
                .context("failed to write ProxyServer")?,
            None => {
                let _ = key.delete_value("ProxyServer");
            }
        }

        match snapshot
            .auto_config_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            Some(value) => key
                .set_value("AutoConfigURL", &value)
                .context("failed to write AutoConfigURL")?,
            None => {
                let _ = key.delete_value("AutoConfigURL");
            }
        }

        Ok(())
    }
}

pub fn config_dir() -> Result<PathBuf> {
    if let Some(portable_dir) = portable_config_dir()? {
        return Ok(portable_dir);
    }

    let local_app_data = dirs::data_local_dir().ok_or_else(|| anyhow!("LOCALAPPDATA is unavailable"))?;
    Ok(local_app_data.join(CONFIG_DIR_NAME))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

pub fn is_portable_mode() -> Result<bool> {
    Ok(portable_base_dir()?.is_some())
}

pub fn helper_executable_path() -> Result<PathBuf> {
    let exe_path = std::env::current_exe().context("failed to resolve current executable path")?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("current executable has no parent directory"))?;
    Ok(exe_dir.join(HELPER_EXE_NAME))
}

pub fn is_helper_auto_start_enabled() -> Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(RUN_SUBKEY, KEY_READ)
        .with_context(|| format!("failed to open run key {}", RUN_SUBKEY))?;
    Ok(key.get_value::<String, _>(RUN_VALUE_NAME).is_ok())
}

pub fn set_helper_auto_start(enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_SUBKEY)
        .with_context(|| format!("failed to open or create run key {}", RUN_SUBKEY))?;

    if enabled {
        let quoted = format!("\"{}\"", helper_executable_path()?.display());
        key.set_value(RUN_VALUE_NAME, &quoted)
            .context("failed to write helper auto-start registry value")?;
    } else {
        let _ = key.delete_value(RUN_VALUE_NAME);
    }

    Ok(())
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let mut config = serde_json::from_str::<AppConfig>(&raw)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    if matches!(config.cleanup_scope, CleanupScope::ShutdownOnly) {
        config.cleanup_scope = CleanupScope::ShutdownAndRestart;
    }
    Ok(config)
}

pub fn save_config(mut config: AppConfig) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config directory {}", dir.display()))?;
    config.meta.version = 1;
    config.meta.saved_at = Utc::now();
    let path = dir.join(CONFIG_FILE_NAME);
    let raw = serde_json::to_string_pretty(&config).context("failed to serialize config")?;
    fs::write(&path, raw).with_context(|| format!("failed to write config file {}", path.display()))?;
    Ok(())
}

pub fn scan_candidates_from_store(store: &impl ProxySettingsStore) -> Result<Vec<ProxyCandidate>> {
    let snapshot = store.load_snapshot()?;
    Ok(scan_candidates_from_snapshot(&snapshot))
}

pub fn scan_candidates_from_snapshot(snapshot: &SystemProxySnapshot) -> Vec<ProxyCandidate> {
    let mut candidates = Vec::new();

    if let Some(raw_proxy_server) = snapshot
        .proxy_server
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(parsed) = parse_manual_proxy(raw_proxy_server) {
            let normalized = parsed.normalized();
            let recommended = parsed.is_loopback_only();
            let title = if parsed.has_protocol_map {
                "分协议代理".to_string()
            } else {
                "手动代理".to_string()
            };
            let detail = normalized.clone();
            let label = format!("{title}: {detail}");
            candidates.push(ProxyCandidate {
                id: format!("manual::{normalized}"),
                title,
                detail,
                recommended,
                rule: ManagedRule::ManualProxy {
                    label,
                    normalized_proxy_server: normalized,
                    recommended,
                },
            });
        }
    }

    if let Some(raw_pac_url) = snapshot
        .auto_config_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = normalize_pac_url(raw_pac_url);
        let recommended = is_loopback_url(&normalized);
        let label = format!("PAC: {normalized}");
        candidates.push(ProxyCandidate {
            id: format!("pac::{normalized}"),
            title: "PAC".to_string(),
            detail: normalized.clone(),
            recommended,
            rule: ManagedRule::PacUrl {
                label,
                normalized_url: normalized,
                recommended,
            },
        });
    }

    candidates
}

pub fn cleanup_from_store(
    store: &impl ProxySettingsStore,
    config: &AppConfig,
    event: &CleanupEvent,
) -> Result<CleanupResult> {
    if !event.is_logoff && !config.cleanup_scope.includes_non_logoff() {
        return Ok(CleanupResult::default());
    }

    if event.is_logoff && !config.cleanup_scope.includes_logoff() {
        return Ok(CleanupResult::default());
    }

    let mut snapshot = store.load_snapshot()?;
    let result = cleanup_snapshot(&mut snapshot, config);
    if result.changed {
        store.save_snapshot(&snapshot)?;
        refresh_system_proxy()?;
    }
    Ok(result)
}

pub fn cleanup_snapshot(snapshot: &mut SystemProxySnapshot, config: &AppConfig) -> CleanupResult {
    let manual_match = snapshot
        .proxy_server
        .as_deref()
        .and_then(parse_manual_proxy)
        .map(|proxy| proxy.normalized())
        .and_then(|normalized| {
            config.managed_rules.iter().find(|rule| {
                matches!(
                    rule,
                    ManagedRule::ManualProxy {
                        normalized_proxy_server,
                        ..
                    } if normalized_proxy_server == &normalized
                )
            })
        })
        .is_some();

    let pac_match = snapshot
        .auto_config_url
        .as_deref()
        .map(normalize_pac_url)
        .and_then(|normalized| {
            config.managed_rules.iter().find(|rule| {
                matches!(
                    rule,
                    ManagedRule::PacUrl { normalized_url, .. } if normalized_url == &normalized
                )
            })
        })
        .is_some();

    let mut result = CleanupResult::default();

    if manual_match {
        snapshot.proxy_enable = false;
        snapshot.proxy_server = None;
        result.cleaned_manual_proxy = true;
        result.changed = true;
    }

    if pac_match {
        snapshot.auto_config_url = None;
        result.cleaned_pac = true;
        result.changed = true;
    }

    result
}

pub fn refresh_system_proxy() -> Result<()> {
    unsafe {
        InternetSetOptionW(None, INTERNET_OPTION_PROXY_SETTINGS_CHANGED, None, 0)
            .context("failed to notify Windows about proxy setting changes")?;
        InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0)
            .context("failed to refresh Windows proxy settings")?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManualProxySpec {
    has_protocol_map: bool,
    entries: Vec<ManualProxyEntry>,
}

impl ManualProxySpec {
    fn normalized(&self) -> String {
        let mut entries = self.entries.clone();
        entries.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        if self.has_protocol_map {
            entries
                .into_iter()
                .map(|entry| {
                    format!(
                        "{}={}:{}",
                        entry.protocol.as_deref().unwrap_or("all"),
                        entry.host,
                        entry.port
                    )
                })
                .collect::<Vec<_>>()
                .join(";")
        } else {
            let entry = entries.into_iter().next().expect("manual proxy entry");
            format!("{}:{}", entry.host, entry.port)
        }
    }

    fn is_loopback_only(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|entry| is_loopback_host(&entry.host))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManualProxyEntry {
    protocol: Option<String>,
    host: String,
    port: u16,
}

impl ManualProxyEntry {
    fn sort_key(&self) -> (String, String, u16) {
        (
            self.protocol.clone().unwrap_or_default(),
            self.host.clone(),
            self.port,
        )
    }
}

fn parse_manual_proxy(raw: &str) -> Option<ManualProxySpec> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    if value.contains('=') {
        let mut dedup = BTreeMap::<String, (String, u16)>::new();
        for part in value.split(';').map(str::trim).filter(|part| !part.is_empty()) {
            let (protocol, endpoint) = part.split_once('=')?;
            let protocol = protocol.trim().to_ascii_lowercase();
            let (host, port) = parse_endpoint(endpoint.trim())?;
            dedup.insert(protocol, (host, port));
        }
        let entries = dedup
            .into_iter()
            .map(|(protocol, (host, port))| ManualProxyEntry {
                protocol: Some(protocol),
                host,
                port,
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return None;
        }
        return Some(ManualProxySpec {
            has_protocol_map: true,
            entries,
        });
    }

    let (host, port) = parse_endpoint(value)?;
    Some(ManualProxySpec {
        has_protocol_map: false,
        entries: vec![ManualProxyEntry {
            protocol: None,
            host,
            port,
        }],
    })
}

fn parse_endpoint(value: &str) -> Option<(String, u16)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(stripped) = trimmed.strip_prefix('[').and_then(|value| value.split_once(']')) {
        let (host, remainder) = stripped;
        let port = remainder.strip_prefix(':')?.parse::<u16>().ok()?;
        return Some((host.to_ascii_lowercase(), port));
    }

    let (host, port) = trimmed.rsplit_once(':')?;
    let host = host.trim().to_ascii_lowercase();
    let port = port.trim().parse::<u16>().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host, port))
}

fn normalize_pac_url(url: &str) -> String {
    url.trim().to_ascii_lowercase()
}

fn is_loopback_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    [
        "http://127.0.0.1",
        "https://127.0.0.1",
        "http://localhost",
        "https://localhost",
        "http://[::1]",
        "https://[::1]",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn portable_config_dir() -> Result<Option<PathBuf>> {
    Ok(portable_base_dir()?.map(|base| base.join(PORTABLE_CONFIG_DIR_NAME)))
}

fn portable_base_dir() -> Result<Option<PathBuf>> {
    let exe_path = std::env::current_exe().context("failed to resolve current executable path")?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("current executable has no parent directory"))?;
    let marker_path = exe_dir.join(PORTABLE_MARKER_FILE);
    if marker_path.is_file() {
        return Ok(Some(exe_dir.to_path_buf()));
    }
    Ok(None)
}

pub fn selected_rules_from_candidates(
    candidates: &[ProxyCandidate],
    selected_ids: &BTreeSet<String>,
) -> Vec<ManagedRule> {
    candidates
        .iter()
        .filter(|candidate| selected_ids.contains(&candidate.id))
        .map(|candidate| candidate.rule.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        snapshot: std::sync::Mutex<SystemProxySnapshot>,
    }

    impl MemoryStore {
        fn new(snapshot: SystemProxySnapshot) -> Self {
            Self {
                snapshot: std::sync::Mutex::new(snapshot),
            }
        }
    }

    impl ProxySettingsStore for MemoryStore {
        fn load_snapshot(&self) -> Result<SystemProxySnapshot> {
            Ok(self.snapshot.lock().unwrap().clone())
        }

        fn save_snapshot(&self, snapshot: &SystemProxySnapshot) -> Result<()> {
            *self.snapshot.lock().unwrap() = snapshot.clone();
            Ok(())
        }
    }

    #[test]
    fn parses_simple_proxy_server() {
        let spec = parse_manual_proxy("127.0.0.1:7890").unwrap();
        assert_eq!(spec.normalized(), "127.0.0.1:7890");
        assert!(spec.is_loopback_only());
    }

    #[test]
    fn parses_protocol_proxy_server() {
        let spec = parse_manual_proxy("https=127.0.0.1:7890;http=127.0.0.1:7890").unwrap();
        assert_eq!(spec.normalized(), "http=127.0.0.1:7890;https=127.0.0.1:7890");
    }

    #[test]
    fn scans_manual_and_pac_candidates() {
        let snapshot = SystemProxySnapshot {
            proxy_enable: true,
            proxy_server: Some("http=127.0.0.1:7890;https=127.0.0.1:7890".to_string()),
            auto_config_url: Some("http://localhost:7890/proxy.pac".to_string()),
        };

        let candidates = scan_candidates_from_snapshot(&snapshot);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| candidate.recommended));
    }

    #[test]
    fn cleanup_matches_only_selected_manual_proxy() {
        let store = MemoryStore::new(SystemProxySnapshot {
            proxy_enable: true,
            proxy_server: Some("127.0.0.1:7890".to_string()),
            auto_config_url: Some("http://remote/pac".to_string()),
        });
        let config = AppConfig {
            managed_rules: vec![ManagedRule::ManualProxy {
                label: "Manual".to_string(),
                normalized_proxy_server: "127.0.0.1:7890".to_string(),
                recommended: true,
            }],
            cleanup_scope: CleanupScope::ShutdownAndRestart,
            cleanup_on_login: false,
            auto_start_helper: false,
            meta: ConfigMeta::default(),
        };

        let result = cleanup_from_store(&store, &config, &CleanupEvent { is_logoff: false }).unwrap();
        let snapshot = store.load_snapshot().unwrap();

        assert!(result.cleaned_manual_proxy);
        assert!(!result.cleaned_pac);
        assert!(!snapshot.proxy_enable);
        assert!(snapshot.proxy_server.is_none());
        assert_eq!(snapshot.auto_config_url.as_deref(), Some("http://remote/pac"));
    }

    #[test]
    fn cleanup_matches_only_selected_pac() {
        let store = MemoryStore::new(SystemProxySnapshot {
            proxy_enable: false,
            proxy_server: Some("remote:8080".to_string()),
            auto_config_url: Some("http://localhost:7890/proxy.pac".to_string()),
        });
        let config = AppConfig {
            managed_rules: vec![ManagedRule::PacUrl {
                label: "PAC".to_string(),
                normalized_url: "http://localhost:7890/proxy.pac".to_string(),
                recommended: true,
            }],
            cleanup_scope: CleanupScope::ShutdownAndRestart,
            cleanup_on_login: false,
            auto_start_helper: false,
            meta: ConfigMeta::default(),
        };

        let result = cleanup_from_store(&store, &config, &CleanupEvent { is_logoff: false }).unwrap();
        let snapshot = store.load_snapshot().unwrap();

        assert!(!result.cleaned_manual_proxy);
        assert!(result.cleaned_pac);
        assert_eq!(snapshot.proxy_server.as_deref(), Some("remote:8080"));
        assert!(snapshot.auto_config_url.is_none());
    }

    #[test]
    fn logoff_cleanup_respects_scope() {
        let store = MemoryStore::new(SystemProxySnapshot {
            proxy_enable: true,
            proxy_server: Some("127.0.0.1:7890".to_string()),
            auto_config_url: None,
        });
        let config = AppConfig {
            managed_rules: vec![ManagedRule::ManualProxy {
                label: "Manual".to_string(),
                normalized_proxy_server: "127.0.0.1:7890".to_string(),
                recommended: true,
            }],
            cleanup_scope: CleanupScope::ShutdownAndRestart,
            cleanup_on_login: false,
            auto_start_helper: false,
            meta: ConfigMeta::default(),
        };

        let result = cleanup_from_store(&store, &config, &CleanupEvent { is_logoff: true }).unwrap();
        let snapshot = store.load_snapshot().unwrap();

        assert!(!result.changed);
        assert!(snapshot.proxy_server.is_some());
    }

    #[test]
    fn portable_mode_detection_is_false_without_marker() {
        assert!(!is_portable_mode().unwrap());
    }
}
