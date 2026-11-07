#!/usr/bin/env python3
"""从 ECDICT 全量 CSV 生成中→英反查表（span<TAB>英文），供离线分词兜底。

数据来源：https://github.com/skywind3000/ECDICT （MIT License）
用法：python3 scripts/build_zh_dict.py <ecdict.csv> <src/dict_zh.tsv>

只收「释义里的中文片段」这一种证据，按词频与词级选最常用英文词：
多字词条 2-4 字（分词最长匹配用），单字取 2-4 字释义的首字（命名用）。
人工资讯术语 PAIRS 在运行时优先于本表，所以这里只管通用词覆盖。
"""

import csv
import re
import sys

CJK = re.compile(r"[\u4e00-\u9fff]+")
LEXWORD = re.compile(r"[A-Za-z][A-Za-z'\- ]{0,30}")
BRACKET = re.compile(r"[\[（(【<][^\[（(】\]>]{0,14}[\]）)】>]?")
SEP = re.compile(r"[；;，,、。\.\s/｜|…—\-\"'“”‘’:：?？!！]+")
MAX_SPAN = 4
MAX_ENTRIES = 60000

# 常用词人工修正：ECDICT 的反查会选到 promotion(创建)、preservation(保存)、
# scalar(数量) 这类同义冷僻词。PAIRS 已覆盖的 CS 术语不在这里重复。
HAND = {
    "过期": "expire", "创建": "create", "保存": "save", "加载": "load",
    "写入": "write", "读取": "read", "重启": "restart", "重试": "retry",
    "选中": "selection", "选区": "selection", "数量": "count",
    "高度": "height", "宽度": "width", "长度": "length", "包含": "contain",
    "成功": "success", "需要": "need", "支持": "support", "使用": "use",
    "删除": "delete", "添加": "add", "更新": "update", "连接": "connect",
}

# 单字只留人工判过的常用字：ECDICT 的首字证据会给「万→versatile」「个→character」
# 这类错配，分词兜底宁可留原字也不能乱译。
SINGLE = {
    "一": "one", "二": "two", "三": "three", "四": "four", "五": "five",
    "六": "six", "七": "seven", "八": "eight", "九": "nine", "十": "ten",
    "百": "hundred", "千": "thousand", "万": "ten thousand", "半": "half",
    "我": "I", "你": "you", "他": "he", "她": "she", "它": "it",
    "这": "this", "那": "that", "该": "this", "此": "this",
    "是": "be", "不": "not", "没": "not", "无": "none", "否": "not",
    "有": "have", "在": "at", "和": "and", "或": "or", "的": "of", "了": "already",
    "个": "one", "每": "each", "各": "each", "多": "many", "少": "few",
    "名": "name", "值": "value", "数": "number", "键": "key", "表": "table",
    "项": "item", "组": "group", "行": "row", "列": "column", "页": "page",
    "图": "image", "文": "text", "字": "word", "位": "bit", "级": "level",
    "率": "rate", "量": "amount", "额": "amount", "金": "money",
    "长": "length", "宽": "width", "高": "height", "大": "big", "小": "small",
    "新": "new", "旧": "old", "总": "total", "首": "first", "尾": "tail",
    "前": "previous", "后": "next", "左": "left", "右": "right",
    "内": "inner", "外": "outer", "开": "open", "关": "close",
    "加": "add", "减": "subtract", "乘": "multiply", "除": "divide",
    "分": "score", "秒": "second", "时": "time", "日": "day", "月": "month",
    "年": "year", "好": "good", "坏": "bad", "真": "real", "假": "fake",
    "空": "empty", "满": "full", "全": "all",
    "已": "already", "请": "please", "过": "pass", "期": "period",
    "要": "want", "会": "can", "能": "can", "可": "may", "需": "need",
    "用": "use", "做": "do", "到": "to", "从": "from", "为": "for",
    "与": "and", "及": "and", "但": "but", "让": "let", "给": "give",
    "查": "check", "改": "change", "换": "change", "删": "delete",
    "读": "read", "写": "write", "发": "send", "收": "receive",
    "存": "save", "取": "get", "输": "input", "出": "output", "入": "input",
    "停": "stop", "启": "start",
}


def clean_senses(translation):
    """只取正文释义：整行都是 [计]/[网络]/[医] 标注的是术语对译，
    常把缩写词（ad）的冷门义项排到常用词前面，整行丢弃；
    正文里的括号注释（（水）、<爱尔兰>）仍然剥掉。"""
    out = []
    for raw in translation.replace("\\n", "\n").split("\n"):
        if raw.strip().startswith("["):
            continue
        s = BRACKET.sub("", raw)
        s = re.sub(r"^[a-z]{1,4}\.(?:\s+[a-z]{1,4}\.)*\s*", "", s.strip())
        out.extend(seg.strip() for seg in SEP.split(s or "") if seg.strip())
    return out


def level(row):
    """越低越基础；只信基础词的释义，避免把术语对译当成常用词。"""
    oxford, collins = row.get("oxford") or "", row.get("collins") or ""
    tag, frq = row.get("tag") or "", row.get("frq") or "0"
    if oxford == "1":
        return 0
    if oxford == "2":
        return 1
    if collins and collins >= "3":
        return 2
    if any(t in tag for t in ("zk", "gk", "cet4", "cet6")):
        return 3
    return 4 if frq.isdigit() and int(frq) > 0 else 5


def freq_rank(row):
    ranks = [int(row.get(k) or 0) for k in ("frq", "bnc")]
    ranks = [r for r in ranks if r > 0]
    return min(ranks) if ranks else 10 ** 7


def main():
    src, dst = sys.argv[1], sys.argv[2]
    table = {}

    def offer(span, word, key):
        old = table.get(span)
        if old is None or key < old[0]:
            table[span] = (key, word)

    def supply(sense, idx, word, lv, rank):
        # 整条释义就是该中文 = 最强证据；释义里的片段次之；
        # 释义越靠前越能代表这个词的本义（wish 的第 5 个义项才是「请求」）。
        for seg in CJK.findall(sense):
            if not 2 <= len(seg) <= MAX_SPAN or len(sense) > 12:
                continue
            tier = 0 if seg == sense else 1
            offer(seg, word, (tier, idx, lv, rank, len(word)))

    with open(src, newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            word = (row.get("word") or "").strip()
            tr = row.get("translation") or ""
            if not tr.strip() or not LEXWORD.fullmatch(word):
                continue
            if len(word) == 1 or word.isupper():
                continue
            lv, rank = level(row), freq_rank(row)
            if lv > 4 or rank >= 10 ** 7:
                continue
            for idx, sense in enumerate(clean_senses(tr)[:6]):
                supply(sense, idx, word, lv, rank)

    items = [(span, rec) for span, rec in table.items() if len(span) > 1]
    items.sort(key=lambda kv: kv[1][0])
    items = items[:MAX_ENTRIES]
    items += [(span, ((0, 0, 0, 0, 0), en)) for span, en in HAND.items()]
    items += [(span, ((0, 0, 0, 0, 0), en)) for span, en in SINGLE.items()]
    items.sort(key=lambda kv: kv[0])
    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(f"{span}\t{rec[1]}" for span, rec in items))
    print(f"{len(items)} entries -> {dst}")


if __name__ == "__main__":
    main()
