#!/usr/bin/env python3
"""离线词典基准：按运行时同样的优先级做最长匹配，比较 before/after。

用法：
  python3 scripts/bench_dict.py --zh <dict_zh.tsv> [--fallback <tsv>] [--modern <tsv>]
  python3 scripts/bench_dict.py --before <dict_zh_before.tsv>     # 只跑通用表+PAIRS
  python3 scripts/bench_dict.py --ident                            # 标识符英→中

优先链与 src/dict.rs 一致：PAIRS 反查 > modern_terms > dict_zh > dict_fallback。
标识符模式：PAIRS > code_terms > modern 反查 > ECDICT（before 只有 PAIRS + ECDICT）。
"""

import argparse
import re
import sys


def load_tsv(path):
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if "\t" not in line or line.startswith("#"):
                continue
            k, v = line.rstrip("\n").split("\t", 1)
            out.setdefault(k, v)
    return out


def load_pairs(src="src/dict.rs"):
    text = open(src, encoding="utf-8").read()
    body = text.split("const PAIRS", 1)[1].split("];", 1)[0]
    pairs = re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', body)
    rev = {}
    for en, zh in pairs:
        rev.setdefault(zh, en)
    return rev


def load_pairs_en(src="src/dict.rs"):
    text = open(src, encoding="utf-8").read()
    body = text.split("const PAIRS", 1)[1].split("];", 1)[0]
    out = {}
    for en, zh in re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', body):
        out.setdefault(en, zh)
    return out


def load_modern_rev(path):
    """modern_terms.tsv 是 中文<TAB>英文，这里反过来建 英文→中文。"""
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or "\t" not in line:
                continue
            zh, en = line.rstrip("\n").split("\t", 1)
            out.setdefault(en.strip().lower(), zh.strip())
    return out


CAMEL = re.compile(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+")


def split_identifier(s):
    words = []
    for chunk in re.sub(r"[_\-.]", " ", s).split():
        words.extend(CAMEL.findall(chunk) or [chunk])
    return words


def find_exact(word, chain):
    for table in chain:
        if word in table:
            return table[word]
    return None


def phrase_key(s):
    return " ".join(re.split(r"[-_\s]+", s.strip().lower()))


def build_phrase_dict(*tables):
    out = {}
    for table in tables:
        for en, zh in table.items():
            if " " in en or "-" in en:
                out.setdefault(phrase_key(en), zh)
    return out


def resolve_ident(text, chain, phrases=None):
    """镜像 translate 的代码路径：整词/多词命中优先，否则逐词查。"""
    t = text.strip().lower()
    hit = find_exact(t, chain)
    if hit:
        return hit
    if phrases and (" " in t or "-" in t):
        hit = phrases.get(phrase_key(t))
        if hit:
            return hit
    return " ".join(
        find_exact(w.lower(), chain) or w for w in split_identifier(text)
    )


def resolve(zh, chain, max_span=8):
    for table in chain:
        if zh in table:
            return table[zh]
    chars = list(zh)
    out, i = [], 0
    while i < len(chars):
        hit = None
        for n in range(min(max_span, len(chars) - i), 0, -1):
            span = "".join(chars[i:i + n])
            for table in chain:
                if span in table:
                    hit = (n, table[span])
                    break
            if hit:
                break
        if hit:
            out.append(hit[1])
            i += hit[0]
        else:
            out.append(chars[i])
            i += 1
    return " ".join(out)


def norm(s):
    return re.sub(r"\s+", " ", s.strip().lower())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--zh", default="src/dict_zh.tsv")
    ap.add_argument("--fallback")
    ap.add_argument("--modern")
    ap.add_argument("--code", default="src/code_terms.tsv")
    ap.add_argument("--before", help="只和这张旧表对比")
    ap.add_argument("--ident", action="store_true", help="标识符英→中基准（gold_ident.tsv）")
    ap.add_argument("--gold", default="scripts/gold_terms.tsv")
    args = ap.parse_args()

    if args.ident:
        pairs = load_pairs_en()
        ecdict = load_tsv("src/dict_data.tsv")
        code = load_tsv(args.code)
        modern_rev = load_modern_rev(args.modern or "scripts/modern_terms.tsv")
        gold = args.gold if args.gold != "scripts/gold_terms.tsv" else "scripts/gold_ident.tsv"
        categories = ("type", "data", "net", "storage", "security", "arch", "ai", "ui", "action", "heldout", "abbrev")
        before_fn = lambda t, c: resolve_ident(t, c)
        phrases = build_phrase_dict(code, modern_rev)
        after_fn = lambda t, c: resolve_ident(t, c, phrases)
        report(
            "ident before (PAIRS + ECDICT)",
            score(gold, [pairs, ecdict], before_fn, miss_on="ascii"),
            categories,
        )
        report(
            "ident after (PAIRS + code_terms + modern_rev + ECDICT)",
            score(gold, [pairs, code, modern_rev, ecdict], after_fn, miss_on="ascii"),
            categories,
            show=True,
        )
        return 0

    pairs = load_pairs()
    if args.before:
        chain = [pairs, load_tsv(args.before)]
        label = f"before ({args.before} + PAIRS)"
    else:
        chain = [pairs]
        if args.modern:
            chain.append(load_tsv(args.modern))
        chain.append(load_tsv(args.zh))
        if args.fallback:
            chain.append(load_tsv(args.fallback))
        label = "after (PAIRS + modern + zh + fallback)"
    report(label, score(args.gold, chain, resolve, miss_on="cjk"), ("core", "modern", "dist", "heldout", "general"), show=True)
    return 0


def score(path, chain, fn, miss_on=None):
    stats = {}
    diffs, misses = [], []
    with open(path, encoding="utf-8") as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 3:
                continue
            src, want, cat = parts[0], parts[1], parts[2]
            got = fn(src, chain)
            s = stats.setdefault(cat, {"n": 0, "ok": 0, "diff": 0, "miss": 0})
            s["n"] += 1
            if norm(got) == norm(want):
                s["ok"] += 1
            elif (miss_on == "ascii" and re.search(r"[A-Za-z]", got)) or (
                miss_on == "cjk" and re.search(r"[\u4e00-\u9fff]", got)
            ):
                s["miss"] += 1
                misses.append((cat, src, want, got))
            else:
                s["diff"] += 1
                diffs.append((cat, src, want, got))
    return stats, diffs, misses


def report(label, result, categories, show=False):
    stats, diffs, misses = result
    print(label)
    total = {"n": 0, "ok": 0, "diff": 0, "miss": 0}
    for cat in categories:
        if cat not in stats:
            continue
        s = stats[cat]
        for k in total:
            total[k] += s[k]
        print(f"  {cat:8s} n={s['n']:3d}  exact={s['ok']:3d}  different={s['diff']:3d}  missing={s['miss']:3d}")
    print(f"  {'TOTAL':8s} n={total['n']:3d}  exact={total['ok']:3d}  different={total['diff']:3d}  missing={total['miss']:3d}"
          f"   exact%={100 * total['ok'] / max(1, total['n']):.1f}")

    if show and diffs:
        print("\n  译法不同（前 25）:")
        for cat, src, want, got in diffs[:25]:
            print(f"    [{cat}] {src}: 期望={want}  实际={got}")
    if show and misses:
        print("\n  未命中（前 25）:")
        for cat, src, want, got in misses[:25]:
            print(f"    [{cat}] {src}: 期望={want}  实际={got}")
    print()


if __name__ == "__main__":
    sys.exit(main())
