#!/usr/bin/env python3
"""从 ECDICT 全量 CSV 提取高频词条，生成嵌入用的紧凑 TSV（word<TAB>释义）。

数据来源：https://github.com/skywind3000/ECDICT （MIT License）
用法：python3 scripts/build_dict.py <ecdict.csv> <src/dict_data.tsv> [条数上限]
"""

import csv
import re
import sys

TAG = re.compile(r"\[[^\]]{1,10}\]")
POS = re.compile(r"^[a-z]{1,4}\.(?:\s+[a-z]{1,4}\.)*\s*")
SEP = re.compile(r"[；;，,、]+")


def to_int(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return 0


def clean_senses(translation):
    """ECDICT 的多行释义是字面量 \\n，要替换后再切；剥掉词性与 [计]/[医] 标签，
    但保留标签后的中文（「用户」「失败」这类 CS 义项都在标签行里）。"""
    out = []
    text = translation.replace("\\r\\n", "\n").replace("\\n", "\n").replace("\r\n", "\n")
    for raw in text.split("\n"):
        s = POS.sub("", TAG.sub("", raw).strip())
        for seg in SEP.split(s):
            seg = seg.strip(" .。")
            if seg and seg not in out:
                out.append(seg)
    return out


def main():
    src, dst = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 80000
    rows = []
    with open(src, newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            word = (row.get("word") or "").strip()
            tr = (row.get("translation") or "").strip()
            if not word or not tr:
                continue
            rows.append((word, tr, to_int(row.get("frq")), to_int(row.get("bnc"))))
    # frq/bnc 是名次（小 = 常用）：升序把常用词排前，无词频的词按原顺序补足
    with_freq = [r for r in rows if r[2] > 0 or r[3] > 0]
    without_freq = [r for r in rows if r[2] == 0 and r[3] == 0]
    with_freq.sort(key=lambda r: (r[2], r[3]))
    rows = with_freq + without_freq

    seen = set()
    out = []
    for word, tr, _, _ in rows:
        key = word.lower()
        if key in seen:
            continue
        seen.add(key)
        gloss = "；".join(clean_senses(tr)[:3])[:80]
        if not gloss:
            continue
        out.append(f"{word}\t{gloss}")
        if len(out) >= limit:
            break

    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(f"{len(out)} entries -> {dst}")


if __name__ == "__main__":
    main()
