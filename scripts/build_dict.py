#!/usr/bin/env python3
"""从 ECDICT 全量 CSV 提取高频词条，生成嵌入用的紧凑 TSV（word<TAB>释义）。

数据来源：https://github.com/skywind3000/ECDICT （MIT License）
用法：python3 scripts/build_dict.py <ecdict.csv> <src/dict_data.tsv> [条数上限]
"""

import csv
import sys


def to_int(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return 0


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
    # 有词频标注的按 (frq, bnc) 降序排前；无词频的按原顺序排后补足
    with_freq = [r for r in rows if r[2] > 0 or r[3] > 0]
    without_freq = [r for r in rows if r[2] == 0 and r[3] == 0]
    with_freq.sort(key=lambda r: (r[2], r[3]), reverse=True)
    rows = with_freq + without_freq

    seen = set()
    out = []
    for word, tr, _, _ in rows:
        key = word.lower()
        if key in seen:
            continue
        seen.add(key)
        senses = [s.strip() for s in tr.split("\n") if s.strip()]
        gloss = "；".join(senses[:2])[:100]
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
