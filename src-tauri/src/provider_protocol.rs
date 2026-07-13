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
        match self {
            Self::OpenAi | Self::Other => CHAT_COMPLETIONS_ENDPOINT_SUFFIX,
            Self::Anthropic => MESSAGES_ENDPOINT_SUFFIX,
            Self::CpaMc => RESPONSES_ENDPOINT_SUFFIX,
        }
    }
}
