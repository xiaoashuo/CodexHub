use crate::{
    CHAT_COMPLETIONS_ENDPOINT_SUFFIX, MESSAGES_ENDPOINT_SUFFIX, RESPONSES_ENDPOINT_SUFFIX,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderProtocol {
    OpenAi,
    Other,
    Anthropic,
    CpaMc,
}

/// Protocol-specific concerns live behind this interface.  Request/response
/// conversion remains in the router because it is Codex-format specific,
/// while endpoint and authentication conventions stay with the provider
/// adapter.
pub(crate) trait ProviderAdapter: Send + Sync {
    fn default_completion_endpoint(&self) -> &'static str;
    fn apply_authentication(&self, request: ureq::Request, api_key: &str) -> ureq::Request;
}

struct BearerAdapter(ProviderProtocol);
struct AnthropicAdapter;

impl ProviderAdapter for BearerAdapter {
    fn default_completion_endpoint(&self) -> &'static str {
        match self.0 {
            ProviderProtocol::OpenAi | ProviderProtocol::Other => CHAT_COMPLETIONS_ENDPOINT_SUFFIX,
            ProviderProtocol::CpaMc => RESPONSES_ENDPOINT_SUFFIX,
            ProviderProtocol::Anthropic => unreachable!("Anthropic uses its own adapter"),
        }
    }

    fn apply_authentication(&self, request: ureq::Request, api_key: &str) -> ureq::Request {
        request.set("Authorization", &format!("Bearer {}", api_key))
    }
}

impl ProviderAdapter for AnthropicAdapter {
    fn default_completion_endpoint(&self) -> &'static str { MESSAGES_ENDPOINT_SUFFIX }
    fn apply_authentication(&self, request: ureq::Request, api_key: &str) -> ureq::Request {
        request
            .set("x-api-key", api_key)
            .set("anthropic-version", "2023-06-01")
    }
}

impl ProviderProtocol {
    pub(crate) fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" | "chat-com" | "chat-completion" | "chat-completions" => Self::OpenAi,
            "deepseek" | "other" => Self::Other,
            "anthropic" | "claude" | "messages" => Self::Anthropic,
            "responses" | "cpamc" | "openapi" | "" => Self::CpaMc,
            _ => Self::CpaMc,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Other => "other",
            Self::Anthropic => "anthropic",
            Self::CpaMc => "cpamc",
        }
    }

    pub(crate) fn default_completion_endpoint(self) -> &'static str {
        self.adapter().default_completion_endpoint()
    }

    pub(crate) fn adapter(self) -> &'static dyn ProviderAdapter {
        static OPENAI: BearerAdapter = BearerAdapter(ProviderProtocol::OpenAi);
        static OTHER: BearerAdapter = BearerAdapter(ProviderProtocol::Other);
        static CPAMC: BearerAdapter = BearerAdapter(ProviderProtocol::CpaMc);
        static ANTHROPIC: AnthropicAdapter = AnthropicAdapter;
        match self {
            Self::OpenAi => &OPENAI,
            Self::Other => &OTHER,
            Self::Anthropic => &ANTHROPIC,
            Self::CpaMc => &CPAMC,
        }
    }
}
