//! 免费在线翻译源池：有道（国内快）+ 谷歌免费接口（国外快），均无需 key。
//! 非官方接口，可能限流；选择与熔断策略在 lib.rs 的 web_translate 里。

use openidea_plugin::{host, Error, HttpRequest};
use serde_json::Value;

// fanyi.youdao.com/translate 已 302 到 error.html；aidemo 是同一家仍在服务的
// 免 key 接口，参数与响应都不同，见 youdao()。
const YOUDAO_URL: &str = "https://aidemo.youdao.com/trans";
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
    let to = if to_zh { "zh-CHS" } else { "en" };
    let body = format!("q={}&from=Auto&to={to}", form_encode(text));
    let mut req =
        HttpRequest::post(YOUDAO_URL, body).header("Content-Type", "application/x-www-form-urlencoded");
    req.timeout_ms = Some(TIMEOUT_MS);
    let resp = host::fetch(&req)?;
    if resp.status != 200 {
        return Err(format!("有道接口返回 {}", resp.status).into());
    }
    let v: Value = serde_json::from_slice(&resp.body).map_err(|e| format!("解析有道响应失败: {e}"))?;
    parse_youdao(&v)
}

fn parse_youdao(v: &Value) -> Result<String, Error> {
    // errorCode 在这里是字符串（"0"），老接口是数字，两种都收
    let code = v["errorCode"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| v["errorCode"].as_i64().map(|n| n.to_string()))
        .unwrap_or_else(|| "-1".into());
    if code != "0" {
        let msg = v["msg"].as_str().unwrap_or("").trim();
        return Err(if msg.is_empty() {
            format!("有道接口错误码 {code}").into()
        } else {
            format!("有道接口错误码 {code}：{msg}").into()
        });
    }
    // translation 是译文片段数组，长句可能拆成多段
    let out = v["translation"]
        .as_array()
        .map(|segs| {
            segs.iter()
                .filter_map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
        .trim()
        .to_string();
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
    fn parses_youdao_translation() {
        let v = serde_json::json!({
            "errorCode": "0",
            "translation": ["配置", "接口地址"]
        });
        assert_eq!(parse_youdao(&v).unwrap(), "配置接口地址");
    }

    #[test]
    fn reports_youdao_error_code_and_message() {
        let v = serde_json::json!({"errorCode": "411", "msg": "请求频率过快"});
        assert_eq!(
            parse_youdao(&v).unwrap_err().to_string(),
            "有道接口错误码 411：请求频率过快"
        );
    }
}
