//! AI 翻译源：任何 OpenAI 兼容接口（DeepSeek / Kimi / 本地 Ollama 等）。
//! API Key 由 IDE 代填：请求头里的 `{{secret:deepseek-key}}`。

use openidea_plugin::{host, Error, HttpRequest};
use serde_json::{json, Value};

use crate::SECRET;
use crate::setting_str;

pub fn chat_text(prompt: &str) -> Result<String, Error> {
    let endpoint = setting_str("endpoint", "https://api.deepseek.com");
    let model = setting_str("model", "deepseek-chat");
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.2
    });

    let mut req =
        HttpRequest::post(&chat_completions_url(&endpoint), body.to_string()).header("Content-Type", "application/json");
    if host::secret_is_set(SECRET) {
        req = req.header("Authorization", "Bearer {{secret:deepseek-key}}");
    }

    let resp = host::fetch(&req)?;
    if resp.status != 200 {
        let snippet: String = resp.text().chars().take(200).collect();
        return Err(format!("翻译接口返回 {}: {}", resp.status, snippet).into());
    }
    let v: Value =
        serde_json::from_slice(&resp.body).map_err(|e| format!("解析翻译接口响应失败: {e}"))?;
    let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return Err("翻译接口返回了空内容".into());
    }
    Ok(content.trim().to_string())
}

pub fn translate(text: &str, to_zh: bool) -> Result<String, Error> {
    let target = if to_zh { "Simplified Chinese" } else { "English" };
    let prompt = format!(
        "Translate the following text into {target}. Reply with the translation only, no explanations:\n\n{text}"
    );
    chat_text(&prompt)
}

/// 容忍三种端点写法：`https://api.deepseek.com`、`…/v1`、已带完整路径 `…/v1/chat/completions`。
fn chat_completions_url(endpoint: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.ends_with("chat/completions") {
        return base.to_string();
    }
    let last = base.rsplit('/').next().unwrap_or("");
    if last.len() > 1
        && last.starts_with('v')
        && last[1..].chars().all(|c| c.is_ascii_digit())
    {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_url_for_all_endpoint_styles() {
        assert_eq!(
            chat_completions_url("https://api.deepseek.com"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("http://localhost:11434/v1"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://x.io/v1/chat/completions"),
            "https://x.io/v1/chat/completions"
        );
    }
}
