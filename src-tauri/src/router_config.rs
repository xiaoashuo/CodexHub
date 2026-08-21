use crate::{
    build_user_relative_path, default_router_auth_method, default_router_concurrency_limit,
    default_router_name, default_router_port, ensure_parent_dir, load_app_settings,
    normalize_port, normalize_router_auth_method, normalize_router_concurrency_limit,
    ROUTER_CONFIG_RELATIVE_PATH, ROUTER_HOST,
};
use std::fs;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouterCommonConfig {
    pub router_name: String,
    pub base_url: String,
    #[serde(default = "default_router_auth_method")]
    pub auth_method: String,
    #[serde(default)]
    pub auth_external_token: String,
    #[serde(default)]
    pub auth_env_key: String,
    #[serde(default)]
    pub model_catalog_json: String,
    #[serde(default)]
    pub default_model: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouterSystemConfig {
    #[serde(flatten)]
    pub common: RouterCommonConfig,
    #[serde(default = "default_router_port")]
    pub router_port: u16,
    #[serde(default = "default_router_concurrency_limit")]
    pub concurrency_limit: usize,
}

pub type RouterExternalConfig = RouterCommonConfig;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouterRuntimeConfig {
    pub router_mode: i32,
    pub restart: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouterConfig {
    pub system_config: RouterSystemConfig,
    pub external_config: RouterExternalConfig,
    pub runtime: RouterRuntimeConfig,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            system_config: RouterSystemConfig {
                common: RouterCommonConfig {
                    router_name: default_router_name(),
                    base_url: String::new(),
                    auth_method: default_router_auth_method(),
                    auth_external_token: String::new(),
                    auth_env_key: String::new(),
                    model_catalog_json: String::new(),
                    default_model: String::new(),
                },
                router_port: default_router_port(),
                concurrency_limit: default_router_concurrency_limit(),
            },
            external_config: RouterCommonConfig {
                router_name: String::new(),
                base_url: String::new(),
                auth_method: default_router_auth_method(),
                auth_external_token: String::new(),
                auth_env_key: String::new(),
                model_catalog_json: String::new(),
                default_model: String::new(),
            },
            runtime: RouterRuntimeConfig {
                router_mode: 0,
                restart: 0,
            },
        }
    }
}

fn router_config_path() -> Result<std::path::PathBuf, String> {
    build_user_relative_path(ROUTER_CONFIG_RELATIVE_PATH)
}

fn normalize_router_config(mut config: RouterConfig) -> RouterConfig {
    normalize_common_config(&mut config.system_config.common);
    config.system_config.router_port = normalize_port(config.system_config.router_port, default_router_port());
    config.system_config.concurrency_limit = normalize_router_concurrency_limit(config.system_config.concurrency_limit);
    config.system_config.common.base_url = format!(
        "http://{}:{}/v1",
        ROUTER_HOST,
        config.system_config.router_port
    );
    normalize_common_config(&mut config.external_config);
    config.runtime.router_mode = if config.runtime.router_mode == 1 { 1 } else { 0 };
    config.runtime.restart = if config.runtime.restart == 1 { 1 } else { 0 };
    config
}

fn normalize_common_config(config: &mut RouterCommonConfig) {
    config.router_name = config.router_name.trim().to_string();
    config.base_url = config.base_url.trim().to_string();
    config.auth_method = normalize_router_auth_method(&config.auth_method);
    config.auth_external_token = config.auth_external_token.trim().to_string();
    config.auth_env_key = config.auth_env_key.trim().to_string();
    config.model_catalog_json = config.model_catalog_json.trim().to_string();
    config.default_model = config.default_model.trim().to_string();
}

fn migrate_router_config_from_app_settings() -> RouterConfig {
    let mut config = RouterConfig::default();
    if let Ok(settings) = load_app_settings() {
        if !settings.router_name.trim().is_empty() {
            config.system_config.common.router_name = settings.router_name.trim().to_string();
        }
        config.system_config.common.base_url = settings.router_base_url.trim().to_string();
        config.system_config.common.auth_method = settings.router_auth_method.trim().to_string();
        config.system_config.common.auth_external_token = settings.router_auth_external_token.trim().to_string();
        config.system_config.common.auth_env_key = settings.router_auth_env_key.trim().to_string();
        config.system_config.common.model_catalog_json = settings.router_model_catalog_json.trim().to_string();
        config.system_config.common.default_model = settings.router_default_model.trim().to_string();
        config.system_config.router_port = settings.router_port;
        config.system_config.concurrency_limit = settings.router_concurrency_limit;
        config.runtime.router_mode = if settings.router_mode == "third" { 1 } else { 0 };
        config.runtime.restart = if settings.router_auto_restart { 1 } else { 0 };
    }
    config
}

pub fn load_router_config_file() -> Result<RouterConfig, String> {
    let path = router_config_path()?;
    ensure_parent_dir(&path)?;
    if !path.exists() {
        let config = migrate_router_config_from_app_settings();
        write_router_config_file(&config)?;
        return Ok(config);
    }

    let text = fs::read_to_string(&path)
        .map_err(|error| format!("读取路由配置失败：{}，路径：{}", error, path.display()))?;
    if text.trim().is_empty() {
        return Ok(RouterConfig::default());
    }
    let config = serde_json::from_str::<RouterConfig>(&text)
        .map_err(|error| format!("解析路由配置失败：{}，路径：{}", error, path.display()))?;
    Ok(normalize_router_config(config))
}

fn write_router_config_file(config: &RouterConfig) -> Result<(), String> {
    let path = router_config_path()?;
    ensure_parent_dir(&path)?;
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| format!("序列化路由配置失败：{}", error))?;
    fs::write(&path, text)
        .map_err(|error| format!("写入路由配置失败：{}，路径：{}", error, path.display()))
}

pub fn save_router_config_file(config: RouterConfig) -> Result<RouterConfig, String> {
    let normalized = normalize_router_config(config);
    write_router_config_file(&normalized)?;
    Ok(normalized)
}
