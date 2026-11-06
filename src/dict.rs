//! 离线翻译兜底，三层结构：
//! 1. 人工精选的编程术语对（PAIRS）——命名分词与常见 CS 词优先
//! 2. ECDICT 开源词典数据（MIT，github.com/skywind3000/ECDICT）——按词频提取的高频词条
//! 3. 都查不到时报错并指引配置在线源

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
    ("migration", "迁移"),
    ("migrate", "迁移"),
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

fn ec_dict() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| ECDICT_TSV.lines().filter_map(|l| l.split_once('\t')).collect())
}

fn lookup_en_zh(en: &str) -> Option<&'static str> {
    let key = en.trim().to_lowercase();
    if let Some((_, z)) = PAIRS.iter().find(|(e, _)| e.eq_ignore_ascii_case(&key)) {
        return Some(z);
    }
    ec_dict().get(key.as_str()).copied()
}

fn lookup_zh_en(zh: &str) -> Option<&'static str> {
    PAIRS.iter().find(|(_, z)| *z == zh).map(|(e, _)| *e)
}

pub fn word_zh(en: &str) -> Option<&'static str> {
    lookup_en_zh(en)
}

pub fn translate(text: &str, to_zh: bool) -> Result<String, Error> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("没有可翻译的文本".into());
    }
    // 整词精确命中
    if to_zh {
        if let Some(zh) = lookup_en_zh(trimmed) {
            return Ok(zh.to_string());
        }
    } else if let Some(en) = lookup_zh_en(trimmed) {
        return Ok(en.to_string());
    }
    // 按分隔符 / 驼峰拆词逐个翻译（变量名、蛇形命名的报错等）
    let words = crate::naming::split_identifier(trimmed);
    if words.len() > 1 {
        let translated: Option<Vec<&str>> = words
            .iter()
            .map(|w| if to_zh { lookup_en_zh(w) } else { lookup_zh_en(w) })
            .collect();
        if let Some(parts) = translated {
            return Ok(parts.join(" "));
        }
    }
    Err(if to_zh {
        "词典里没有这个词。在 设置 › TransPal 配置接口地址，并在密钥中保存 AI 翻译 API Key 即可启用 AI 翻译".to_string()
    } else {
        "词典里没有这个词。配置 AI 翻译源即可翻译中文".to_string()
    }
    .into())
}

/// 中文 → camelCase。贪心最长匹配切词，任一段无法翻译即报错。
pub fn to_camel(zh: &str) -> Result<String, Error> {
    let chars: Vec<char> = zh.chars().filter(|c| !c.is_whitespace()).collect();
    let mut words: Vec<&'static str> = Vec::new();
    let mut unknown: Vec<char> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut matched = false;
        for len in (1..=4).rev() {
            if i + len > chars.len() {
                continue;
            }
            let seg: String = chars[i..i + len].iter().collect();
            if let Some(en) = lookup_zh_en(&seg) {
                words.push(en);
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
    Ok(camel_case(&words.iter().map(|s| s.to_string()).collect::<Vec<_>>()))
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
