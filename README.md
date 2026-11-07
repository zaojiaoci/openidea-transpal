# TransPal Translate

**TransPal Translate** is a free translation plugin for [OpenIDEA](https://github.com/l0ng-ai/openidea): translate the selection or the word at the cursor, explain identifiers in Chinese, and turn Chinese into camelCase names. It works out of the box with free web sources (Youdao / Google) plus an offline ECDICT dictionary, and upgrades to AI translation with any OpenAI-compatible key (DeepSeek, Kimi, local Ollama…). Results appear as a hover overlay, status bar item, balloon, and a dock panel. [中文说明见下](#功能)。

---

[OpenIDEA](https://github.com/l0ng-ai/openidea) 插件：选中即译、变量名中英互译、中文命名生成。
零配置可用（有道/谷歌免费接口 + 离线词典兜底），配置一个 OpenAI 兼容接口（DeepSeek / Kimi / 本地 Ollama…）后自动升级为 AI 翻译。

## 功能

| 命令 | 快捷键 | 作用 |
| --- | --- | --- |
| 翻译选中文本 | `Alt+T`（可在键映射里改绑） | 选中即译，自动判断中英方向；无选区时翻译光标所在单词 |
| 变量名译成中文 | — | `userAccountBalance` → 「用户 账号 余额」 |
| 中文命名 | — | 选中「用户余额」→ 原位替换为 `userBalance`，并给出 `user_balance` |

右键编辑器菜单也有这三个入口。翻译结果显示在四处：**鼠标停在刚翻译的词上看悬停浮层**（设置里 `hover` 可关）、右下角通知气泡、状态栏（「翻译：…」，悬停看全文）、右侧边条「翻译结果」停靠面板（保留最近 20 条，可一键复制，跨会话持久）。

## 翻译源（三档，自动降级）

1. **AI（可选，质量最佳）**：任何 OpenAI 兼容接口。默认 DeepSeek，也可换本地 Ollama。
   1. `设置 › TransPal` 确认接口地址和模型名；
   2. 首次使用时在弹窗中保存 API Key（或 `设置 › 密钥` 里填「AI 翻译 API Key」）。
   Key 只发送给你配置的那个接口域名（`net:api.deepseek.com` 权限）。
   **端点限制（IDE 权限系统）**：网络权限按域名静态声明，目前内置 `api.deepseek.com`（默认）和 `localhost`（本地 Ollama）。其他厂商的端点（Kimi、中转站等）会被 IDE 拒绝，这是 OpenIDEA 能力系统的边界，已向官方提议支持「用户自配端点的动态授权」。
2. **有道公共接口（默认，零配置）**：不填任何 key 就能整句翻译，国内直连。
3. **内置词典（离线兜底）**：8 万条高频词（来自开源项目 [ECDICT](https://github.com/skywind3000/ECDICT)，MIT License）+ 2 万条整条对译兜底表 + 约 400 条现代术语（云原生、界面、运营指标、AI）+ 约 350 条代码上下文词表。整句按最长匹配分词后逐段翻译，译不出的片段保留原文；毫秒级，网络接口失败或未授权联网时自动接管。

   英文→中文会区分上下文：标识符（`stringBuffer`、`user-agent`）走代码义项与常用缩写（`string`→字符串、`ctx`→上下文、`values`→值），散文句子仍用通用首义（`string`→线）。

设置里 `翻译源` 可强制：`free`（免费模式，配了密钥也绝不调 AI，零花费）、`ai`、`web`、`dict`。翻译结果按「源 + 文本 + 方向」本地缓存，重复词条不重复请求。

## 安装

```sh
cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
git clone https://github.com/zaojiaoci/openidea-transpal
cd openidea-transpal
openidea-plugin build   # → dist/plugin.wasm
openidea-plugin link    # 在 IDE 里：设置 › 插件 › 重新加载
```

收录进 OpenIDEA 社区市场后可直接在 IDE 内安装（待办）。

## 权限说明

- 必需：`editor.read`（读取选区）、`project.read`（悬停提示走诊断通道）、`net:aidemo.youdao.com` + `net:translate.googleapis.com`（免费整句翻译的两个源，启用时一次授权）
- 可选：`editor.write`（中文命名替换）、`net:api.deepseek.com`（AI 翻译）、`net:localhost`（本地 Ollama）、`secrets`（保存 API Key）、`storage`（翻译缓存和历史）

不给可选权限插件照样能用：免费接口被拒时自动落内置词典。翻译结果同时显示在状态栏（「翻译：…」，悬停看全文），通知被关闭也能看到。

## 开发

```sh
cargo test   # 逻辑测试走 SDK 的 mock host，无需 IDE

# 词典改动前后对比（before 用改动前的 dict_zh.tsv）
python3 scripts/bench_dict.py --before /path/to/old/dict_zh.tsv
python3 scripts/bench_dict.py --modern scripts/modern_terms.tsv --fallback src/dict_fallback.tsv

# 标识符英→中基准（代码上下文选义）
python3 scripts/bench_dict.py --ident
```

结构：

```
src/lib.rs            命令入口、选区/光标取词、缓存、翻译源调度、悬停/状态栏/面板展示
src/dict.rs           离线词典：编程术语对 + ECDICT 高频词（include_str! 嵌入）+ 双向分词
src/dict_data.tsv     ECDICT 提取的英→中高频词条（scripts/build_dict.py 生成）
src/dict_zh.tsv       ECDICT 反查的中→英词表（scripts/build_zh_dict.py 生成）
src/dict_fallback.tsv ECDICT 整条对译兜底表（scripts/build_fallback_dict.py 生成）
src/code_terms.tsv    代码上下文英→中词表（标识符义项与缩写）
scripts/modern_terms.tsv  人工整理的现代术语（中→英）
scripts/gold_terms.tsv    250 词中→英基准集
scripts/gold_ident.tsv    292 词标识符英→中基准集
scripts/bench_dict.py     词典命中率 before/after 对比
src/naming.rs         标识符拆词与命名风格转换
src/web.rs            有道 + 谷歌免费翻译接口（源池、故障转移）
src/ai.rs             OpenAI 兼容接口调用
web/index.html        「翻译结果」停靠面板
```

## License

MIT。词典数据来自 [ECDICT](https://github.com/skywind3000/ECDICT)（MIT License），构建脚本见 `scripts/build_dict.py`、`scripts/build_zh_dict.py` 与 `scripts/build_fallback_dict.py`。`build_fallback_dict.py` 在构建期用 [jieba](https://github.com/fxsjy/jieba) 的 `dict.txt`（MIT License）做中文词边界校验，该文件不随插件分发。
