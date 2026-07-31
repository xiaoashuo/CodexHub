//! Responses <-> Chat conversion based on waliapi's `protocol` module.
//!
//! Codex adds `custom` and `namespace` tools to the public Responses shape;
//! those are normalized here at the protocol boundary instead of leaking into
//! provider adapters.

use serde_json::{json, Value};

pub(crate) fn responses_to_openai(payload: &Value, upstream_model: &str) -> Value {
    let mut body = json!({
        "model": upstream_model,
        "messages": input_to_messages(payload.get("input")),
        "stream": payload.get("stream").and_then(Value::as_bool).unwrap_or(false),
        "max_tokens": payload.get("max_output_tokens").cloned().unwrap_or_else(|| json!(4096)),
    });

    if let Some(instructions) = payload.get("instructions").and_then(Value::as_str).filter(|v| !v.is_empty()) {
        if let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) {
            messages.insert(0, json!({"role": "system", "content": instructions}));
        }
    }
    for key in ["temperature", "top_p", "parallel_tool_calls"] {
        if let Some(value) = payload.get(key) { body[key] = value.clone(); }
    }
    if let Some(tools) = payload.get("tools").and_then(Value::as_array) {
        let tools = tools.iter().flat_map(response_tool_to_openai).collect::<Vec<_>>();
        if !tools.is_empty() { body["tools"] = Value::Array(tools); }
    }
    if let Some(choice) = payload.get("tool_choice") {
        body["tool_choice"] = tool_choice_to_openai(choice);
    }
    body
}

fn input_to_messages(input: Option<&Value>) -> Value {
    let Some(input) = input else { return Value::Array(Vec::new()); };
    let Some(items) = input.as_array() else {
        return Value::Array(vec![json!({"role": "user", "content": text(input)})]);
    };
    let mut messages = Vec::new();
    let mut pending_calls = Vec::new();
    let flush_calls = |messages: &mut Vec<Value>, calls: &mut Vec<Value>| {
        if !calls.is_empty() {
            messages.push(json!({"role": "assistant", "content": null, "tool_calls": std::mem::take(calls)}));
        }
    };
    for item in items {
        match item.get("type").and_then(Value::as_str).unwrap_or("") {
            "function_call" | "custom_tool_call" => {
                if let Some(call) = input_call_to_openai(item) { pending_calls.push(call); }
            }
            "function_call_output" | "custom_tool_call_output" => {
                flush_calls(&mut messages, &mut pending_calls);
                let id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("");
                messages.push(json!({"role": "tool", "tool_call_id": id, "content": text(item.get("output").unwrap_or(item))}));
            }
            _ => {
                flush_calls(&mut messages, &mut pending_calls);
                if let Some(role) = item.get("role").and_then(Value::as_str) {
                    messages.push(json!({"role": normalize_role(role), "content": text(item.get("content").unwrap_or(item))}));
                } else if item.is_string() || item.get("text").is_some() {
                    messages.push(json!({"role": "user", "content": text(item)}));
                }
            }
        }
    }
    flush_calls(&mut messages, &mut pending_calls);
    Value::Array(messages)
}

fn input_call_to_openai(item: &Value) -> Option<Value> {
    let name = item.get("name")?.as_str()?.trim();
    if name.is_empty() { return None; }
    let name = normalize_chat_tool_name(name);
    let id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("call_router");
    let arguments = if item.get("type").and_then(Value::as_str) == Some("custom_tool_call") {
        json!({"input": item.get("input").cloned().unwrap_or(Value::Null)}).to_string()
    } else {
        item.get("arguments").map(|value| value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string())).unwrap_or_else(|| "{}".into())
    };
    Some(json!({"id": id, "type": "function", "function": {"name": name, "arguments": arguments}}))
}

fn response_tool_to_openai(tool: &Value) -> Vec<Value> {
    match tool.get("type").and_then(Value::as_str).unwrap_or("") {
        "function" => {
            let mut function = tool.get("function").cloned().unwrap_or_else(|| json!({
                "name": tool.get("name").cloned().unwrap_or(Value::Null),
                "description": tool.get("description").cloned().unwrap_or(Value::Null),
                "parameters": tool.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }));
            if let Some(name) = function.get("name").and_then(Value::as_str) {
                function["name"] = Value::String(normalize_chat_tool_name(name));
            }
            vec![json!({"type": "function", "function": function})]
        }
        "custom" => tool.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).map(|name| vec![json!({"type":"function", "function":{"name":normalize_chat_tool_name(name), "description":tool.get("description").cloned().unwrap_or(Value::Null), "parameters":{"type":"object","properties":{"input":{"type":"string"}},"required":["input"]}}})]).unwrap_or_default(),
        "namespace" => tool.get("tools").and_then(Value::as_array).map(|children| children.iter().filter_map(|child| {
            let namespace = tool.get("name").and_then(Value::as_str).unwrap_or("");
            let name = child.get("name").and_then(Value::as_str)?;
            (child.get("type").and_then(Value::as_str) == Some("function")).then(|| json!({"type":"function","function":{"name":normalize_chat_tool_name(&flatten(namespace, name)),"description":child.get("description").cloned().unwrap_or(Value::Null),"parameters":child.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object","properties":{}}))}}))
        }).collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn tool_choice_to_openai(choice: &Value) -> Value {
    if let Some(name) = choice.get("name").and_then(Value::as_str) {
        return json!({"type":"function", "function":{"name":normalize_chat_tool_name(name)}});
    }
    choice.clone()
}

fn flatten(namespace: &str, name: &str) -> String { if namespace.is_empty() { name.into() } else { format!("{}__{}", namespace, name) } }
fn normalize_chat_tool_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            normalized.push(character);
        } else if character == '.' {
            normalized.push_str("__");
        } else {
            normalized.push('_');
        }
    }
    normalized
}
fn normalize_role(role: &str) -> &str { match role { "assistant" | "system" | "tool" => role, _ => "user" } }
fn text(value: &Value) -> String {
    match value { Value::String(value) => value.clone(), Value::Array(values) => values.iter().map(text).filter(|value| !value.is_empty()).collect::<Vec<_>>().join("\n"), Value::Object(_) => value.get("text").and_then(Value::as_str).map(str::to_string).or_else(|| value.get("content").map(text)).unwrap_or_else(|| value.to_string()), Value::Null => String::new(), value => value.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_parallel_calls_and_flattens_namespace_tools() {
        let payload = json!({"input":[{"type":"function_call","call_id":"a","name":"read","arguments":"{}"},{"type":"function_call","call_id":"b","name":"write","arguments":"{}"}],"tools":[{"type":"namespace","name":"fs","tools":[{"type":"function","name":"read","parameters":{}}]}]});
        let result = responses_to_openai(&payload, "test");
        assert_eq!(result["messages"][0]["tool_calls"].as_array().unwrap().len(), 2);
        assert_eq!(result["tools"][0]["function"]["name"], "fs__read");
    }

    #[test]
    fn normalizes_dotted_tool_names_for_chat_completions() {
        let payload = json!({
            "input": [{
                "type": "function_call",
                "call_id": "call_1",
                "name": "image_gen.imagegen",
                "arguments": "{}"
            }],
            "tools": [
                {
                    "type": "function",
                    "name": "image_gen.imagegen",
                    "parameters": {"type": "object", "properties": {}}
                },
                {
                    "type": "custom",
                    "name": "computer.use"
                }
            ],
            "tool_choice": {
                "type": "function",
                "name": "image_gen.imagegen"
            }
        });

        let result = responses_to_openai(&payload, "test");
        assert_eq!(
            result["messages"][0]["tool_calls"][0]["function"]["name"],
            "image_gen__imagegen"
        );
        assert_eq!(
            result["tools"][0]["function"]["name"],
            "image_gen__imagegen"
        );
        assert_eq!(result["tools"][1]["function"]["name"], "computer__use");
        assert_eq!(
            result["tool_choice"]["function"]["name"],
            "image_gen__imagegen"
        );
    }
}
