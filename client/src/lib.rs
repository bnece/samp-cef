#![allow(non_snake_case)]

use std::fs::{self, File};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Once, OnceLock};

use serde::Deserialize;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use winapi::shared::minwindef::HMODULE;
use winapi::um::libloaderapi::DisableThreadLibraryCalls;
use winapi::um::winnt::DLL_PROCESS_ATTACH;

pub mod app;
pub mod audio;
pub mod browser;

#[cfg(feature = "crash_logger")]
pub mod crash_logger;
pub mod external;
pub mod network;
pub mod render;
pub mod static_cell;
pub mod utils;

// TODO: Сделать человеческие модули звука

static INIT: Once = Once::new();
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

const DEFAULT_PORT_OFFSET: u16 = 100;

#[derive(Debug, Deserialize)]
struct ClientConfig {
    #[serde(default = "default_log_level")]
    log_level: String,
    #[serde(default = "default_port_offset")]
    port_offset: u16,
}

fn default_log_level() -> String {
    "info".to_owned()
}

const fn default_port_offset() -> u16 {
    DEFAULT_PORT_OFFSET
}

fn read_client_config(path: &Path) -> (ClientConfig, Option<String>) {
    let fallback = || ClientConfig {
        log_level: default_log_level(),
        port_offset: default_port_offset(),
    };

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (fallback(), None),
        Err(error) => {
            return (
                fallback(),
                Some(format!("cannot read client configuration: {error}")),
            );
        }
    };

    match serde_json::from_str::<ClientConfig>(&contents) {
        Ok(config) => (config, None),
        Err(error) => (
            fallback(),
            Some(format!("cannot parse client configuration: {error}")),
        ),
    }
}

fn parse_log_level(value: &str) -> Option<LevelFilter> {
    LevelFilter::from_str(value.trim().to_ascii_lowercase().as_str()).ok()
}

fn configured_log_level(config: &ClientConfig) -> (LevelFilter, Option<String>) {
    match parse_log_level(&config.log_level) {
        Some(level) => (level, None),
        None => (
            LevelFilter::INFO,
            Some(format!(
                "unknown log level {:?}; using info",
                config.log_level
            )),
        ),
    }
}

fn initialize_logging() {
    let config_path = crate::utils::cef_dir().join("config.json");
    let (config, config_warning) = read_client_config(&config_path);
    let (log_level, log_warning) = configured_log_level(&config);
    let log_path = crate::utils::game_dir().join("cef_client.log");
    let Ok(log_file) = File::create(&log_path) else {
        return;
    };

    let (writer, guard) = tracing_appender::non_blocking(log_file);
    let filter = Targets::new()
        .with_default(LevelFilter::WARN)
        .with_target("cef_client", log_level)
        .with_target("client_api", log_level)
        .with_target("cef_network", log_level);
    let format = tracing_subscriber::fmt::layer()
        .compact()
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_writer(writer);

    if tracing_subscriber::registry()
        .with(filter)
        .with(format)
        .try_init()
        .is_ok()
    {
        let _ = LOG_GUARD.set(guard);
        tracing::info!(
            level = %log_level,
            config = %config_path.display(),
            log = %log_path.display(),
            "logging initialized"
        );

        if let Some(warning) = config_warning {
            tracing::warn!(reason = %warning, "logging configuration fallback applied");
        }
        if let Some(warning) = log_warning {
            tracing::warn!(reason = %warning, "logging configuration fallback applied");
        }
    }
}

pub(crate) fn configured_port_offset() -> u16 {
    let config_path = crate::utils::cef_dir().join("config.json");
    let (config, warning) = read_client_config(&config_path);
    if let Some(warning) = warning {
        tracing::warn!(reason = %warning, "network configuration fallback applied");
    }
    config.port_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_info() {
        let path = std::env::temp_dir().join(format!(
            "samp-cef-missing-config-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let (config, warning) = read_client_config(&path);
        assert!(warning.is_none());
        assert_eq!(config.log_level, "info");
        assert_eq!(config.port_offset, DEFAULT_PORT_OFFSET);
    }

    #[test]
    fn parses_supported_log_levels_case_insensitively() {
        assert_eq!(parse_log_level("off"), Some(LevelFilter::OFF));
        assert_eq!(parse_log_level("ERROR"), Some(LevelFilter::ERROR));
        assert_eq!(parse_log_level(" warn "), Some(LevelFilter::WARN));
        assert_eq!(parse_log_level("info"), Some(LevelFilter::INFO));
        assert_eq!(parse_log_level("debug"), Some(LevelFilter::DEBUG));
        assert_eq!(parse_log_level("trace"), Some(LevelFilter::TRACE));
        assert_eq!(parse_log_level("verbose"), None);
    }

    #[test]
    fn parses_custom_port_offset() {
        let config: ClientConfig = serde_json::from_str(
            r#"{"log_level":"debug","port_offset":120}"#,
        )
        .unwrap();

        assert_eq!(config.port_offset, 120);
        assert_eq!(configured_log_level(&config), (LevelFilter::DEBUG, None));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cef_client_initialize() {
    INIT.call_once(|| {
        initialize_logging();

        #[cfg(feature = "crash_logger")]
        crash_logger::initialize();

        app::initialize();
    });
}

/// # Safety
/// `instance` must be a valid module handle provided by the loader.
#[unsafe(no_mangle)]
pub unsafe extern "stdcall" fn DllMain(instance: HMODULE, reason: u32, _reserved: u32) -> bool {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(instance);
        }
    }

    true
}
