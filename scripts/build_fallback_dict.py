#!/usr/bin/env python3
"""生成中→英兜底表（span<TAB>英文），只收「整条释义完全相等」的强证据。

数据来源：https://github.com/skywind3000/ECDICT （MIT License）
词边界白名单：https://github.com/fxsjy/jieba 的 dict.txt（MIT License，仅构建期使用）
用法：
  python3 scripts/build_fallback_dict.py <ecdict.csv> <jieba dict.txt> <src/dict_zh.tsv> <src/dict_fallback.tsv>

为什么不像通用表那样收「释义里的片段」：实测把「超高速缓存」切成「储器」会得到
储器→as、理机→solomon 这类垃圾，支持度再高也救不回来。中文缺词边界信息时，
只有「整条释义就是该词」才算可靠证据；分词交给运行时最长匹配。

四道过滤：整条释义相等、jieba 词表收录、英文本身是常用词（复数还原后查 ECDICT
词频）、排除人名/地名/缩写。运行时仅当通用表查不到时才查本表，所以这里宁缺毋滥。
"""

import csv
import re
import sys

BRACKET = re.compile(r"[\[（(【<][^\[（(】\]>]{0,14}[\]）)】>]?")
SEP = re.compile(r"[；;，,、。\.\s/｜|…—\-\"'“”‘’:：?？!！]+")
LEXWORD = re.compile(r"[A-Za-z][A-Za-z'\- ]{0,30}")
NOISE = re.compile(r"人名|地名|缩写|abbr|简称|俗称|亦作|参见|复数|词形")
TRAILING = re.compile(r"[的地得]$")
BAD_LEAD = re.compile(r"^(a|an|the)[ -]", re.I)

MAX_SPAN = 4
MAX_ENTRIES = 20000
COMMON_RANK = 50000


def singular(w):
    if " " in w or "-" in w:
        return w
    lw = w.lower()
    if lw.endswith("ies") and len(w) > 4:
        return w[:-3] + "y"
    if lw.endswith(("sses", "shes", "ches", "xes", "zes")):
        return w[:-2]
    if lw.endswith("s") and not lw.endswith(("ss", "us", "is")) and len(w) > 3:
        return w[:-1]
    return w


def clean(raw):
    s = BRACKET.sub("", raw)
    s = re.sub(r"^[a-z]{1,4}\.(?:\s+[a-z]{1,4}\.)*\s*", "", s.strip())
    return [x.strip() for x in SEP.split(s) if x.strip()]


def domain(raw):
    if raw.startswith("[计]"):
        return 0, raw[3:].strip()
    if raw.startswith("[网络]"):
        return 1, raw[5:].strip()
    return 2, raw


def read_jieba(path):
    words = set()
    with open(path, encoding="utf-8") as f:
        for line in f:
            parts = line.split()
            if parts:
                words.add(parts[0])
    return words


def head_rank(term, common):
    """多词术语按中心词判常用度：cache storage -> storage。"""
    t = singular(term).lower()
    if " " in t:
        t = t.split()[-1]
    if "-" in t:
        t = t.split("-")[-1]
    return common.get(t, 10 ** 7)


def main():
    src, jieba_path, general_path, dst = sys.argv[1:5]
    words = read_jieba(jieba_path)
    general = set()
    with open(general_path, encoding="utf-8") as f:
        general = {line.split("\t")[0] for line in f if "\t" in line}

    rows = list(csv.DictReader(open(src, newline="", encoding="utf-8")))

    common = {}
    for row in rows:
        w = (row.get("word") or "").strip().lower()
        if not w:
            continue
        ranks = [int(row.get(k) or 0) for k in ("frq", "bnc")]
        ranks = [r for r in ranks if r > 0]
        if ranks:
            common[w] = min(common.get(w, 10 ** 9), min(ranks))

    cands = {}

    def offer(span, term, key):
        bucket = cands.setdefault(span, {})
        old = bucket.get(term)
        if old is None or key < old:
            bucket[term] = key

    for row in rows:
        word = (row.get("word") or "").strip()
        tr = row.get("translation") or ""
        if not tr.strip() or not LEXWORD.fullmatch(word):
            continue
        if len(word) < 2 or word.isupper() or NOISE.search(word):
            continue
        tokens = word.split()
        if len(tokens) > 3 or any(t.isupper() and len(t) > 1 for t in tokens):
            continue
        # 多词且含大写开头 = 专名（Jesse Jackson / World War I），不收
        if len(tokens) > 1 and any(t[:1].isupper() for t in tokens):
            continue
        # 「a week」「a single」是释义不是术语
        if BAD_LEAD.match(word):
            continue
        term = singular(word).lower()
        if head_rank(term, common) >= COMMON_RANK:
            continue
        ranks = [int(row.get(k) or 0) for k in ("frq", "bnc")]
        rank = min([r for r in ranks if r > 0] or [10 ** 7])
        for raw in tr.replace("\\n", "\n").split("\n"):
            raw = raw.strip()
            if not raw:
                continue
            dom, body = domain(raw)
            for sense in clean(body):
                if sense in general or sense not in words:
                    continue
                if not 2 <= len(sense) <= MAX_SPAN or NOISE.search(sense):
                    continue
                if TRAILING.search(sense) or len(sense) > 6:
                    continue
                offer(sense, term, (dom, len(term.split()), len(term), rank))

    items = []
    for span, bucket in cands.items():
        term, key = min(bucket.items(), key=lambda kv: kv[1])
        items.append((key, span, term))
    items.sort(key=lambda it: it[0])
    items = items[:MAX_ENTRIES]
    items.sort(key=lambda it: it[1])
    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(f"{span}\t{term}" for _, span, term in items))
    import os
    print(f"{len(items)} entries, {os.path.getsize(dst) / 1024:.0f} KB -> {dst}")


if __name__ == "__main__":
    main()
