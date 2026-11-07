//! 离线翻译兜底，四层结构：
//! 1. 人工精选的编程术语对（PAIRS）与 scripts/modern_terms.tsv（云原生/界面/运营/AI 新词）
//! 2. ECDICT 开源词典数据（MIT，github.com/skywind3000/ECDICT）——按词频提取的高频词条
//! 3. 通用表查不到时，再查「整条释义完全相等」的兜底表（同样来自 ECDICT）
//! 4. 中文最长匹配分词 + 逐段翻译，未收录的片段原样保留；一段都译不出才报错

use std::collections::HashMap;
use std::sync::OnceLock;

use openidea_plugin::Error;

const PAIRS: &[(&str, &str)] = &[
    ("variable", "变量"),
    ("function", "函数"),
    ("constant", "常量"),
    ("interface", "接口"),
    ("class", "类"),
    ("method", "方法"),
    ("object", "对象"),
    ("array", "数组"),
    ("list", "列表"),
    ("map", "映射"),
    ("set", "集合"),
    ("queue", "队列"),
    ("stack", "栈"),
    ("heap", "堆"),
    ("tree", "树"),
    ("graph", "图"),
    ("node", "节点"),
    ("pointer", "指针"),
    ("reference", "引用"),
    ("value", "值"),
    ("key", "键"),
    ("field", "字段"),
    ("property", "属性"),
    ("attribute", "属性"),
    ("parameter", "参数"),
    ("argument", "实参"),
    ("return", "返回"),
    ("thread", "线程"),
    ("process", "进程"),
    ("lock", "锁"),
    ("mutex", "互斥锁"),
    ("acquire", "获取"),
    ("release", "释放"),
    ("poisoned", "中毒"),
    ("cache", "缓存"),
    ("buffer", "缓冲区"),
    ("memory", "内存"),
    ("storage", "存储"),
    ("database", "数据库"),
    ("query", "查询"),
    ("index", "索引"),
    ("schema", "模式"),
    ("table", "表"),
    ("row", "行"),
    ("column", "列"),
    ("transaction", "事务"),
    ("commit", "提交"),
    ("rollback", "回滚"),
    ("migrate", "迁移"),
    ("migration", "迁移"),
    ("request", "请求"),
    ("response", "响应"),
    ("header", "请求头"),
    ("body", "正文"),
    ("token", "令牌"),
    ("session", "会话"),
    ("timeout", "超时"),
    ("retry", "重试"),
    ("failed", "失败"),
    ("error", "错误"),
    ("exception", "异常"),
    ("panic", "崩溃"),
    ("warning", "警告"),
    ("debug", "调试"),
    ("log", "日志"),
    ("parse", "解析"),
    ("serialize", "序列化"),
    ("deserialize", "反序列化"),
    ("encode", "编码"),
    ("decode", "解码"),
    ("encrypt", "加密"),
    ("decrypt", "解密"),
    ("hash", "哈希"),
    ("sign", "签名"),
    ("verify", "校验"),
    ("validate", "验证"),
    ("format", "格式化"),
    ("render", "渲染"),
    ("draw", "绘制"),
    ("layout", "布局"),
    ("widget", "组件"),
    ("button", "按钮"),
    ("window", "窗口"),
    ("page", "页面"),
    ("view", "视图"),
    ("model", "模型"),
    ("controller", "控制器"),
    ("service", "服务"),
    ("component", "组件"),
    ("module", "模块"),
    ("package", "包"),
    ("library", "库"),
    ("framework", "框架"),
    ("plugin", "插件"),
    ("driver", "驱动"),
    ("kernel", "内核"),
    ("network", "网络"),
    ("socket", "套接字"),
    ("connection", "连接"),
    ("client", "客户端"),
    ("server", "服务器"),
    ("host", "主机"),
    ("port", "端口"),
    ("protocol", "协议"),
    ("deploy", "部署"),
    ("build", "构建"),
    ("compile", "编译"),
    ("link", "链接"),
    ("test", "测试"),
    ("benchmark", "基准测试"),
    ("profile", "性能分析"),
    ("optimize", "优化"),
    ("performance", "性能"),
    ("config", "配置"),
    ("configure", "配置"),
    ("settings", "设置"),
    ("environment", "环境"),
    ("path", "路径"),
    ("file", "文件"),
    ("directory", "目录"),
    ("folder", "目录"),
    ("document", "文档"),
    ("user", "用户"),
    ("account", "账号"),
    ("balance", "余额"),
    ("price", "价格"),
    ("order", "订单"),
    ("payment", "支付"),
    ("count", "计数"),
    ("total", "总数"),
    ("maximum", "最大值"),
    ("minimum", "最小值"),
    ("average", "平均值"),
    ("default", "默认"),
    ("current", "当前"),
    ("previous", "上一个"),
    ("next", "下一个"),
    ("first", "第一个"),
    ("last", "最后一个"),
    ("empty", "空"),
    ("null", "空"),
    ("invalid", "无效"),
    ("valid", "有效"),
    ("missing", "缺失"),
    ("duplicate", "重复"),
    ("permission", "权限"),
    ("password", "密码"),
    ("login", "登录"),
    ("logout", "注销"),
    ("register", "注册"),
    ("email", "邮箱"),
    ("address", "地址"),
    ("phone", "电话"),
    ("name", "名称"),
    ("title", "标题"),
    ("description", "描述"),
    ("message", "消息"),
    ("content", "内容"),
    ("type", "类型"),
    ("status", "状态"),
    ("state", "状态"),
    ("event", "事件"),
    ("signal", "信号"),
    ("callback", "回调"),
    ("task", "任务"),
    ("job", "作业"),
    ("worker", "工作器"),
    ("schedule", "调度"),
    ("timer", "定时器"),
    ("delay", "延迟"),
    ("async", "异步"),
    ("sync", "同步"),
    ("await", "等待"),
    ("stream", "流"),
    ("channel", "通道"),
    ("iterator", "迭代器"),
    ("generator", "生成器"),
    ("closure", "闭包"),
    ("borrow", "借用"),
    ("lifetime", "生命周期"),
    ("trait", "特征"),
    ("enum", "枚举"),
    ("struct", "结构体"),
    ("implement", "实现"),
    ("inherit", "继承"),
    ("override", "重写"),
    ("overload", "重载"),
    ("generic", "泛型"),
    ("template", "模板"),
    ("macro", "宏"),
    ("import", "导入"),
    ("export", "导出"),
    ("namespace", "命名空间"),
    ("scope", "作用域"),
    ("global", "全局"),
    ("local", "局部"),
    ("static", "静态"),
    ("dynamic", "动态"),
    ("runtime", "运行时"),
    ("version", "版本"),
    ("branch", "分支"),
    ("merge", "合并"),
    ("conflict", "冲突"),
    ("patch", "补丁"),
    ("release", "发布"),
    ("tag", "标签"),
    ("repository", "仓库"),
    ("upstream", "上游"),
    ("downstream", "下游"),
    ("time", "时间"),
    ("date", "日期"),
    ("image", "图片"),
    ("video", "视频"),
    ("audio", "音频"),
    ("color", "颜色"),
    ("theme", "主题"),
    ("language", "语言"),
    ("currency", "货币"),
    ("amount", "金额"),
    ("quantity", "数量"),
    ("discount", "折扣"),
    ("coupon", "优惠券"),
    ("shipping", "配送"),
    ("inventory", "库存"),
    ("product", "商品"),
    ("cart", "购物车"),
    ("customer", "客户"),
    ("merchant", "商家"),
    ("comment", "评论"),
    ("article", "文章"),
    ("category", "分类"),
    ("search", "搜索"),
    ("filter", "过滤"),
    ("sort", "排序"),
    ("upload", "上传"),
    ("download", "下载"),
    ("preview", "预览"),
    ("detail", "详情"),
    ("summary", "摘要"),
    ("history", "历史"),
];

/// ECDICT 高频词条（scripts/build_dict.py 生成），word<TAB>释义。
const ECDICT_TSV: &str = include_str!("dict_data.tsv");
/// ECDICT 中→英反查表（scripts/build_zh_dict.py 生成），中文<TAB>英文。
const ZH_EN_TSV: &str = include_str!("dict_zh.tsv");
/// 人工整理的现代术语（scripts/modern_terms.tsv），优先级仅次于 PAIRS。
const MODERN_TSV: &str = include_str!("../scripts/modern_terms.tsv");
/// 整条释义完全相等的兜底对译（scripts/build_fallback_dict.py 生成）。
const FALLBACK_TSV: &str = include_str!("dict_fallback.tsv");
/// 代码上下文专用英中词表：只收 ECDICT 通用首义会译错的词。
const CODE_TSV: &str = include_str!("code_terms.tsv");

/// 分词最长匹配的窗口。通用/兜底表最多 4 字；人工术语表里有「上下文窗口」这类 5-6 字短语。
const MAX_SPAN: usize = 8;

fn tsv_map(
    src: &'static str,
    cache: &'static OnceLock<HashMap<&'static str, &'static str>>,
) -> &'static HashMap<&'static str, &'static str> {
    cache.get_or_init(|| {
        src.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split_once('\t'))
            .collect()
    })
}

fn ec_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    tsv_map(ECDICT_TSV, &MAP)
}

fn zh_en_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    tsv_map(ZH_EN_TSV, &MAP)
}

fn modern_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    tsv_map(MODERN_TSV, &MAP)
}

fn fallback_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    tsv_map(FALLBACK_TSV, &MAP)
}

fn code_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    tsv_map(CODE_TSV, &MAP)
}

/// 现代术语表反向索引（英文→中文）：同一英文有多个中文时取先出现的。
fn modern_rev() -> &'static HashMap<String, &'static str> {
    static MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for line in MODERN_TSV.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some((zh, en)) = line.split_once('\t') {
                map.entry(en.trim().to_lowercase()).or_insert(zh.trim());
            }
        }
        map
    })
}

fn pairs_en() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| PAIRS.iter().map(|(e, z)| (*e, *z)).collect())
}

fn pairs_zh() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for (e, z) in PAIRS {
            map.entry(*z).or_insert(*e);
        }
        map
    })
}

fn lookup_en_zh(en: &str) -> Option<&'static str> {
    let key = en.trim().to_lowercase();
    pairs_en()
        .get(key.as_str())
        .copied()
        .or_else(|| ec_dict().get(key.as_str()).copied())
}

/// 常见屈折后缀的原形候选（returns/configured/retrying）。
fn stems(word: &str) -> Vec<String> {
    let w = word.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    if let Some(s) = w.strip_suffix("ies").filter(|s| s.len() >= 3) {
        out.push(format!("{s}y"));
    }
    for suf in ["es", "s", "ed", "ing", "ly"] {
        if let Some(s) = w.strip_suffix(suf).filter(|s| s.len() >= 3) {
            out.push(s.to_string());
            if matches!(suf, "ed" | "ing") {
                out.push(format!("{s}e"));
            }
        }
    }
    out
}

/// 代码上下文的英文选义：PAIRS > 代码词表 > 现代术语反查 > ECDICT 通用首义。
/// 「string/char/float」这类词在通用词典里的首义是线/杂务/漂流物，
/// 标识符场景必须走这条链；散文句子仍走 lookup_en_zh 的通用义。
fn lookup_en_zh_code(en: &str) -> Option<&'static str> {
    let key = en.trim().to_lowercase();
    pairs_en()
        .get(key.as_str())
        .copied()
        .or_else(|| code_dict().get(key.as_str()).copied())
        // 复数/动词形态优先回 CS 词表的原形：标识符里的 values/fields 是「值/字段」，
        // 不该被 ECDICT 的单数同形词条（values→价值观、keys→关键帧）截走。
        .or_else(|| {
            stems(&key).iter().find_map(|s| {
                pairs_en()
                    .get(s.as_str())
                    .or_else(|| code_dict().get(s.as_str()))
                    .copied()
            })
        })
        .or_else(|| modern_rev().get(&key).copied())
        .or_else(|| ec_dict().get(key.as_str()).copied())
}

/// 多词术语键归一化：小写、连字符/下划线视同空格、压缩空白（user-agent = user_agent）。
fn phrase_key(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn en_phrase_dict() -> &'static HashMap<String, &'static str> {
    static MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: HashMap<String, &'static str> = HashMap::new();
        for (en, zh) in code_dict().iter() {
            if en.contains(' ') || en.contains('-') {
                map.entry(phrase_key(en)).or_insert(zh);
            }
        }
        for (en, zh) in modern_rev().iter() {
            if en.contains(' ') || en.contains('-') {
                map.entry(phrase_key(en)).or_insert(zh);
            }
        }
        map
    })
}

/// 多词术语的整句精确命中（canary release / status code / user-agent）。
/// 只查含空格或连字符的键：单个 report 在散文里该是「报告」，在代码里才是「报表」。
fn lookup_en_zh_phrase(en: &str) -> Option<&'static str> {
    let t = en.trim();
    if !t.contains(' ') && !t.contains('-') && !t.contains('_') {
        return None;
    }
    en_phrase_dict().get(&phrase_key(t)).copied()
}

fn lookup_zh_en(zh: &str) -> Option<&'static str> {
    pairs_zh()
        .get(zh)
        .copied()
        .or_else(|| modern_dict().get(zh).copied())
        .or_else(|| zh_en_dict().get(zh).copied())
        // 兜底表只在前两层都没收录时才查：ECDICT 的术语行缺少中文词边界，
        // 直接混进通用表会把「苹果」这类常用词带偏（实测苹果→APPLE computer）。
        .or_else(|| fallback_dict().get(zh).copied())
}

/// 英文先查原形，再试常见屈折后缀（returns/configured/retrying）。
/// 词表只有原形，整句逐词译时靠这一步避免「returns」原样留在中文里。
fn lookup_en_inflected(word: &str, code: bool) -> Option<&'static str> {
    let base = |w: &str| {
        if code {
            lookup_en_zh_code(w)
        } else {
            lookup_en_zh(w)
        }
    };
    if let Some(zh) = base(word) {
        return Some(zh);
    }
    stems(word).iter().find_map(|s| base(s))
}

pub fn word_zh(en: &str) -> Option<&'static str> {
    lookup_en_zh_code(en)
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn is_ascii_run(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '-' | '.' | '/' | '@' | '#' | '$' | '%' | '&' | '*' | '+' | '='
        )
}

/// 中文标点 → 英文标点；其余 ASCII 标点原样保留。
fn punct_en(c: char) -> Option<&'static str> {
    Some(match c {
        '，' | '、' => ",",
        '。' => ".",
        '？' => "?",
        '！' => "!",
        '：' => ":",
        '；' => ";",
        '（' => "(",
        '）' => ")",
        '“' | '”' => "\"",
        '‘' | '’' => "'",
        '…' => "...",
        '—' | '－' => "-",
        _ => return None,
    })
}

fn push_token(out: &mut String, tok: &str) {
    if matches!(tok, ")" | "," | "." | "?" | "!" | ":" | ";" | "，") {
        out.push_str(tok);
    } else if matches!(tok, "(" | "\"" | "'") {
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(tok);
    } else {
        if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') {
            out.push(' ');
        }
        out.push_str(tok);
    }
}

/// 标识符/路径形态：无空白、无中文、无句读，且至少含一个字母。
/// 命中即按代码上下文选义；散文句子（含空格或标点）保持通用义。
fn looks_like_identifier(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty()
        && !t.chars().any(|c| c.is_whitespace() || is_cjk(c))
        && t.chars().any(|c| c.is_ascii_alphabetic())
        && t.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '_' | '-'
                        | '.'
                        | '/'
                        | '\\'
                        | ':'
                        | '@'
                        | '#'
                        | '$'
                        | '%'
                        | '&'
                        | '*'
                        | '+'
                        | '='
                )
        })
}

/// 中文最长匹配分词后逐段译英；未收录片段原样保留。
/// 返回 (译文, 命中段数)：命中数为 0 时上层报错，不做整段留原文的假翻译。
fn zh_to_en(text: &str) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut hits = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_cjk(c) {
            let max = MAX_SPAN.min(chars.len() - i);
            let mut hit = None;
            for len in (1..=max).rev() {
                let span: String = chars[i..i + len].iter().collect();
                if let Some(en) = lookup_zh_en(&span) {
                    hit = Some((len, en));
                    break;
                }
            }
            match hit {
                Some((len, en)) => {
                    push_token(&mut out, en);
                    hits += 1;
                    i += len;
                }
                None => {
                    push_token(&mut out, &c.to_string());
                    i += 1;
                }
            }
        } else if let Some(p) = punct_en(c) {
            push_token(&mut out, p);
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if is_ascii_run(c) {
            let start = i;
            while i < chars.len() && is_ascii_run(chars[i]) {
                i += 1;
            }
            push_token(&mut out, &chars[start..i].iter().collect::<String>());
        } else {
            push_token(&mut out, &c.to_string());
            i += 1;
        }
    }
    (out.trim().to_string(), hits)
}

/// 英文按词切开逐词译中（取首义，避免整句堆满释义）；未收录的词原样保留。
/// `code` 决定选义链：标识符走代码词表，散文走通用首义。
fn en_to_zh(text: &str, code: bool) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut hits = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() || c == '\'' {
            let start = i;
            // 标识符里的数字属于同一个词（v2User、h264Decoder），交给 split_identifier 归类
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '\'' | '-'))
            {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            for w in crate::naming::split_identifier(&run) {
                match lookup_en_inflected(&w, code) {
                    Some(zh) => {
                        let first = zh
                            .split(|c| c == '；' || c == ';')
                            .next()
                            .unwrap_or(zh)
                            .trim();
                        push_token(&mut out, first);
                        hits += 1;
                    }
                    None => push_token(&mut out, &w),
                }
            }
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || matches!(chars[i], '.' | '%')) {
                i += 1;
            }
            push_token(&mut out, &chars[start..i].iter().collect::<String>());
        } else if c.is_whitespace() || c == '_' {
            i += 1;
        } else {
            push_token(&mut out, &c.to_string());
            i += 1;
        }
    }
    (out.trim().to_string(), hits)
}

pub fn translate(text: &str, to_zh: bool) -> Result<String, Error> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("没有可翻译的文本".into());
    }
    // 整词精确命中优先；整句则分词后逐段翻译，译不出的片段保留原文
    if to_zh {
        // 标识符（getUserById、Cargo.toml）按代码义项翻译；散文句子相反。
        // 多词 CS 术语（canary release）整句精确命中时两种上下文都优先。
        let code = looks_like_identifier(trimmed);
        let exact = if code {
            lookup_en_zh_code(trimmed)
        } else {
            lookup_en_zh(trimmed)
        };
        if let Some(zh) = exact.or_else(|| lookup_en_zh_phrase(trimmed)) {
            return Ok(zh.to_string());
        }
        let (out, hits) = en_to_zh(trimmed, code);
        if hits > 0 {
            return Ok(out);
        }
    } else {
        if let Some(en) = lookup_zh_en(trimmed) {
            return Ok(en.to_string());
        }
        let (out, hits) = zh_to_en(trimmed);
        if hits > 0 {
            return Ok(out);
        }
    }
    Err(if to_zh {
        "离线词典没有收录这段文本。在 设置 › TransPal 配置接口地址，并在密钥中保存 AI 翻译 API Key 即可启用 AI 翻译".to_string()
    } else {
        "离线词典没有收录这段文本。配置 AI 翻译源即可翻译中文".to_string()
    }
    .into())
}

/// 中文 → camelCase。贪心最长匹配切词，任一段无法翻译即报错。
pub fn to_camel(zh: &str) -> Result<String, Error> {
    let chars: Vec<char> = zh.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() {
        return Err("没有可翻译的文本".into());
    }
    let mut words: Vec<&'static str> = Vec::new();
    let mut unknown: Vec<char> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut matched = false;
        for len in (1..=MAX_SPAN).rev() {
            if i + len > chars.len() {
                continue;
            }
            let seg: String = chars[i..i + len].iter().collect();
            if let Some(en) = lookup_zh_en(&seg) {
                // 术语表里有多词译法（event tracking），命名时要拆开再拼 camelCase
                for part in en.split(|c: char| !c.is_ascii_alphanumeric()) {
                    if !part.is_empty() {
                        words.push(part);
                    }
                }
                i += len;
                matched = true;
                break;
            }
        }
        if !matched {
            unknown.push(chars[i]);
            i += 1;
        }
    }
    if !unknown.is_empty() {
        return Err(format!(
            "词典无法翻译「{}」。配置 AI 翻译源可获得完整支持",
            unknown.iter().collect::<String>()
        )
        .into());
    }
    Ok(camel_case(
        &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    ))
}

pub fn camel_case(words: &[String]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_lowercase();
        if i == 0 {
            out.push_str(&lower);
        } else {
            let mut cs = lower.chars();
            if let Some(first) = cs.next() {
                out.extend(first.to_uppercase());
                out.push_str(cs.as_str());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_cs_phrases_for_naming() {
        assert_eq!(
            translate("配置接口地址", false).unwrap(),
            "config interface address"
        );
        assert_eq!(translate("缓存失败", false).unwrap(), "cache failed");
        assert_eq!(to_camel("用户余额").unwrap(), "userBalance");
        assert_eq!(to_camel("配置接口地址").unwrap(), "configInterfaceAddress");
    }

    #[test]
    fn segments_general_chinese() {
        let out = translate("用户名", false).unwrap();
        assert_eq!(out, "user name");
        assert!(translate("网络，服务器", false).unwrap().contains(','));
    }

    #[test]
    fn partial_english_sentence_keeps_unknown_words() {
        let out = translate("failed to acquire lock", true).unwrap();
        assert_eq!(out, "失败 到 获取 锁");
        let out = translate("failed to xyzzy", true).unwrap();
        assert!(out.contains("失败") && out.contains("xyzzy"), "{out}");
    }

    #[test]
    fn unknown_only_text_still_errors() {
        assert!(translate("龘", false).is_err());
        assert!(translate("xyzzy", true).is_err());
    }

    #[test]
    fn modern_terms_beat_general_table() {
        assert_eq!(translate("埋点", false).unwrap(), "event tracking");
        assert_eq!(translate("日活", false).unwrap(), "daily active users");
        assert_eq!(translate("灰度发布", false).unwrap(), "canary release");
        assert_eq!(translate("集群", false).unwrap(), "cluster");
        assert_eq!(translate("磁盘", false).unwrap(), "disk");
        assert_eq!(translate("上下文窗口", false).unwrap(), "context window");
        assert_eq!(to_camel("缓存穿透").unwrap(), "cachePenetration");
    }

    #[test]
    fn fallback_table_covers_terms_missing_everywhere_else() {
        assert_eq!(translate("监听器", false).unwrap(), "listener");
        assert_eq!(translate("分频器", false).unwrap(), "frequency divider");
    }

    #[test]
    fn code_context_picks_computing_senses() {
        for (en, zh) in [
            ("string", "字符串"),
            ("char", "字符"),
            ("integer", "整数"),
            ("float", "浮点数"),
            ("matrix", "矩阵"),
            ("payload", "载荷"),
            ("listener", "监听器"),
            ("mime", "MIME 类型"),
            ("report", "报表"),
            ("dashboard", "仪表盘"),
        ] {
            assert_eq!(translate(en, true).unwrap(), zh, "{en}");
        }
        // 同一个词在散文里保留通用义，两条选义链不能合并
        assert_ne!(lookup_en_zh("string"), lookup_en_zh_code("string"));
    }

    #[test]
    fn plurals_and_suffixes_prefer_cs_stem() {
        // ECDICT 有 values/keys/fields 的单数同形词条，标识符里应先回到 CS 原形
        for (en, zh) in [
            ("returns", "返回"),
            ("caches", "缓存"),
            ("values", "值"),
            ("fields", "字段"),
            ("keys", "键"),
            ("interfaces", "接口"),
            ("configured", "配置"),
            ("retrying", "重试"),
        ] {
            assert_eq!(translate(en, true).unwrap(), zh, "{en}");
        }
    }

    #[test]
    fn identifiers_split_into_words() {
        assert_eq!(translate("stringBuffer", true).unwrap(), "字符串 缓冲区");
        assert_eq!(
            translate("userAccountBalance", true).unwrap(),
            "用户 账号 余额"
        );
        assert_eq!(translate("v2User", true).unwrap(), "v2 用户");
        assert_eq!(translate("getUserById", true).unwrap(), "获取 用户 按 标识");
        assert_eq!(translate("isValid", true).unwrap(), "是 有效");
        assert_eq!(translate("hasPermission", true).unwrap(), "有 权限");
        assert_eq!(translate("onClick", true).unwrap(), "在 点击");
        assert_eq!(word_zh("cache"), Some("缓存"));
    }

    #[test]
    fn multiword_phrases_use_curated_terms() {
        assert_eq!(translate("canary release", true).unwrap(), "灰度发布");
        assert_eq!(translate("rate limit", true).unwrap(), "限流");
        assert_eq!(translate("status code", true).unwrap(), "状态码");
        assert_eq!(
            translate("blue-green deployment", true).unwrap(),
            "蓝绿部署"
        );
        // 连字符与空格两种写法等价（标识符里更常见连字符）
        assert_eq!(translate("user-agent", true).unwrap(), "用户代理");
        assert_eq!(translate("user agent", true).unwrap(), "用户代理");
        assert_eq!(translate("user_agent", true).unwrap(), "用户代理");
    }

    #[test]
    fn prose_sentences_keep_general_senses() {
        // 含空格的整句不算标识符，report 不该被硬译成「报表」
        assert!(lookup_en_zh_phrase("report").is_none());
        let out = translate("please report the error", true).unwrap();
        assert!(out.contains("报告") && out.contains("错误"), "{out}");
    }

    #[test]
    fn empty_and_whitespace_input_errors() {
        for s in ["", "   ", "\n\t", "\u{3000}"] {
            assert!(translate(s, false).is_err(), "{s:?}");
            assert!(translate(s, true).is_err(), "{s:?}");
            assert!(to_camel(s).is_err(), "{s:?}");
        }
    }

    #[test]
    fn punctuation_and_digits_survive() {
        assert_eq!(
            translate("网络，服务器。", false).unwrap(),
            "network, server."
        );
        assert_eq!(
            translate("缓存命中率 99.9%", false).unwrap(),
            "cache hit rate 99.9%"
        );
        assert_eq!(
            translate("请求 API 失败", false).unwrap(),
            "request API failed"
        );
    }

    #[test]
    fn general_words_are_not_overridden_by_fallback_noise() {
        // 兜底表里 苹果 曾被 ECDICT 术语行带偏成 APPLE computer，前两层必须挡住
        assert_eq!(translate("苹果", false).unwrap(), "apple");
    }

    #[test]
    fn to_camel_reports_the_untranslatable_part() {
        assert_eq!(to_camel(" 用户 余额 ").unwrap(), "userBalance");
        let err = to_camel("缓存龘").unwrap_err().to_string();
        assert!(err.contains("龘"), "{err}");
        // 纯英文不是可命名的中文，应报错而不是原样返回
        assert!(to_camel("abc").is_err());
    }

    #[test]
    fn case_and_separator_variants_agree() {
        assert_eq!(translate("GET", true).unwrap(), "获取");
        assert_eq!(
            translate("get_user_by_id", true).unwrap(),
            "获取 用户 按 标识"
        );
        assert_eq!(translate("UserId", true).unwrap(), "用户 标识");
        assert_eq!(word_zh("definitely_not_a_word"), None);
    }

    #[test]
    fn detects_identifier_shapes() {
        for s in [
            "getUserById",
            "user_name",
            "Cargo.toml",
            "src/dict.rs",
            "v2User",
            "a",
            "@scope/pkg",
        ] {
            assert!(looks_like_identifier(s), "{s}");
        }
        for s in [
            "failed to acquire lock",
            "缓存",
            "hello,",
            "",
            "   ",
            "123",
            "你好world",
        ] {
            assert!(!looks_like_identifier(s), "{s}");
        }
    }
}
