"""从 Scintilla.h / SciLexer.h 生成 Rust 常量绑定，避免手抄消息号出错。

用法: python scripts/gen_sci_const.py
输出: src/sci_const.rs
"""
import re, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
HDRS = [ROOT / "native/scintilla/include/Scintilla.h", ROOT / "native/lexilla/include/SciLexer.h"]
OUT = ROOT / "src/sci_const.rs"

# 只导出原型用到的命名空间，保持绑定少而明确
PREFIX = ("SCI_", "SCINTILLA_", "SC_CP_", "SC_WRAP_", "SC_MARGIN_", "SC_ELEMENT_", "SC_MODEVENT",
          "SCN_", "SC_STATUS_", "SC_ALPHA_", "SC_INDICATOR_", "SC_MARK_", "SC_EOL_", "SC_UPDATE_",
          "SC_FIND_", "SC_LINE_", "SC_LIST_", "SC_MOD_", "SC_TASK_", "SC_INTERFACE_",
          "SCE_MARKDOWN_", "SCLEX_", "SC_STATUS_", "SC_EDITCONTROLOVERFLOW")


def parse(path: pathlib.Path) -> dict:
    out = {}
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        m = re.match(r"#define\s+([A-Za-z_][A-Za-z0-9_]*)\s+(-?(?:0[xX])?[0-9A-Fa-f]+)\s*(//.*)?$", line.strip())
        if m and m.group(1).startswith(PREFIX):
            out[m.group(1)] = int(m.group(2).rstrip("uUlL"), 0)
    return out


def main():
    consts = {}
    for h in HDRS:
        if h.exists():
            consts.update(parse(h))
    assert len(consts) > 300, f"常量过少({len(consts)})：Scintilla.h 未解包？"
    # SC_MOD_* / SC_FIND_* 在 5.15 里是 C++ enum，不在 Scintilla.h #define 中，故只校验宏形式的常量
    for probe in ("SCI_SETTEXT", "SCI_APPENDTEXT", "SCI_GETLINECOUNT", "SCI_SETILEXER", "SCE_MARKDOWN_HEADER1",
                  "SCLEX_MARKDOWN", "SCN_MODIFIED", "SCI_SETMARGINTYPEN", "SC_MODEVENTMASKALL", "SC_CP_UTF8"):
        assert probe in consts, f"缺少 {probe}"
    lines = [
        "// 自动生成：scripts/gen_sci_const.py，勿手工编辑。",
        f"// 来源 Scintilla.h / SciLexer.h，共 {len(consts)} 个常量。",
        "",
    ]
    for k in sorted(consts):
        ty = "u32" if k.startswith("SCI_") or k.startswith("SCN_") else "usize"
        lines.append(f"pub const {k}: {ty} = {consts[k]};")
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{OUT}: {len(consts)} consts")
    for probe in ("SCI_SETTEXT", "SCI_APPENDTEXT", "SCI_GETTEXTLENGTH", "SCI_SETILEXER", "SCE_MARKDOWN_HEADER1", "SCLEX_MARKDOWN"):
        print(f"  {probe} = {consts[probe]}")


if __name__ == "__main__":
    main()
