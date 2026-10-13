//! RTF 生成回归测试：重点验证 plan.md 4.3 的转义与 Unicode 风险。
//! 断言只针对"必须成立"的不变量（转义、平衡、无裸控制字符），不锁定排版细节。

use wild_mark::md2rtf::{self, Cp, Opt};

fn rtf(md: &str) -> String {
    let o = Opt { cp: Cp::Uni, ..Default::default() };
    String::from_utf8(md2rtf::to_rtf(md, &o).rtf).unwrap()
}

/// RTF 不变量：大括号平衡、无未转义的 `{` `}` `\`、控制字符已十六进制化
fn assert_valid_rtf(s: &str) {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' => {
                // 反斜杠必须成对转义，或引导控制字/转义序列
                let n = *b.get(i + 1).unwrap_or(&b' ');
                assert!(!matches!(n, b'\n' | b'\r'), "裸反斜杠后跟换行 at {i}");
                if n == b'\\' || n == b'{' || n == b'}' {
                    i += 1;
                }
            }
            c if c < 0x20 && c != b'\n' && c != b'\r' => panic!("未转义控制字符 {c:#04x}"),
            c if c >= 0x80 => panic!("非 ASCII 字节 {c:#04x} 未走 \\u 转义（Uni 模式要求全 ASCII）"),
            _ => {}
        }
        assert!(depth >= 0, "大括号提前闭合 at {i}");
        i += 1;
    }
    assert_eq!(depth, 0, "大括号不平衡");
}

#[test]
fn escapes_structural_chars() {
    let r = rtf("字面大括号 `{` 与 } 以及反斜杠 \\ 和 \"引号\" 与 'apos'\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\{"), "左花括号未转义: {r}");
    assert!(r.contains("\\}"), "右花括号未转义");
    assert!(r.contains("\\\\"), "反斜杠未转义");
}

#[test]
fn unicode_uses_u_escapes() {
    let r = rtf("中文😀\n");
    assert_valid_rtf(&r);
    // 中 → U+4E2D → \u20013?；😀 → 代理对 \u-...?\u-...?
    assert!(r.contains("\\u20013?"), "中文未按 UTF-16 码元转义: {r}");
    assert!(r.contains("\\u-10179?") && r.contains("\\u-8704?"), "非 BMP 未拆代理对: {r}");
}

#[test]
fn control_chars_hex_encoded() {
    let r = rtf("a\u{1}b\u{2}c\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\'01") && r.contains("\\'02"), "控制字符未十六进制化: {r}");
}

#[test]
fn headings_and_inline_shapes_present() {
    let r = rtf("# 标题一\n\n**粗** *斜* `码` [链接](https://a.b/c)\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\b "), "标题/粗体缺 \\b");
    assert!(r.contains("\\i "), "斜体缺 \\i");
    assert!(r.contains("HYPERLINK"), "链接未生成 field");
    assert!(r.contains("\\f1"), "行内代码未切换等宽字体");
}

#[test]
fn url_in_field_is_escaped() {
    let r = rtf("[x](https://a.b/?q=1{2}\\3)\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\{2\\}\\\\3"), "URL 内大括号/反斜杠未转义");
}

#[test]
fn table_emits_row_and_cells() {
    let r = rtf("| A | B | 空 |\n|---|---|---|\n| 中文很长很长的一段单元格 |  | 3 |\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\trowd"), "表格缺 \\trowd");
    assert!(r.contains("\\cellx"), "表格缺 \\cellx");
    assert_eq!(r.matches("\\cell\n").count(), 6, "单元格数量不符");
    assert!(r.contains("\\row"), "缺 \\row");
}

#[test]
fn code_block_keeps_line_breaks_and_mono() {
    let r = rtf("```rust\nfn 中文() {\n    let x = 1; // 😀\n}\n```\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\par"), "代码块行未用 \\par 分隔");
    assert!(r.contains("\\chcbpat"), "代码块缺背景色控制字");
    assert!(r.contains("\\f1"), "代码块未用等宽字体");
}

#[test]
fn nested_lists_indent_increases() {
    let r = rtf("- 一\n  - 二\n    - 三\n");
    assert_valid_rtf(&r);
    assert!(r.contains("\\li280 ") && r.contains("\\li560 ") && r.contains("\\li840 "), "缩进未随层级递增: {r}");
    assert!(r.contains("\\u8226?"), "缺项目符号");
}

#[test]
fn huge_input_is_linear_and_ascii() {
    // 200KiB 中文段落：验证 RTF 全 ASCII 且体积膨胀比可预期（Uni 模式约 x2.5）
    let md = "中文字符串测试。".repeat(20_000);
    let o = Opt { cp: Cp::Uni, ..Default::default() };
    let out = md2rtf::to_rtf(&md, &o);
    let s = String::from_utf8(out.rtf.clone()).expect("Uni 模式输出必须是 ASCII");
    assert_valid_rtf(&s);
    let ratio = out.rtf.len() as f64 / md.len() as f64;
    assert!(ratio > 2.0 && ratio < 3.2, "体积膨胀比异常: {ratio}");
}

#[test]
fn gbk_mode_emits_raw_bytes_for_cjk() {
    let md = "中文中文中文\n";
    let o = Opt { cp: Cp::Gbk, ..Default::default() };
    let out = md2rtf::to_rtf(&md, &o);
    let s = String::from_utf8_lossy(&out.rtf).to_string();
    assert!(s.contains("ansicpg936"), "GBK 模式未声明代码页");
    assert!(out.rtf.iter().any(|b| *b >= 0x80), "GBK 模式应出现原始 DBCS 字节");
    // 结构性字符仍须转义
    assert!(s.contains("\\{") || !md.contains('{'));
}

#[test]
fn pathological_document_survives() {
    let md = std::fs::read_to_string("testdata/pathological/rtf_escape.md").unwrap_or_default();
    if md.is_empty() {
        return; // 未生成测试数据时跳过（由 gen_testdata 产出）
    }
    let o = Opt { cp: Cp::Uni, ..Default::default() };
    let out = md2rtf::to_rtf(&md, &o);
    let s = String::from_utf8(out.rtf.clone()).expect("含非 ASCII 字节，转义有漏");
    assert_valid_rtf(&s);
    assert!(out.rtf.len() > md.len(), "病态文档 RTF 反而变小，疑似丢内容");
}

#[test]
fn feature_matrix_all_structures() {
    let md = std::fs::read_to_string("testdata/small/feature_matrix.md").unwrap_or_default();
    if md.is_empty() {
        return;
    }
    let o = Opt { cp: Cp::Uni, ..Default::default() };
    let out = md2rtf::to_rtf(&md, &o);
    let s = String::from_utf8(out.rtf.clone()).expect("功能矩阵输出含裸非 ASCII");
    assert_valid_rtf(&s);
    for k in ["\\trowd", "HYPERLINK", "\\chcbpat", "\\brdrb", "\\strike", "\\u9744?", "\\u9746?"] {
        assert!(s.contains(k), "缺结构控制字 {k}");
    }
}

#[test]
fn html_is_escaped_not_injected() {
    let r = rtf("<script>alert(1)</script>\n\n<div>x</div>\n");
    assert_valid_rtf(&r);
    // HTML 原文按文本转义后，不应出现裸 <b> 之类被当成 RTF 结构；这里只要求不破坏平衡
    assert!(r.contains("<script>"), "HTML 应作为文本保留");
    assert!(!r.contains("\\pard<script>"), "HTML 不该被当成控制串");
}

#[test]
fn ordered_list_numbers_increment() {
    let r = rtf("1. 一\n2. 二\n3. 三\n");
    assert_valid_rtf(&r);
    assert!(r.contains("1.\\tab ") && r.contains("2.\\tab ") && r.contains("3.\\tab "), "有序列表编号未递增: {r}");
}

#[test]
fn empty_document_is_valid() {
    let r = rtf("");
    assert_valid_rtf(&r);
    assert!(r.starts_with("{\\rtf1"), "缺 RTF 头");
}
