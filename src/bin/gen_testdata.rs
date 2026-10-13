//! 测试数据生成：small/（功能矩阵+病态用例，入库）与 large/（1/10/32/64/128MiB，不入库）。
//! 内容刻意混排中文、全角标点、Emoji、代码块、表格，覆盖 plan.md 4.3 的 P2 清单。

use std::io::Write;

const ZH: &str = "Markdown 编辑器需要处理中英文混排：标点（，。；：“”‘’（）、《》）与 Emoji 😀🎉 都要正确回退与断行。";
const EN: &str = "The quick brown fox jumps over the lazy dog while the editor keeps the caret stable.";

fn block(i: usize, structed: bool, code: bool) -> String {
    if code {
        return format!("```rust\nfn case_{i}() -> Result<(), Error> {{\n    let v = vec![{i}, {i}, {i}]; // {ZH}\n    Ok(())\n}}\n```\n\n");
    }
    if structed {
        return match i % 5 {
            0 => format!("## 标题 {i}：{ZH}\n\n{EN}\n\n- 项目一：{ZH}\n- 项目二\n  - 嵌套项\n    - 更深一层 {i}\n\n"),
            1 => format!("### 小节 {i}\n\n> 引用块：{ZH}\n>\n> 第二行引用 {EN}\n\n1. 有序一\n2. 有序二\n3. 有序三 {i}\n\n"),
            2 => format!(
                "#### 表格 {i}\n\n| 名称 | 说明 | 备注 |\n|---|---|---|\n| 中文列 | {ZH} | 空 |\n| 空单元格 |  | 很长很长的单元格内容{EN} |\n| Emoji 😀 | 反斜杠 `\\` 与大括号 `{{}}` | {i} |\n\n"
            ),
            3 => format!("##### 强调 {i}\n\n**粗体**、*斜体*、`行内代码`、[链接](https://example.com/{i} “标题”)、~~删除线~~。\n\n{EN}\n\n---\n\n"),
            _ => format!("###### 分隔与长行 {i}\n\n{}\n\n{ZH}\n\n", "非常长的单行中文段落".repeat(30)),
        };
    }
    format!("{EN} {ZH} 段落 {i}。\n\n")
}

fn build(size_mb: usize, structed: bool, code: bool) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::with_capacity(size_mb * 1024 * 1024 + 4096);
    v.extend_from_slice("# wild-markdown 基准文档\n\n".as_bytes());
    let mut i = 0usize;
    while v.len() < size_mb * 1024 * 1024 {
        let b = block(i, structed, code && i % 4 == 0);
        v.extend_from_slice(b.as_bytes());
        i += 1;
    }
    v
}

/// 病态用例：RTF 控制字符、非 BMP、超长段落、大量重复节点、深嵌套
fn pathological() -> String {
    let mut s = String::new();
    s.push_str("# 病态用例 RTF 转义\n\n");
    s.push_str("大括号 `{` `}`、反斜杠 `\\`、引号 \" 与 '、制表\t、非 BMP：😀🎉🀄𠀋𝄞。\n\n");
    s.push_str("控制字符：<0x01><0x02>，HTML 尖括号 <b>bold</b>，实体 &amp; &lt;。\n\n");
    s.push_str(&format!("超长单段：{}\n\n", "这是一段没有空格的中文长句用于测试换行。".repeat(2000)));
    s.push_str(&format!("超长单行代码：{}\n\n", "int x=0; ".repeat(3000)));
    s.push_str("1. 深嵌套\n");
    let mut nest = String::from("- 第0层\n");
    for lvl in 1..40 {
        nest.push_str(&format!("{}- 第{lvl}层\n", "  ".repeat(lvl)));
    }
    s.push_str(&nest);
    s.push('\n');
    let mut row = String::from("|");
    for c in 0..40 {
        row.push_str(&format!(" 列{c} |"));
    }
    s.push_str(&row);
    s.push('\n');
    s.push_str(&format!("|{}|\n", "|---".repeat(40)));
    for r in 0..40 {
        let cells: Vec<String> = (0..40).map(|c| if c % 7 == 0 { String::new() } else { format!("r{r}c{c} 😀") }).collect();
        s.push_str(&format!("|{}|\n", cells.join("|")));
    }
    s.push_str("\n```\n");
    for l in 0..3000 {
        s.push_str(&format!("line {l} 中文内容 {ZH}\n"));
    }
    s.push_str("```\n");
    s
}

/// 功能矩阵：plan.md P2 需要覆盖的全部元素
fn feature_matrix() -> String {
    r#"# 一级标题：中文与 English 混排

段落里包含 **粗体**、*斜体*、***粗斜***、`行内代码`、~~删除线~~、[带标题的链接](https://example.com "示例站点")、
[裸链接](https://example.com/bare)、以及 Emoji 😀 与全角标点（，。！？；：）「」《》」。

## 二级标题 Heading 2

> 引用一层：{zh}
>
> > 引用嵌套：第二层引用
> > 第三行

### 列表

- 无序一
- 无序二
  - 嵌套子项 A
  - 嵌套子项 B
    - 三级嵌套
- 无序三含 `代码` 与 **粗体**

1. 有序第一
2. 有序第二
   1. 子有序 1
   2. 子有序 2
3. 有序第三

- [ ] 未完成任务
- [x] 已完成任务

### 代码块

```python
def 中文函数(a, b):
    """docstring 中文说明"""
    return a + b  # 注释：{zh}
```

    缩进式代码块
    第二行

### 表格（含空单元格、长单元格、中文换行）

| 短 | 中文很长的列标题说明 | 空 | 尾列 |
|:---|---:|:---:|---|
| A | 这是一段比较长的中文单元格内容，用于测试表格内换行行为 |  | 1 |
| B |  | 占位 | 2 |
| 😀🎉 | `\| { } \` 特殊字符 | 空 | 3 |

### 分隔线与硬换行
第一行硬换行  
第二行

---

***

### 图片与 HTML（第一版按占位处理）

![图片描述](./missing.png "标题")

<div class="raw">内嵌 HTML 原始文本</div>

<span>行内 <i>HTML</i> 标签</span>

### 脚注

带脚注的文本[^note]。

[^note]: 脚注内容，也包含中文。
"#
    .replace("{zh}", ZH)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want_large = a.iter().any(|s| s == "--large");
    let dir_small = std::path::Path::new("testdata/small");
    let dir_path = std::path::Path::new("testdata/pathological");
    std::fs::create_dir_all(dir_small).ok();
    std::fs::create_dir_all(dir_path).ok();
    std::fs::write(dir_small.join("feature_matrix.md"), feature_matrix()).unwrap();
    std::fs::write(dir_path.join("rtf_escape.md"), pathological()).unwrap();
    std::fs::write(
        dir_path.join("single_huge_line.md"),
        format!("# 单行超长\n\n{}\n", "无空格中文长句".repeat(200_000)),
    )
    .unwrap();
    println!("small/pathological 已生成");
    if want_large {
        let dir_large = std::path::Path::new("testdata/large");
        std::fs::create_dir_all(dir_large).ok();
        for (mb, kind) in [
            (1usize, "norm"),
            (1, "struct"),
            (1, "code"),
            (10, "norm"),
            (10, "struct"),
            (10, "code"),
            (32, "norm"),
            (32, "struct"),
            (32, "code"),
            (64, "norm"),
            (64, "struct"),
            (64, "code"),
            (128, "norm"),
            (128, "struct"),
            (128, "code"),
        ] {
            let p = dir_large.join(format!("md_{mb}MiB_{kind}.md"));
            if p.exists() {
                println!("跳过 {}", p.display());
                continue;
            }
            let t0 = std::time::Instant::now();
            let mut f = std::io::BufWriter::new(std::fs::File::create(&p).unwrap());
            let data = build(mb, kind == "struct", kind == "code");
            f.write_all(&data).unwrap();
            f.flush().unwrap();
            println!("{} {:.1}MiB in {:?}", p.display(), data.len() as f64 / 1048576.0, t0.elapsed());
        }
    }
}
