//! 免费在线翻译源池：有道（国内快）+ 谷歌免费接口（国外快），均无需 key。
//! 非官方接口，可能限流；选择与熔断策略在 lib.rs 的 web_translate 里。

use openidea_plugin::{host, Error, HttpRequest};
use serde_json::Value;

const YOUDAO_URL: &str = "https://fanyi.youdao.com/translate";
const GOOGLE_URL: &str = "https://translate.googleapis.com/translate_a/single?client=gtx&dt=t";
/// 在线源的每次请求超时；熔断决策的度量衡
const TIMEOUT_MS: u32 = 4000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WebSrc {
    Youdao,
    Google,
}

impl WebSrc {
    pub const ALL: [WebSrc; 2] = [WebSrc::Youdao, WebSrc::Google];

    pub fn index(self) -> usize {
        match self {
            WebSrc::Youdao => 0,
            WebSrc::Google => 1,
        }
    }

    pub fn other(self) -> Self {
        match self {
            WebSrc::Youdao => WebSrc::Google,
            WebSrc::Google => WebSrc::Youdao,
        }
    }

    /// 展示用的来源标签（历史记录、状态栏）
    pub fn label(self) -> &'static str {
        match self {
            WebSrc::Youdao => "有道",
            WebSrc::Google => "谷歌",
        }
    }

    pub fn translate(self, text: &str, to_zh: bool) -> Result<String, Error> {
        match self {
            WebSrc::Youdao => youdao(text, to_zh),
            WebSrc::Google => google(text, to_zh),
        }
    }
}

fn youdao(text: &str, to_zh: bool) -> Result<String, Error> {
    let to = if to_zh { "ZH_CN" } else { "EN" };
    let body = format!("i={}&from=AUTO&to={}&doctype=json", form_encode(text), to);
    let mut req =
        HttpRequest::post(YOUDAO_URL, body).header("Content-Type", "application/x-www-form-urlencoded");
    req.timeout_ms = Some(TIMEOUT_MS);
    let resp = host::fetch(&req)?;
    if resp.status != 200 {
        return Err(format!("有道接口返回 {}", resp.status).into());
    }
    let v: Value = serde_json::from_slice(&resp.body).map_err(|e| format!("解析有道响应失败: {e}"))?;
    let code = v["errorCode"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(format!("有道接口错误码 {code}").into());
    }
    // translateResult 是按行分组的数组，行内每段一个 {src, tgt}
    let mut lines: Vec<String> = Vec::new();
    if let Some(rows) = v["translateResult"].as_array() {
        for row in rows {
            let line: String = row
                .as_array()
                .map(|segs| {
                    segs.iter()
                        .filter_map(|s| s["tgt"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            lines.push(line);
        }
    }
    let out = lines.join("\n").trim().to_string();
    if out.is_empty() {
        return Err("有道接口返回了空内容".into());
    }
    Ok(out)
}

fn google(text: &str, to_zh: bool) -> Result<String, Error> {
    let tl = if to_zh { "zh-CN" } else { "en" };
    let url = format!("{}&sl=auto&tl={tl}&q={}", GOOGLE_URL, form_encode(text));
    let mut req = HttpRequest::get(&url);
    req.timeout_ms = Some(TIMEOUT_MS);
    let resp = host::fetch(&req)?;
    if resp.status != 200 {
        return Err(format!("谷歌接口返回 {}", resp.status).into());
    }
    let v: Value = serde_json::from_slice(&resp.body).map_err(|e| format!("解析谷歌响应失败: {e}"))?;
    // 响应形如 [[["译文","原文",…],…],…]，译文片段在 v[0][i][0]
    let out = v[0]
        .as_array()
        .map(|segs| {
            segs.iter()
                .filter_map(|s| s[0].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
        .trim()
        .to_string();
    if out.is_empty() {
        return Err("谷歌接口返回了空内容".into());
    }
    Ok(out)
}

/// application/x-www-form-urlencoded 的百分号编码（空格转 +）。
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_form_values() {
        assert_eq!(form_encode("hello world"), "hello+world");
        assert_eq!(form_encode("余额"), "%E4%BD%99%E9%A2%9D");
        assert_eq!(form_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn parses_youdao_segments() {
        let body = serde_json::json!({
            "errorCode": 0,
            "translateResult": [[{"src": "poisoned", "tgt": "中毒"}, {"src": "mutex", "tgt": "互斥锁"}]]
        })
        .to_string();
        let v: Value = serde_json::from_str(&body).unwrap();
        let segs = &v["translateResult"][0];
        let joined: String = segs.as_array().unwrap().iter().filter_map(|s| s["tgt"].as_str()).collect();
        assert_eq!(joined, "中毒互斥锁");
    }
}
