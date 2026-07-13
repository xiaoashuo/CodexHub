use super::*;

pub(crate) const ROUTER_HOST: &str = "127.0.0.1";
pub(crate) const ROUTER_PORT: u16 = 25817;
pub(crate) const HEALTH_PATH: &str = "/health";
pub(crate) const RESPONSES_PATH: &str = "/codex/router/v1/responses";
pub(crate) const CHAT_COMPLETIONS_PATH: &str = "/codex/router/v1/chat/completions";
pub(crate) const ACCOUNT_PROXY_MODELS_PATH: &str = "/v1/models";
pub(crate) const ACCOUNT_PROXY_RESPONSES_PATH: &str = "/v1/responses";
pub(crate) const ACCOUNT_PROXY_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
pub(crate) const ACCOUNT_PROXY_MESSAGES_PATH: &str = "/v1/messages";
pub(crate) const OAUTH_CALLBACK_PATH: &str = "/auth/callback";
pub(crate) const OAUTH_CALLBACK_PORT: u16 = 1455;
pub(crate) const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub(crate) const OAUTH_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub(crate) const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const OAUTH_SCOPE: &str = "openid email profile offline_access";
pub(crate) const CHATGPT_SESSION_API_URL: &str = "https://chatgpt.com/api/auth/session";
pub(crate) const OFFICIAL_CODEX_RESPONSES_URL: &str =
    "https://chatgpt.com/backend-api/codex/responses";
pub(crate) const OFFICIAL_CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/codex/usage";
pub(crate) const OFFICIAL_WHAM_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
pub(crate) const DEFAULT_CODEX_INSTRUCTIONS: &str = "You are Codex, a coding assistant.";
pub(crate) const OFFICIAL_CODEX_ORIGINATOR: &str = "codex_cli_rs";
pub(crate) const OFFICIAL_CODEX_BETA_HEADER_VALUE: &str = "responses=experimental";
pub(crate) const OFFICIAL_CODEX_COMPLETED_EVENT: &str = "event: response.completed";
pub(crate) const OFFICIAL_CODEX_COMPLETED_DATA: &str = "data: {}";
pub(crate) const OFFICIAL_TARGET_PROVIDER: &str = "official";
pub(crate) const OFFICIAL_INPUT_KEY: &str = "input";
pub(crate) const RESPONSES_INPUT_NAMESPACE_KEY: &str = "namespace";
pub(crate) const RESTART_TARGET_CODEX: &str = "codex";
pub(crate) const RESTART_TARGET_CHATGPT: &str = "chatgpt";
pub(crate) const OFFICIAL_INSTRUCTIONS_KEY: &str = "instructions";
pub(crate) const OFFICIAL_STORE_KEY: &str = "store";
pub(crate) const OFFICIAL_STREAM_KEY: &str = "stream";
pub(crate) const OFFICIAL_TEMPERATURE_KEY: &str = "temperature";
pub(crate) const OFFICIAL_MAX_OUTPUT_TOKENS_KEY: &str = "max_output_tokens";
pub(crate) const DEFAULT_MAX_OUTPUT_TOKENS: u64 = 4096;
pub(crate) const CODEX_AUTH_ACCOUNTS_KEY: &str = "accounts";
pub(crate) const CODEX_ACCESS_TOKEN_KEYS: &[&str] = &["access_token", "accessToken"];
pub(crate) const CODEX_ACCOUNT_ID_KEYS: &[&str] = &[
    "account_id",
    "accountId",
    "chatgpt_account_id",
    "chatgptAccountId",
];
pub(crate) const SSE_LINE_ENDING: &str = "\n";
pub(crate) const SERVICE_NAME: &str = "codex-router";
pub(crate) const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) static UPDATE_DOWNLOAD_CANCELED: AtomicBool = AtomicBool::new(false);
pub(crate) const RESPONSE_NOT_FOUND: &str = "{\"error\":\"not_found\"}";
pub(crate) const RESPONSE_METHOD_NOT_ALLOWED: &str = "{\"error\":\"method_not_allowed\"}";
pub(crate) const RESPONSE_CONFIG_MISSING: &str = "{\"error\":\"router_provider_config_missing\",\"message\":\"请创建本地配置文件.codex\\\\ai-router-workspace\\\\config\\\\router_provider_config.json\"}";
pub(crate) const RESPONSE_ROUTE_MISSING: &str = "{\"error\":\"router_provider_route_missing\"}";
pub(crate) const ADDRESS_IN_USE_ERROR_CODE: i32 = 10048;
pub(crate) const UNKNOWN_PORT_OWNER_VALUE: &str = "unknown";
pub(crate) const REQUEST_LOG_LIMIT: usize = 200;
pub(crate) const LISTENER_IDLE_SLEEP_MS: u64 = 50;
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 20 * 1024 * 1024;
pub(crate) const FORWARD_TIMEOUT_SECONDS: u64 = 600;
pub(crate) const OFFICIAL_FORWARD_TIMEOUT_SECONDS: u64 = 300;
pub(crate) const IMAGE_GENERATION_FORWARD_TIMEOUT_SECONDS: u64 = 300;
pub(crate) const UPSTREAM_NETWORK_RETRY_ATTEMPTS: usize = 2;
pub(crate) const UPSTREAM_NETWORK_RETRY_DELAY_MS: u64 = 500;
pub(crate) const DEFAULT_ROUTER_CONCURRENCY_LIMIT: usize = 8;
pub(crate) const MIN_ROUTER_CONCURRENCY_LIMIT: usize = 1;
pub(crate) const MAX_ROUTER_CONCURRENCY_LIMIT: usize = 64;
pub(crate) const MODEL_TEST_TIMEOUT_SECONDS: u64 = 30;
pub(crate) const MODEL_LIST_TIMEOUT_SECONDS: u64 = 30;
pub(crate) const MODELS_PATH: &str = "/models";
pub(crate) const CATALOG_MODELS_KEY: &str = "models";
pub(crate) const MODELS_ENDPOINT_SUFFIX: &str = "/models";
pub(crate) const CHAT_COMPLETIONS_ENDPOINT_SUFFIX: &str = "/chat/completions";
pub(crate) const MESSAGES_ENDPOINT_SUFFIX: &str = "/messages";
pub(crate) const RESPONSES_ENDPOINT_SUFFIX: &str = "/responses";
pub(crate) const V1_PATH_SUFFIX: &str = "/v1";
pub(crate) const CODEX_PROXY_MANAGED_KEY: &str = "codex_proxy_managed";
pub(crate) const WORKSPACE_RELATIVE_PATH: &[&str] = &[".codex", "ai-router-workspace"];
pub(crate) const WORKSPACE_CONFIG_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "config"];
pub(crate) const WORKSPACE_LOGS_APP_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "logs", "app"];
pub(crate) const WORKSPACE_LOGS_ROUTER_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "logs", "router"];
pub(crate) const WORKSPACE_BACKUP_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "backup"];
pub(crate) const WORKSPACE_CACHE_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "cache"];
pub(crate) const WORKSPACE_RUNTIME_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "runtime"];
pub(crate) const WORKSPACE_ACCOUNTS_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "accounts"];
pub(crate) const CONFIG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "config",
    "router_provider_config.json",
];
pub(crate) const CATALOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "config",
    "codex_router_catalog.json",
];
pub(crate) const CATALOG_BASE_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "config",
    "codex_router_catalog_base.json",
];
pub(crate) const CODEX_CONFIG_RELATIVE_PATH: &[&str] = &[".codex", "config.toml"];
pub(crate) const CODEX_AUTH_RELATIVE_PATH: &[&str] = &[".codex", "auth.json"];
pub(crate) const CODEX_PLUGINS_CACHE_RELATIVE_PATH: &[&str] = &[".codex", "plugins", "cache"];
pub(crate) const CODEX_PLUGIN_STATE_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "config",
    "codex_plugin_state.json",
];
#[allow(dead_code)]
pub(crate) const CODEX_GLOBAL_STATE_RELATIVE_PATH: &[&str] =
    &[".codex", ".codex-global-state.json"];
#[allow(dead_code)]
pub(crate) const CODEX_GLOBAL_STATE_BACKUP_RELATIVE_PATH: &[&str] =
    &[".codex", ".codex-global-state.json.bak"];
pub(crate) const CODEX_ACCOUNTS_REGISTRY_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "accounts", "registry.json"];
pub(crate) const CODEX_ACCOUNTS_BACKUPS_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "backup", "accounts"];
pub(crate) const CODEX_ACCOUNTS_SNAPSHOTS_RELATIVE_PATH: &[&str] =
    &[".codex", "ai-router-workspace", "accounts", "snapshots"];
pub(crate) const CODEX_CONFIG_BACKUP_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "backup",
    "codex-config",
    "config.codex-router.bak.toml",
];
pub(crate) const APP_LOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "logs",
    "app",
    "app_operation.log",
];
pub(crate) const ROUTER_LOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "logs",
    "router",
    "router_request.log",
];
pub(crate) const ROUTER_DEBUG_LOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "logs",
    "router",
    "router_debug.log",
];
pub(crate) const ROUTER_FULL_DEBUG_LOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "logs",
    "router",
    "router_full_debug.log",
];
pub(crate) const ROUTER_FULL_DEBUG_LOG_ENABLED: bool = false;
pub(crate) const ACCOUNT_PROXY_LOG_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "logs",
    "router",
    "account_proxy_request.log",
];
pub(crate) const APP_SETTINGS_RELATIVE_PATH: &[&str] = &[
    ".codex",
    "ai-router-workspace",
    "config",
    "app_settings.json",
];
pub(crate) const MODELS_CACHE_RELATIVE_PATH: &[&str] = &[".codex", "models_cache.json"];
pub(crate) const EMPTY_JSON_OBJECT_CONTENT: &str = "{}";
pub(crate) const APP_LOG_MAX_SIZE_BYTES: u64 = 1024 * 1024;
pub(crate) const APP_LOG_TRIM_KEEP_BYTES: usize = 512 * 1024;
pub(crate) const APP_LOG_DEFAULT_LIMIT: usize = 100;
pub(crate) const APP_LOG_MAX_LIMIT: usize = 500;
pub(crate) const APP_LOG_DEDUP_WINDOW_MS: u128 = 1500;
pub(crate) const ACCOUNT_USAGE_VERBOSE_APP_LOGS: bool = true;
pub(crate) const FILE_PREVIEW_MAX_BYTES: usize = 64 * 1024;
pub(crate) const ROUTER_DEBUG_STRING_LIMIT: usize = 16 * 1024;
pub(crate) const ROUTER_DEBUG_BODY_LIMIT: usize = 512 * 1024;
pub(crate) const EMPTY_LOG_VALUE: &str = "-";
pub(crate) const TOKEN_USAGE_SOURCE_UPSTREAM: &str = "upstream";
pub(crate) const TOKEN_USAGE_SOURCE_MISSING: &str = "missing";
pub(crate) const CUSTOM_IMAGE_GENERATION_UNSUPPORTED_MESSAGE: &str =
    "当前模型不支持图片生成，或第三方 Provider 没有返回可识别的图片结果。请切换支持图片生成的模型后重试。";
pub(crate) const CUSTOM_UPSTREAM_EMPTY_RESPONSE_MESSAGE: &str =
    "自定义模型上游返回空文本响应。当前会话可能过长，或包含大量工具调用、命令输出和 Codex 原生上下文；请切换官方模型继续，或新开会话并带上简要进度后重试。";
pub(crate) const HEADER_CONTENT_TYPE: &str = "Content-Type";
pub(crate) const HEADER_AUTHORIZATION: &str = "Authorization";
pub(crate) const HEADER_ACCEPT: &str = "Accept";
pub(crate) const HEADER_COOKIE: &str = "Cookie";
pub(crate) const HEADER_ANTHROPIC_API_KEY: &str = "x-api-key";
pub(crate) const HEADER_ANTHROPIC_VERSION: &str = "anthropic-version";
pub(crate) const ANTHROPIC_VERSION_VALUE: &str = "2023-06-01";
pub(crate) const HEADER_CHATGPT_ACCOUNT_ID: &str = "chatgpt-account-id";
pub(crate) const HEADER_OPENAI_BETA: &str = "OpenAI-Beta";
pub(crate) const HEADER_ORIGINATOR: &str = "originator";
pub(crate) const HEADER_ORIGIN: &str = "Origin";
pub(crate) const HEADER_REFERER: &str = "Referer";
pub(crate) const HEADER_USER_AGENT: &str = "User-Agent";
pub(crate) const HEADER_JSON: &str = "application/json";
pub(crate) const HEADER_JSON_UTF8: &str = "application/json; charset=utf-8";
pub(crate) const HEADER_EVENT_STREAM: &str = "text/event-stream; charset=utf-8";
pub(crate) const HTTPS_PROXY_ENV: &str = "HTTPS_PROXY";
pub(crate) const HTTP_PROXY_ENV: &str = "HTTP_PROXY";
pub(crate) const ALL_PROXY_ENV: &str = "ALL_PROXY";
pub(crate) const OFFICIAL_CONNECT_TIMEOUT_HINT: &str =
    "官方 Codex 后端连接超时，请检查网络是否能直连 chatgpt.com，或在设置中配置官方转发代理。";
pub(crate) const PROXY_TEST_URL: &str = "https://chatgpt.com";
pub(crate) const PROXY_TEST_TIMEOUT_SECONDS: u64 = 5;
pub(crate) const VERSION_CHECK_TIMEOUT_SECONDS: u64 = 10;
pub(crate) const CODEXHUB_LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/xiaoashuo/CodexHub/releases/latest";
pub(crate) const CODEXHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/xiaoashuo/CodexHub/releases";
pub(crate) const DEFAULT_ACCOUNT_USAGE_REFRESH_SECONDS: u64 = 60;
pub(crate) const ACCOUNT_USAGE_REFRESH_ALLOWED_SECONDS: &[u64] = &[30, 60, 180, 300];
pub(crate) const ACCOUNT_USAGE_INITIAL_DELAY_SECONDS: u64 = 12;
pub(crate) const ACCOUNT_USAGE_REFRESH_POLL_SECONDS: u64 = 30;
pub(crate) const ACCOUNT_USAGE_REQUEST_TIMEOUT_SECONDS: u64 = 5;
pub(crate) const PROXY_DETECT_CANDIDATES: &[&str] = &[
    "http://127.0.0.1:7890",
    "http://127.0.0.1:7897",
    "http://127.0.0.1:10809",
    "http://127.0.0.1:4002",
    "socks5://127.0.0.1:7890",
];
pub(crate) const HTTP_OK: u16 = 200;
pub(crate) const HTTP_NO_CONTENT: u16 = 204;
pub(crate) const HTTP_BAD_REQUEST: u16 = 400;
pub(crate) const HTTP_UNAUTHORIZED: u16 = 401;
pub(crate) const HTTP_FORBIDDEN: u16 = 403;
pub(crate) const HTTP_NOT_FOUND: u16 = 404;
pub(crate) const HTTP_METHOD_NOT_ALLOWED: u16 = 405;
pub(crate) const HTTP_PAYLOAD_TOO_LARGE: u16 = 413;
pub(crate) const HTTP_BAD_GATEWAY: u16 = 502;
pub(crate) const HTTP_SERVICE_UNAVAILABLE: u16 = 503;
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;
pub(crate) const CODEX_ROUTER_TOP_MANAGED_START_MARKER: &str =
    "# <<< codex-router top managed start";
pub(crate) const CODEX_ROUTER_TOP_MANAGED_END_MARKER: &str = "# <<< codex-router top managed end";
pub(crate) const CODEX_ROUTER_MANAGED_START_MARKER: &str =
    "# <<< codex-router provider managed start";
pub(crate) const CODEX_ROUTER_MANAGED_END_MARKER: &str = "# <<< codex-router provider managed end";
pub(crate) const CODEX_PROVIDER_NAME: &str = "ai-router";
pub(crate) const CODEX_MODEL_PROVIDER_NAME: &str = "Codex\u{4f34}\u{4fa3}";
pub(crate) const CODEX_WIRE_API: &str = "responses";
pub(crate) const CODEX_ROUTER_MODEL_DESCRIPTION: &str =
    "Custom model forwarded through local router.";
pub(crate) const CODEX_ROUTER_IDENTITY_PREFIX: &str =
    "You are Codex, a coding agent routed through the local AI Router.";
pub(crate) const ACCOUNT_KEY_SEPARATOR: &str = "__";
pub(crate) const TRAY_SHOW_ID: &str = "show";
pub(crate) const TRAY_EXIT_ID: &str = "exit";
pub(crate) const CODEX_SESSIONS_RELATIVE_PATH: &[&str] = &[".codex", "sessions"];
pub(crate) const CODEX_ARCHIVED_SESSIONS_RELATIVE_PATH: &[&str] = &[".codex", "archived_sessions"];
pub(crate) const CODEX_SESSION_INDEX_RELATIVE_PATH: &[&str] = &[".codex", "session_index.jsonl"];
pub(crate) const CODEX_STATE_DB_RELATIVE_PATH: &[&str] = &[".codex", "state_5.sqlite"];
pub(crate) const CODEX_SKILLS_RELATIVE_PATH: &[&str] = &[".codex", "skills"];
pub(crate) const MAX_THREAD_TITLE_CHARS: usize = 120;

pub(crate) static ROUTER_STATE: OnceLock<Mutex<RouterRuntime>> = OnceLock::new();
pub(crate) static ROUTER_LOGS: OnceLock<Mutex<Vec<RouterLogEntry>>> = OnceLock::new();
pub(crate) static CODEX_OAUTH_STATE: OnceLock<Mutex<Option<CodexOAuthLoginState>>> =
    OnceLock::new();
pub(crate) static CODEX_OAUTH_LAST_RESULT: OnceLock<Mutex<Option<CodexOAuthLoginStatus>>> =
    OnceLock::new();
pub(crate) static CODEX_OAUTH_CALLBACK_LISTENER: OnceLock<Result<(), String>> = OnceLock::new();
pub(crate) static CODEX_OAUTH_CALLBACK_LISTENER_PORT: OnceLock<u16> = OnceLock::new();
pub(crate) static ACCOUNT_USAGE_REFRESH_WORKER: OnceLock<()> = OnceLock::new();
pub(crate) static ACCOUNT_USAGE_REFRESH_IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> =
    OnceLock::new();
pub(crate) static ACCOUNT_TOKEN_REFRESH_IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> =
    OnceLock::new();
