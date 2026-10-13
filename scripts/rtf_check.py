"""RTF 平衡/转义诊断：逐行报告大括号深度变化，定位生成器的结构性错误。

用法: python scripts/rtf_check.py <file.rtf>
"""
import sys


def main(path):
    s = open(path, encoding="utf-8", errors="replace").read()
    depth = 0
    mindepth = 0
    bad = []
    i = 0
    line = 1
    while i < len(s):
        c = s[i]
        if c == "\n":
            line += 1
            i += 1
            continue
        if c == "\\":
            if i + 1 < len(s) and s[i + 1] in "{}\\":
                i += 2
                continue
            # 控制字：吞掉字母与可选数字，以及一个分隔空格
            i += 1
            while i < len(s) and s[i].isalpha():
                i += 1
            k = 0
            while i < len(s) and (s[i].isdigit() or s[i] == "-"):
                i += 1
                k += 1
            if k and i < len(s) and s[i] == " ":
                i += 1
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth < mindepth:
                mindepth = depth
                bad.append((line, "mindepth", depth, s[max(0, i - 60):i + 20].replace("\n", "\\n")))
        elif ord(c) < 0x20 and c not in "\r\n\t":
            bad.append((line, "ctrl", ord(c), ""))
        elif ord(c) >= 0x80:
            bad.append((line, "nonascii", hex(ord(c)), ""))
        i += 1
    print(f"{path}: final_depth={depth} min_depth={mindepth} issues={len(bad)}")
    for b in bad[:12]:
        print("  ", b[:3], str(b[3])[:120])
    return 0 if depth == 0 and mindepth == 0 and not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
