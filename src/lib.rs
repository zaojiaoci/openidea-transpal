//! TransPal — OpenIDEA 翻译插件。
//!
//! 三个命令：选中文本翻译（中英互判）、变量名译成中文、中文命名生成。
//! 翻译源自动选择：密钥里配了 AI Key 走 AI（OpenAI 兼容接口），否则走内置词典。
//! 翻译结果按「源 + 文本 + 方向」缓存在插件 storage 里。

use openidea_plugin::{
    Diagnostic, Error, Invocation, LogLevel, NotificationLevel, Plugin, Position, Range, Severity, StatusItem,
    TextEdit, host, register_plugin,
};

mod ai;
mod dict;
mod naming;
mod web;

pub struct TransPal {
    /// 在线源粘性偏好：上次成功的那条下次优先
    preferred: Option<web::WebSrc>,
    /// 每条在线源本次会话内的连续失败数；达到 2 即熔断跳过
    failures: [u8; web::WebSrc::ALL.len()],
    /// 上次悬停提示发布的文件：翻译换文件时清掉旧文件上的提示
    hover_path: Option<String>,
}

impl Default for TransPal {
    fn default() -> Self {
        Self { preferred: None, failures: [0; web::WebSrc::ALL.len()], hover_path: None }
    }
}

const SECRET: &str = "deepseek-key";
const HISTORY_KEY: &str = "transpal:history";
const HISTORY_MAX: usize = 20;
const VIEW: &str = "results";

fn setting_str(key: &str, default: &str) -> String {
    match host::setting(key) {
        Some(v) => {
            let t = v.trim();
            let unquoted = t.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(t);
            if unquoted.is_empty() { default.to_string() } else { unquoted.to_string() }
        }
        None => default.to_string(),
    }
}

/// 翻译源。auto：配了 AI key 走 AI，否则走有道免费接口，接口失败再落词典。
enum Source {
    Ai,
    Web,
    Dict,
}

fn setting_bool(key: &str, default: bool) -> bool {
    match host::setting(key) {
        Some(v) => match v.trim() {
            "true" | "\"true\"" | "1" => true,
            "false" | "\"false\"" | "0" => false,
            _ => default,
        },
        None => default,
    }
}

fn resolve_source() -> Source {
    match setting_str("source", "auto").as_str() {
        "ai" => Source::Ai,
        "web" => Source::Web,
        "dict" => Source::Dict,
        // free：配了密钥也只用免费在线源，不产生 API 花费
        "free" => Source::Web,
        _ => {
            if host::secret_is_set(SECRET) {
                Source::Ai
            } else {
                Source::Web
            }
        }
    }
}

/// 缓存键的源标签。AI 模式下端点或模型不同结果就不同，一并纳入。
fn cache_tag() -> String {
    match resolve_source() {
        Source::Ai => format!(
            "ai:{}:{}",
            setting_str("endpoint", "https://api.deepseek.com"),
            setting_str("model", "deepseek-chat")
        ),
        Source::Web => "web".into(),
        // 词典随版本更新，缓存键带上版本，升级后不会回放旧词条
        Source::Dict => format!("dict:{}", env!("CARGO_PKG_VERSION")),
    }
}

/// 是否走 AI 源（变量名解释、中文命名走 AI 质量更好；否则本地策略）。
fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

fn offset_at(text: &str, line: u32, col_utf16: u32) -> Option<usize> {
    let mut byte_off = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i as u32 == line {
            let mut units = 0u32;
            for ch in l.chars() {
                if units >= col_utf16 {
                    return Some(byte_off);
                }
                units += ch.len_utf16() as u32;
                byte_off += ch.len_utf8();
            }
            return Some(byte_off);
        }
        byte_off += l.len() + 1;
    }
    None
}

/// 选中文本；无选区时取光标所在的整个单词（含中文连续段）。
/// 返回 (文本, 所在范围)：范围用于编辑器内的悬停提示。
fn grab_text(ctx: &Invocation) -> Result<(String, Range), Error> {
    let editor = ctx.editor().ok_or("没有打开的文件")?;
    let doc = editor.document()?;
    let sel = editor
        .selections()?
        .into_iter()
        .next()
        .ok_or("请把光标放到要翻译的词上，或选中一段文本")?;
    if sel.start != sel.end {
        let text = doc.text_in(sel).ok_or("选区无效")?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("选中的是空白文本".into());
        }
        return Ok((trimmed.to_string(), sel));
    }
    let off = offset_at(&doc.text, sel.start.line, sel.start.character).ok_or("光标位置无效")?;
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut start = off;
    while let Some(c) = doc.text[..start].chars().next_back() {
        if is_word(c) {
            start -= c.len_utf8();
        } else {
            break;
        }
    }
    let mut end = off;
    while let Some(c) = doc.text[end..].chars().next() {
        if is_word(c) {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    if start == end {
        return Err("光标不在单词上。选中一段文本，或把光标放到词中间再试".into());
    }
    Ok((doc.text[start..end].to_string(), range_at(&doc.text, start, end)))
}

/// 字节偏移 → (行, UTF-16 列)。
fn range_at(text: &str, start: usize, end: usize) -> Range {
    let mut pos = Position::new(0, 0);
    for ch in text[..start].chars() {
        if ch == '\n' {
            pos = Position::new(pos.line + 1, 0);
        } else {
            pos = Position::new(pos.line, pos.character + ch.len_utf16() as u32);
        }
    }
    let mut end_pos = pos;
    for ch in text[start..end].chars() {
        end_pos = Position::new(end_pos.line, end_pos.character + ch.len_utf16() as u32);
    }
    Range::new(pos, end_pos)
}

fn cache_key(prefix: &str, text: &str, to_zh: bool) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in format!("{prefix}\u{1}{text}\u{1}{to_zh}").bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("transpal:{prefix}:{hash:x}")
}

/// 最近一次翻译结果常驻状态栏：通知可能被用户的通知设置整组关闭，
/// 状态栏不受影响，保证结果总有一个看得见的地方。
fn show_in_status_bar(text: &str) {
    const MAX: usize = 40;
    let short: String = if text.chars().count() > MAX {
        format!("{}…", text.chars().take(MAX).collect::<String>())
    } else {
        text.to_string()
    };
    host::set_status_item(
        "transpal.result",
        &StatusItem {
            text: format!("翻译：{short}"),
            tooltip: Some(format!("TransPal 最近翻译\n\n{text}")),
            command: None,
        },
    );
}

impl TransPal {
    fn translate_text(&mut self, text: &str, to_zh: bool) -> Result<String, Error> {
        // 缓存键带翻译源（AI 还带 endpoint/model）：换源后同一文本应重新请求，
        // 否则 dict 模式留下的结果会被 AI 模式原样复用
        let key = cache_key(&format!("t:{}", cache_tag()), text, to_zh);
        if let Ok(Some(hit)) = host::storage::get(&key) {
            self.show_results(None, text, &hit);
            return Ok(hit);
        }
        let (out, via) = match resolve_source() {
            Source::Ai => (ai::translate(text, to_zh)?, "AI"),
            Source::Web => match self.web_translate(text, to_zh) {
                Ok((out, via)) => (out, via),
                // 词典的报错带「去哪里配置」的指引，比裸网络错误更有用
                Err(_) => (dict::translate(text, to_zh)?, "词典"),
            },
            Source::Dict => (dict::translate(text, to_zh)?, "词典"),
        };
        let _ = host::storage::set(&key, &out);
        self.show_results(Some(via), text, &out);
        Ok(out)
    }

    /// 结果三处展示：通知（可能被用户设置吞掉）、状态栏（常驻）、停靠面板（历史）。
    /// `via` 为 None 表示缓存命中：不重复记历史，只把已有历史推给面板。
    fn show_results(&self, via: Option<&str>, text: &str, out: &str) {
        show_in_status_bar(out);
        let history = match via {
            Some(via) => {
                let mut history: Vec<serde_json::Value> = host::storage::get(HISTORY_KEY)
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                history.insert(0, serde_json::json!({ "text": text, "out": out, "via": via }));
                history.truncate(HISTORY_MAX);
                let _ = host::storage::set(HISTORY_KEY, &serde_json::to_string(&history).unwrap_or_default());
                history
            }
            None => host::storage::get(HISTORY_KEY)
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
        };
        let _ = host::post_view_message(VIEW, &serde_json::to_string(&history).unwrap_or_default());
    }

    /// 在线源池调度：粘性偏好 + 连续失败熔断。全部不可用时上层落词典。
    /// 成功时同时返回来源标签（有道 / 谷歌），供历史记录标注。
    fn web_translate(&mut self, text: &str, to_zh: bool) -> Result<(String, &'static str), Error> {
        const MAX_FAILURES: u8 = 2;
        let first = self.preferred.unwrap_or(web::WebSrc::Youdao);
        let order = [first, first.other()];
        let mut last_err: Option<Error> = None;
        for src in order {
            let idx = src.index();
            if self.failures[idx] >= MAX_FAILURES {
                continue;
            }
            match src.translate(text, to_zh) {
                Ok(out) => {
                    self.failures[idx] = 0;
                    self.preferred = Some(src);
                    return Ok((out, src.label()));
                }
                Err(e) => {
                    self.failures[idx] = self.failures[idx].saturating_add(1);
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| "在线翻译源暂时都不可用".into()))
    }

    /// 中文命名：先试在线源整句英译再转驼峰，接口不可用或结果不干净时落词典。
    fn web_or_dict_camel(&mut self, zh: &str) -> Result<String, Error> {
        if let Ok((en, _)) = self.web_translate(zh, false) {
            let words = naming::split_identifier(&en);
            if !words.is_empty() && words.iter().all(|w| w.is_ascii()) {
                return Ok(dict::camel_case(&words));
            }
        }
        dict::to_camel(zh)
    }
}

fn notify_info(title: &str, body: &str) {
    host::notify(NotificationLevel::Info, title, body);
}

impl TransPal {
    fn translate(&mut self, ctx: &Invocation) -> Result<Option<String>, Error> {
        let (text, range) = grab_text(ctx)?;
        let to_zh = !has_cjk(&text);
        host::log(
            LogLevel::Info,
            &format!("取词：{text:?}（{}字，方向 {}）", text.chars().count(), if to_zh { "译中" } else { "译英" }),
        );
        let out = self.translate_text(&text, to_zh)?;
        notify_info("TransPal 翻译", &out);
        self.show_hover(ctx, range, &out);
        Ok(None)
    }

    /// 悬停提示：把译文作为 Hint 级诊断放在原文上，鼠标停在词上即见浮层。
    /// 这是唯一贴着原文的展示位；通知可能被用户的通知设置整组关闭。
    fn show_hover(&mut self, ctx: &Invocation, range: Range, out: &str) {
        if !setting_bool("hover", true) {
            return;
        }
        let Some(editor) = ctx.editor() else { return };
        let path = editor.path();
        if let Some(old) = self.hover_path.take() {
            if old != path {
                let _ = host::publish_diagnostics(&old, &[]);
            }
        }
        let diag = Diagnostic::new(range, Severity::Hint, format!("TransPal：{out}"));
        if host::publish_diagnostics(&path, &[diag]).is_ok() {
            self.hover_path = Some(path);
        }
    }

    fn name_explain(&mut self, ctx: &Invocation) -> Result<Option<String>, Error> {
        let (text, _) = grab_text(ctx)?;
        let out = match resolve_source() {
            Source::Ai => ai::chat_text(&format!(
                "解释这个编程标识符的含义：先拆词，再逐词给出中文，最后一句话说明整体含义：\n{text}"
            ))?,
            Source::Web | Source::Dict => {
                let words = naming::split_identifier(&text);
                let zh: Vec<String> = words
                    .iter()
                    .map(|w| dict::word_zh(w).unwrap_or(w.as_str()).to_string())
                    .collect();
                zh.join(" ")
            }
        };
        notify_info("TransPal 变量名", &out);
        Ok(None)
    }

    fn name_from_chinese(&mut self, ctx: &Invocation) -> Result<Option<String>, Error> {
        let editor = ctx.editor().ok_or("没有打开的文件")?;
        let doc = editor.document()?;
        let sel = editor
            .selections()?
            .into_iter()
            .find(|s| s.start != s.end)
            .ok_or("请先选中要命名的中文文本")?;
        let text = doc.text_in(sel).ok_or("选区无效")?.trim().to_string();
        if !has_cjk(&text) {
            return Err("请选中中文文本再执行中文命名".into());
        }
        let camel = match resolve_source() {
            Source::Ai => ai::chat_text(&format!(
                "把下面的中文术语翻译成英文，作为编程标识符。只回复 camelCase 形式的标识符本身，不要任何其他内容：\n{text}"
            ))?,
            // dict 模式承诺零网络：本地词典兜不住就报错指引，绝不触网
            Source::Web => self.web_or_dict_camel(&text)?,
            Source::Dict => dict::to_camel(&text)?,
        };
        let snake = naming::camel_to_snake(&camel);
        editor.apply_edits(doc.version, &[TextEdit { range: sel, text: camel.clone() }])?;
        notify_info("TransPal 中文命名", &format!("{camel}（snake_case：{snake}）"));
        Ok(None)
    }
}

impl Plugin for TransPal {
    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        _arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        let editor_info = ctx.editor().map(|e| format!("{} ({})", e.path(), e.language()));
        host::log(
            LogLevel::Info,
            &format!("命令 {id} 进入：编辑器 {editor_info:?}"),
        );
        let out = match id {
            "translate" => self.translate(ctx),
            "name-explain" => self.name_explain(ctx),
            "name-from-chinese" => self.name_from_chinese(ctx),
            _ => Err(Error::NotFound(format!("command {id}"))),
        };
        match &out {
            Ok(Some(s)) => host::log(LogLevel::Info, &format!("命令 {id} 完成：{s}")),
            Ok(None) => host::log(LogLevel::Info, &format!("命令 {id} 完成（通知/编辑已发出）")),
            Err(e) => host::log(LogLevel::Warn, &format!("命令 {id} 失败：{e}")),
        }
        out
    }
}

register_plugin!(TransPal);

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::HttpResponse;
    use openidea_plugin::mock::MockHost;

    fn canned(content: &str) -> Vec<u8> {
        serde_json::json!({"choices":[{"message":{"content":content}}]}).to_string().into_bytes()
    }

    #[test]
    fn translates_selection_with_dict() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let cache = Cache::new();", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].title, "TransPal 翻译");
        assert_eq!(host.notifications()[0].body, "缓存");
    }

    #[test]
    fn translates_word_under_cursor_without_selection() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let balance = 1;", &[(0, 6, 0, 6)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "余额");
    }

    #[test]
    fn translates_snake_error_message_via_dict() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "poisoned_mutex", &[(0, 0, 0, 14)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "中毒 互斥锁");
    }

    #[test]
    fn dict_mode_matches_documented_table() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "editor.write", "storage"]);
        host.set_setting("source", "\"dict\"");

        for (input, expected) in [
            ("stringBuffer", "字符串 缓冲区"),
            ("ctx", "上下文"),
            ("values", "值"),
            ("user-agent", "用户代理"),
            ("user_agent", "用户代理"),
            ("matrix", "矩阵"),
            ("please report the error", "请 报告 那 错误"),
            ("canary release", "灰度发布"),
        ] {
            let actual = plugin.translate_text(input, true).unwrap();
            println!("{input:>26} => {actual}");
            assert_eq!(actual, expected, "{input}");
        }

        host.open_editor("/project/a.rs", "rust", "getUserById", &[(0, 0, 0, 11)]);
        plugin.run_command("name-explain", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "获取 用户 按 标识");

        host.open_editor("/project/a.rs", "rust", "let 缓存穿透 = 0;", &[(0, 4, 0, 8)]);
        plugin.run_command("name-from-chinese", &host.invocation(), None).unwrap();
        assert_eq!(host.editor_text(), "let cachePenetration = 0;");
    }

    #[test]
    fn dict_translate_reports_missing_word() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "frobnicate", &[(0, 0, 0, 10)]);
        let err = plugin.run_command("translate", &host.invocation(), None).unwrap_err();
        assert!(err.to_string().contains("离线词典没有收录"));
    }

    #[test]
    fn explains_identifier_with_dict() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "user_account_balance", &[(0, 0, 0, 20)]);
        plugin.run_command("name-explain", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "用户 账号 余额");
    }

    #[test]
    fn chinese_selection_becomes_camel_case() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "editor.write", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let 用户余额 = 0;", &[(0, 4, 0, 8)]);
        plugin.run_command("name-from-chinese", &host.invocation(), None).unwrap();
        assert_eq!(host.editor_text(), "let userBalance = 0;");
        assert!(host.notifications()[0].body.contains("user_balance"));
    }

    #[test]
    fn chinese_naming_reports_unknown_words() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "editor.write", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let 魑魅魍魉 = 0;", &[(0, 4, 0, 8)]);
        let err = plugin.run_command("name-from-chinese", &host.invocation(), None).unwrap_err();
        assert!(err.to_string().contains("词典无法翻译"));
        assert_eq!(host.editor_text(), "let 魑魅魍魉 = 0;");
    }

    #[test]
    fn ai_source_uses_secret_and_endpoint() {
        let mut plugin = TransPal::default();
        let host = MockHost::install()
            .with_capabilities(&["editor.read", "storage", "net:api.deepseek.com", "secrets"]);
        host.set_secret("deepseek-key", "sk-test");
        host.set_setting("source", "\"ai\"");
        host.on_fetch(|r| {
            assert_eq!(r.url, "https://api.deepseek.com/v1/chat/completions");
            assert_eq!(r.headers[1].0, "Authorization");
            assert_eq!(r.headers[1].1, "Bearer sk-test");
            Ok(HttpResponse { status: 200, headers: vec![], body: canned("余额") })
        });
        host.open_editor("/project/a.rs", "rust", "balance", &[(0, 0, 0, 7)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "余额");
    }

    #[test]
    fn hover_hint_published_on_translated_word() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "project.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let cache = 0;", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        let diags = host.diagnostics();
        let items = diags.get("/project/a.rs").expect("翻译后应发布悬停提示");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, Severity::Hint);
        assert_eq!(items[0].message, "TransPal：缓存");
        assert_eq!((items[0].range.start.line, items[0].range.start.character), (0, 4));
        assert_eq!((items[0].range.end.line, items[0].range.end.character), (0, 9));
    }

    #[test]
    fn hover_can_be_turned_off() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "project.read", "storage"]);
        host.set_setting("hover", "false");
        host.open_editor("/project/a.rs", "rust", "let cache = 0;", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert!(host.diagnostics().is_empty());
    }

    #[test]
    fn translation_shows_in_status_bar() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let cache = Cache::new();", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        let items = host.status_items();
        let item = items.get("transpal.result").expect("翻译结果应写入状态栏");
        assert_eq!(item.text, "翻译：缓存");
    }

    #[test]
    fn translation_pushes_to_view_panel_with_history() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let cache = 0;", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        host.open_editor("/project/a.rs", "rust", "let lock = 0;", &[(0, 4, 0, 8)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        let messages = host.view_messages();
        let last = messages.last().expect("翻译结果应推送到面板");
        assert_eq!(last.0, "results");
        let entries: Vec<serde_json::Value> = serde_json::from_str(&last.1).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["text"], "lock");
        assert_eq!(entries[0]["out"], "锁");
        assert_eq!(entries[1]["text"], "cache");
        assert!(entries[0]["via"].is_string());
    }

    #[test]
    fn cached_hit_reposts_history_without_duplicates() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let cache = 0;", &[(0, 4, 0, 9)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        let messages = host.view_messages();
        let last = messages.last().unwrap();
        let entries: Vec<serde_json::Value> = serde_json::from_str(&last.1).unwrap();
        assert_eq!(entries.len(), 1, "缓存命中不应重复记历史");
    }

    #[test]
    fn free_mode_ignores_secret_and_uses_web() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&[
            "editor.read",
            "storage",
            "net:api.deepseek.com",
            "net:aidemo.youdao.com",
            "secrets",
        ]);
        host.set_secret("deepseek-key", "sk-test");
        host.set_setting("source", "\"free\"");
        host.on_fetch(|r| {
            assert_eq!(r.url, "https://aidemo.youdao.com/trans");
            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: serde_json::json!({"errorCode":"0","translation":["余额"]}).to_string().into_bytes(),
            })
        });
        host.open_editor("/project/a.rs", "rust", "balance", &[(0, 0, 0, 7)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "余额");
    }

    #[test]
    fn auto_mode_web_denied_falls_back_to_dict() {
        let mut plugin = TransPal::default();
        let host = MockHost::install()
            .with_capabilities(&["editor.read", "storage", "net:api.deepseek.com", "secrets"]);
        host.on_fetch(|_| panic!("无 net:aidemo.youdao.com 权限应在 fetch 前被拒，不应触达 mock"));
        host.open_editor("/project/a.rs", "rust", "cache", &[(0, 0, 0, 5)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "缓存");
    }

    #[test]
    fn auto_mode_without_key_translates_sentences_via_web() {
        let mut plugin = TransPal::default();
        let host =
            MockHost::install().with_capabilities(&["editor.read", "storage", "net:aidemo.youdao.com"]);
        host.on_fetch(|r| {
            assert_eq!(r.url, "https://aidemo.youdao.com/trans");
            let body = String::from_utf8(r.body.clone().unwrap()).unwrap();
            assert!(body.starts_with("q=failed+to+acquire+lock&from=Auto&to=zh-CHS"));
            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: serde_json::json!({
                    "errorCode": "0",
                    "translation": ["获取锁失败", "：互斥锁已中毒"]
                })
                .to_string()
                .into_bytes(),
            })
        });
        host.open_editor("/project/a.rs", "rust", "failed to acquire lock", &[(0, 0, 0, 22)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "获取锁失败：互斥锁已中毒");
    }

    #[test]
    fn chinese_naming_prefers_web_translation() {
        let mut plugin = TransPal::default();
        let host = MockHost::install()
            .with_capabilities(&["editor.read", "editor.write", "storage", "net:aidemo.youdao.com"]);
        host.on_fetch(|_| {
            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: serde_json::json!({
                    "errorCode": "0",
                    "translation": ["user balance"]
                })
                .to_string()
                .into_bytes(),
            })
        });
        host.open_editor("/project/a.rs", "rust", "let 用户余额 = 0;", &[(0, 4, 0, 8)]);
        plugin.run_command("name-from-chinese", &host.invocation(), None).unwrap();
        assert_eq!(host.editor_text(), "let userBalance = 0;");
    }

    #[test]
    fn youdao_failure_fails_over_to_google_and_sticks() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&[
            "editor.read",
            "storage",
            "net:aidemo.youdao.com",
            "net:translate.googleapis.com",
        ]);
        let urls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = urls.clone();
        host.on_fetch(move |r| {
            log.borrow_mut().push(r.url.clone());
            if r.url.starts_with("https://aidemo.youdao.com") {
                Ok(HttpResponse { status: 500, headers: vec![], body: b"down".to_vec() })
            } else {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![],
                    body: serde_json::json!([[["你好 人类", "hello world"]]]).to_string().into_bytes(),
                })
            }
        });
        host.open_editor("/project/a.rs", "rust", "hello world", &[(0, 0, 0, 11)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[0].body, "你好 人类");

        // 换一个词再翻（绕开缓存）：粘性偏好应直接走谷歌，不再碰有道
        host.open_editor("/project/a.rs", "rust", "good morning", &[(0, 0, 0, 12)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[1].body, "你好 人类");
        let urls = urls.borrow().clone();
        assert!(urls[0].starts_with("https://aidemo.youdao.com"), "{urls:?}");
        assert!(urls[1].starts_with("https://translate.googleapis.com"), "{urls:?}");
        assert!(urls[2].starts_with("https://translate.googleapis.com"), "{urls:?}");
    }

    #[test]
    fn circuit_breaker_skips_dead_sources() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&[
            "editor.read",
            "storage",
            "net:aidemo.youdao.com",
            "net:translate.googleapis.com",
        ]);
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = calls.clone();
        host.on_fetch(move |_| {
            counter.set(counter.get() + 1);
            Ok(HttpResponse { status: 500, headers: vec![], body: b"down".to_vec() })
        });
        // 前两次：每条源各失败一次，最终落词典
        for (word, zh) in [("cache", "缓存"), ("balance", "余额"), ("lock", "锁")] {
            let text = format!("let x = \"{word}\";");
            let start = 9;
            host.open_editor("/project/a.rs", "rust", &text, &[(0, start, 0, start + word.len() as u32)]);
            plugin.run_command("translate", &host.invocation(), None).unwrap();
            assert_eq!(host.notifications().last().unwrap().body, zh);
        }
        // 前两次各打满 4 个请求（2 源 × 1 词）；第三次起熔断生效，不再发请求
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn cache_hits_storage_before_network() {
        let mut plugin = TransPal::default();
        let host = MockHost::install()
            .with_capabilities(&["editor.read", "storage", "net:api.deepseek.com", "secrets"]);
        host.set_secret("deepseek-key", "sk-test");
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = calls.clone();
        host.on_fetch(move |_| {
            counter.set(counter.get() + 1);
            Ok(HttpResponse { status: 200, headers: vec![], body: canned("缓存") })
        });
        host.open_editor("/project/a.rs", "rust", "cache", &[(0, 0, 0, 5)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(calls.get(), 1, "第二次应命中缓存");
    }

    #[test]
    fn editor_write_denied_degrades_with_error() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&["editor.read", "storage"]);
        host.open_editor("/project/a.rs", "rust", "let 余额 = 0;", &[(0, 4, 0, 6)]);
        let err = plugin.run_command("name-from-chinese", &host.invocation(), None).unwrap_err();
        assert_eq!(err, Error::Denied("editor.write".into()));
    }

    #[test]
    fn dict_mode_naming_makes_no_network_requests() {
        let mut plugin = TransPal::default();
        // 网络权限全部授予：dict 模式的承诺是不用，而不是用不了
        let host = MockHost::install().with_capabilities(&[
            "editor.read",
            "editor.write",
            "storage",
            "net:aidemo.youdao.com",
            "net:api.deepseek.com",
            "secrets",
        ]);
        host.set_secret("deepseek-key", "sk-test");
        host.set_setting("source", "\"dict\"");
        host.on_fetch(|r| panic!("dict 模式不应发起网络请求：{}", r.url));
        host.open_editor("/project/a.rs", "rust", "let 余额 = 0;", &[(0, 4, 0, 6)]);
        match plugin.run_command("name-from-chinese", &host.invocation(), None) {
            Ok(_) => assert_eq!(host.editor_text(), "let balance = 0;"),
            Err(e) => assert!(e.to_string().contains("词典"), "报错应来自词典而非网络：{e}"),
        }
        assert!(host.requests().is_empty(), "零网络请求");
    }

    #[test]
    fn cache_respects_source_switch() {
        let mut plugin = TransPal::default();
        let host = MockHost::install().with_capabilities(&[
            "editor.read",
            "storage",
            "net:aidemo.youdao.com",
            "net:api.deepseek.com",
            "secrets",
        ]);
        host.set_secret("deepseek-key", "sk-test");
        let ai_calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = ai_calls.clone();

        // dict 模式先翻译：写入词典缓存
        host.set_setting("source", "\"dict\"");
        host.open_editor("/project/a.rs", "rust", "cache", &[(0, 0, 0, 5)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();

        // 切到 ai 模式再翻译同一文本：必须重新请求，不得复用词典缓存
        host.set_setting("source", "\"ai\"");
        host.on_fetch(move |r| {
            assert!(r.url.starts_with("https://api.deepseek.com"), "应调 AI 而非在线源：{}", r.url);
            counter.set(counter.get() + 1);
            Ok(HttpResponse { status: 200, headers: vec![], body: canned("缓存（AI 译文）") })
        });
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(ai_calls.get(), 1, "换源后应重新请求");
        assert_eq!(host.notifications().last().unwrap().body, "缓存（AI 译文）");
    }

    #[test]
    fn cache_respects_model_switch() {
        let mut plugin = TransPal::default();
        let host = MockHost::install()
            .with_capabilities(&["editor.read", "storage", "net:api.deepseek.com", "secrets"]);
        host.set_secret("deepseek-key", "sk-test");
        host.set_setting("source", "\"ai\"");
        let ai_calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = ai_calls.clone();
        host.on_fetch(move |_| {
            counter.set(counter.get() + 1);
            Ok(HttpResponse { status: 200, headers: vec![], body: canned("AI 译文") })
        });
        host.open_editor("/project/a.rs", "rust", "cache", &[(0, 0, 0, 5)]);
        plugin.run_command("translate", &host.invocation(), None).unwrap();

        host.set_setting("model", "\"deepseek-reasoner\"");
        plugin.run_command("translate", &host.invocation(), None).unwrap();
        assert_eq!(ai_calls.get(), 2, "换模型后同一文本应重新请求");
    }
}
