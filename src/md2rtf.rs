//! Markdown → RTF：pulldown-cmark 0.13 事件流直写 RTF 字节缓冲。
//!
//! 转义是第一风险，全部收口在 `Gen::text()`：
//!   - 非 ASCII → `\uN?`（N = UTF-16 码元的有符号十进制，非 BMP 拆代理对，`?` 为 \uc1 的 fallback）；
//!   - 控制字符 → `\'xx`；`{` `}` `\` → 反斜杠转义；
//!   - URL 走独立的 `escape_url`（在 \fldinst 字符串内，还需转义引号）。
//! 其余地方只允许写控制串，杜绝原文直接拼接。

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub const FONT_CN: &str = "Microsoft YaHei";
pub const FONT_MONO: &str = "Cascadia Mono,Consolas,Courier New";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cp {
    Uni, // 纯 ASCII RTF + \u 转义：跨代码页安全（体积膨胀 ~2.6x）
    Gbk, // ansicpg936 + GBK 原始字节：体积小，依赖中文代码页
}

#[derive(Clone)]
pub struct Opt {
    pub cp: Cp,
    pub font_pt: usize,
    pub dark: bool,
    pub page_twips: i32, // 表格总宽（缇）
    pub heading_anchor: bool,
}

impl Default for Opt {
    fn default() -> Self {
        Self { cp: Cp::Uni, font_pt: 11, dark: false, page_twips: 9026, heading_anchor: false }
    }
}

pub struct Out {
    pub rtf: Vec<u8>,
    pub nodes: usize,
}

// \cfN 的颜色表索引（1 起），与 header() 的调色板顺序一一对应
const C_TEXT: i32 = 1;
const C_HEAD: i32 = 2;
const C_QUOTE: i32 = 3;
const C_CODE: i32 = 4;
const C_LINK: i32 = 5;
const C_CODEBG: i32 = 6;
const C_CELLBG: i32 = 7;
const C_RULE: i32 = 8;
const C_FADE: i32 = 9;

struct Gen {
    b: Vec<u8>,
    o: Opt,
    nodes: usize,
    lists: Vec<ListLv>,
    quote: i32,
    cell: Vec<u8>,
    in_cell: bool,
    strong: u8,
    em: u8,
    strike: u8,
    code: bool,
    link: u8,
    para_open: bool,
}

struct ListLv {
    ordered: Option<u64>,
    next: u64,
}

struct Table {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<Vec<u8>>>,
}

impl Gen {
    #[inline]
    fn raw(&mut self, s: &str) {
        if self.in_cell {
            self.cell.extend_from_slice(s.as_bytes());
        } else {
            self.b.extend_from_slice(s.as_bytes());
        }
    }

    #[inline]
    fn byte(&mut self, c: u8) {
        if self.in_cell {
            self.cell.push(c);
        } else {
            self.b.push(c);
        }
    }

    fn uni(&mut self, u: u16) {
        self.raw(&format!("\\u{}?", u as i16));
    }

    /// 文本转义：唯一允许写入正文的入口
    fn text(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '{' => self.raw("\\{"),
                '}' => self.raw("\\}"),
                '\\' => self.raw("\\\\"),
                '\u{0}' => {}
                '\t' => self.raw("\\tab "),
                '\r' => {}
                '\n' => self.raw("\\line "),
                c if (c as u32) < 0x20 => self.raw(&format!("\\'{:02x}", c as u32)),
                c if (c as u32) < 0x80 => self.byte(c as u8),
                c if self.o.cp == Cp::Gbk && (c as u32) <= 0xFFFF && self.push_gbk(c as u16) => {}
                c => {
                    let mut tmp = [0u16; 2];
                    for u in c.encode_utf16(&mut tmp) {
                        self.uni(*u);
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    fn push_gbk(&mut self, u: u16) -> bool {
        use windows::Win32::Globalization::WideCharToMultiByte;
        let mut out = [0u8; 8];
        let n = unsafe { WideCharToMultiByte(936, 0, &[u], Some(&mut out), windows::core::PCSTR::null(), None) };
        if n == 2 {
            // GBK 双字节的尾字节可能落在 { } \ 上，必须转义，否则破坏 RTF 结构
            self.dbyte(out[0], out[1]);
            true
        } else if n == 1 && out[0] >= 0x80 {
            self.dbyte(out[0], 0);
            true
        } else {
            false
        }
    }

    #[cfg(not(windows))]
    fn push_gbk(&mut self, _u: u16) -> bool {
        false
    }

    /// DBCS 双字节写入：尾字节命中结构字符时退化为 \'xx\'yy
    fn dbyte(&mut self, a: u8, b: u8) {
        if matches!(b, b'{' | b'}' | b'\\') || b < 0x20 {
            self.raw(&format!("\\'{a:02x}\\'{b:02x}"));
        } else {
            self.byte(a);
            self.byte(b);
        }
    }

    fn fs(&self) -> i32 {
        self.o.font_pt as i32 * 2
    }

    /// 当前行内样式控制串
    fn inline(&self) -> String {
        let mut s = String::with_capacity(64);
        s.push_str(if self.code { "\\f1\\fs" } else { "\\f0\\fs" });
        s.push_str(&(if self.code { self.fs() - 2 } else { self.fs() }).to_string());
        s.push(' ');
        if self.strong > 0 {
            s.push_str("\\b ");
        }
        if self.em > 0 {
            s.push_str("\\i ");
        }
        if self.strike > 0 {
            s.push_str("\\strike ");
        }
        if self.code {
            s.push_str(&format!("\\cf{C_CODE} \\chshdng100\\chcbpat{C_CODEBG} "));
        } else if self.link == 0 {
            s.push_str(&format!("\\cf{}", if self.quote > 0 { C_QUOTE } else { C_TEXT }));
            s.push(' ');
        }
        s
    }

    /// 带样式地写一段文本（正文唯一出口）
    fn styled(&mut self, s: &str) {
        self.ensure_para();
        let p = self.inline();
        self.raw(&p);
        self.text(s);
        if self.code {
            self.raw("\\chshdng0\\chcbpat0 ");
        }
        if self.strong > 0 {
            self.raw("\\b0 ");
        }
        if self.em > 0 {
            self.raw("\\i0 ");
        }
        if self.strike > 0 {
            self.raw("\\strike0 ");
        }
    }

    /// 段落控制串：缩进 = 列表层级(280) + 引用层级(200)
    fn start_para(&mut self, extra: &str) {
        let li = 280 * self.lists.len() as i32 + 200 * self.quote;
        let fi = if self.lists.is_empty() { 0 } else { -280 };
        self.raw(&format!("\\pard\\plain\\fi{fi}\\li{li} {extra}"));
        self.para_open = true;
    }

    fn end_para(&mut self) {
        if self.para_open {
            self.raw("\\par\n");
            self.para_open = false;
        }
    }

    /// 懒开段落：紧列表的 Item 不发 Paragraph 事件，但正文必须先有 \pard
    fn ensure_para(&mut self) {
        if !self.para_open && !self.in_cell {
            self.start_para("\\sa120 ");
            self.list_marker();
        }
    }

    /// 列表项前缀：项目符号 / 递增编号
    fn list_marker(&mut self) {
        if let Some(lv) = self.lists.last_mut() {
            match lv.ordered {
                Some(_) => {
                    let n = lv.next.to_string();
                    lv.next += 1;
                    self.raw(&format!("\\f0\\fs{} \\cf{C_FADE} {n}.\\tab \\cf{C_TEXT} ", self.fs()));
                }
                None => self.raw(&format!("\\f0\\fs{} \\cf{C_FADE} \\u8226?\\tab \\cf{C_TEXT} ", self.fs())),
            }
        }
    }

    fn head_fs(&self, l: HeadingLevel) -> i32 {
        match l {
            HeadingLevel::H1 => self.fs() + 14,
            HeadingLevel::H2 => self.fs() + 8,
            HeadingLevel::H3 => self.fs() + 4,
            HeadingLevel::H4 => self.fs(),
            HeadingLevel::H5 | HeadingLevel::H6 => self.fs() - 2,
        }
    }

    /// 表格：整表缓冲后统一算列宽（RTF 的 \cellx 必须先于内容确定）
    fn flush_table(&mut self, t: Table) {
        let ncol = t.aligns.len().max(1);
        let mut w = vec![4usize; ncol];
        for row in &t.rows {
            for (i, c) in row.iter().enumerate().take(ncol) {
                let d = display_width(c).clamp(4, 80);
                if d > w[i] {
                    w[i] = d;
                }
            }
        }
        let sum: usize = w.iter().sum();
        let mut acc = 0i32;
        let mut cellx: Vec<i32> = Vec::with_capacity(ncol);
        for i in 0..ncol {
            acc += ((w[i] as f64 / sum as f64) * self.o.page_twips as f64).round() as i32;
            cellx.push(acc);
        }
        *cellx.last_mut().unwrap() = self.o.page_twips;
        self.raw("\\pard\\plain\\fi0\\li0 \\sa120\n");
        for (ri, row) in t.rows.iter().enumerate() {
            let hdr = ri == 0;
            self.raw(&format!("\\trowd\\trgaph85\\trleft0\\trhdr{} ", if hdr { 1 } else { 0 }));
            for cx in &cellx {
                // 单元格四边细边框；表头加底色
                self.raw("\\clbrdrt\\brdrs\\brdrw10\\brdrcf8 \\clbrdrl\\brdrs\\brdrw10\\brdrcf8 \\clbrdrb\\brdrs\\brdrw10\\brdrcf8 \\clbrdrr\\brdrs\\brdrw10\\brdrcf8 ");
                if hdr {
                    self.raw(&format!("\\clcbpat{C_CELLBG} "));
                }
                self.raw(&format!("\\cellx{cx}"));
            }
            self.raw("\n");
            for (i, cell) in row.iter().enumerate().take(ncol) {
                let al = match t.aligns.get(i) {
                    Some(Alignment::Center) => "\\qc ",
                    Some(Alignment::Right) => "\\qr ",
                    _ => "\\ql ",
                };
                self.raw(&format!("\\pard\\plain\\intbl\\fi0\\li0 {al}\\f0\\fs{} \\cf{C_TEXT} ", self.fs() - 2));
                if hdr {
                    self.raw("\\b ");
                }
                self.b.extend_from_slice(cell);
                if hdr {
                    self.raw("\\b0 ");
                }
                self.raw("\\cell\n");
            }
            self.raw("\\row\n");
        }
        self.raw("\\pard\\plain\\par\n");
    }
}

/// RTF 片段的"可见宽度"：ASCII 记 1，`\uN?` 记 2，控制字记 0
fn display_width(frag: &[u8]) -> usize {
    let mut d = 0usize;
    let mut i = 0usize;
    while i < frag.len() {
        match frag[i] {
            b'\\' => {
                if frag[i..].starts_with(b"\\u") {
                    d += 2;
                    while i < frag.len() && frag[i] != b' ' {
                        i += 1;
                    }
                    i += 1;
                } else if frag[i..].starts_with(b"\\tab") {
                    d += 4;
                    i += 4;
                } else {
                    i += 1;
                    while i < frag.len() && frag[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    if i < frag.len() && frag[i] == b' ' {
                        i += 1;
                    }
                }
            }
            b'{' | b'}' => i += 1,
            c if c < 0x80 => {
                d += 1;
                i += 1;
            }
            _ => {
                d += 2;
                i += 1;
            }
        }
    }
    d
}

pub fn header(o: &Opt) -> String {
    let cpg = match o.cp {
        Cp::Uni => "1252",
        Cp::Gbk => "936",
    };
    // 调色板：正文 标题 引用 行内码 链接 代码底 表头底 分隔线 弱化
    let pal: [u32; 9] = if o.dark {
        [0xe8e8e8, 0x69b2ff, 0xa6a6a6, 0xd19a5b, 0x58a6ff, 0x1b1f24, 0x2b3137, 0x454d55, 0x8b8b8b]
    } else {
        [0x202020, 0x0b5ca8, 0x5a5a5a, 0x9a009a, 0x0060c0, 0xf2f4f7, 0xe8eef5, 0xc8c8c8, 0x8a8a8a]
    };
    let ctbl = pal
        .iter()
        .skip(1)
        .map(|v| {
            let (r, g, b) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            format!("\\red{r}\\green{g}\\blue{b};")
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "{{\\rtf1\\ansi\\ansicpg{cpg}\\deff0\\nouicompat\\deflang1033\\deflangfe2052\n\
{{\\fonttbl{{\\f0\\fnil\\fcharset134 {FONT_CN};}}{{\\f1\\fnil\\fcharset0 {FONT_MONO};}}}}\n\
{{\\colortbl ;{ctbl}}}\n\\f0\\fs{}\n",
        o.font_pt as i32 * 2
    )
}

/// Markdown → RTF（预览管线唯一入口：只吃文本，只产 RTF，不碰窗口）
pub fn to_rtf(md: &str, o: &Opt) -> Out {
    let mut g = Gen {
        b: Vec::with_capacity(md.len() * 2 + 4096),
        o: o.clone(),
        nodes: 0,
        lists: Vec::new(),
        quote: 0,
        cell: Vec::new(),
        in_cell: false,
        strong: 0,
        em: 0,
        strike: 0,
        code: false,
        link: 0,
        para_open: false,
    };
    g.raw(&header(o));
    let mut ext = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_FOOTNOTES;
    if o.heading_anchor {
        ext |= Options::ENABLE_HEADING_ATTRIBUTES;
    }
    let mut code_block = false;
    let mut rows: Option<Vec<Vec<Vec<u8>>>> = None;
    let mut cur: Vec<Vec<u8>> = Vec::new();
    let mut aligns: Vec<Alignment> = Vec::new();

    for ev in Parser::new_ext(md, ext) {
        g.nodes += 1;
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                let fs = g.head_fs(level);
                g.start_para(&format!("\\keepn\\sb240\\sa120\\b \\f0\\fs{fs} \\cf{C_HEAD} "));
            }
            Event::End(TagEnd::Heading(_)) => {
                g.raw("\\b0 \\par\n");
                g.para_open = false;
            }
            Event::Start(Tag::Paragraph) => {
                if !code_block {
                    g.start_para("\\sa120 ");
                    g.list_marker();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !code_block {
                    g.end_para();
                }
            }
            Event::Start(Tag::List(num)) => g.lists.push(ListLv { ordered: num, next: num.unwrap_or(1) }),
            Event::End(TagEnd::List(_)) => {
                g.lists.pop();
                g.end_para();
            }
            Event::Start(Tag::Item) => {
                // 紧列表没有 Paragraph 事件；松列表的 Paragraph 走 ensure_para 判定避免重复
                g.para_open = false;
            }
            Event::End(TagEnd::Item) => g.end_para(),
            Event::Start(Tag::BlockQuote(_)) => g.quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => {
                g.quote = (g.quote - 1).max(0);
                g.end_para();
                if g.quote == 0 {
                    g.start_para("\\sb0\\sa120 ");
                    g.raw(&format!("\\f0\\fs{} \\cf{C_RULE} \\u8214?\\cf{C_TEXT} \\par\n", g.fs()));
                    g.para_open = false;
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code_block = true;
                let _lang = match &kind {
                    CodeBlockKind::Fenced(i) => i.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                g.raw(&format!("\\pard\\plain\\fi0\\li200 \\sb120\\sa120 \\f1\\fs{} \\chshdng100\\chcbpat{C_CODEBG} \\cf{C_CODE} ", g.fs() - 2));
                g.para_open = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                code_block = false;
                g.raw("\\chshdng0\\chcbpat0 \\cf1 \\par\n");
                g.para_open = false;
            }
            Event::Start(Tag::Emphasis) => g.em += 1,
            Event::End(TagEnd::Emphasis) => g.em -= 1,
            Event::Start(Tag::Strong) => g.strong += 1,
            Event::End(TagEnd::Strong) => g.strong -= 1,
            Event::Start(Tag::Strikethrough) => g.strike += 1,
            Event::End(TagEnd::Strikethrough) => g.strike -= 1,
            Event::Start(Tag::Link { dest_url, .. }) => {
                g.ensure_para();
                g.link += 1;
                // 结构：{\field{\*\fldinst{HYPERLINK "url"}}{\fldrslt 内容}}
                g.raw("{\\field{\\*\\fldinst{HYPERLINK \"");
                g.raw(&escape_url(&dest_url));
                g.raw("\"}}");
                g.raw(&format!("{{\\fldrslt\\f0\\fs{} \\cf{C_LINK} \\ul ", g.fs()));
            }
            Event::End(TagEnd::Link) => {
                g.raw(&format!("\\ulnone \\cf{C_TEXT}"));
                g.raw("}}");
                g.link -= 1;
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                // MVP 不加载图片：输出占位文本，结构可见但不破坏排版
                g.ensure_para();
                g.raw(&format!("\\cf{C_FADE} "));
                let alt = if title.is_empty() { format!("[图片:{dest_url}]") } else { format!("[图片:{title}]") };
                g.text(&alt);
                g.raw(&format!(" \\cf{C_TEXT} "));
            }
            Event::End(TagEnd::Image) => {}
            Event::Start(Tag::Table(al)) => {
                aligns = al;
                rows = Some(Vec::new());
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => cur.clear(),
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if let Some(r) = &mut rows {
                    r.push(std::mem::take(&mut cur));
                }
            }
            Event::Start(Tag::TableCell) => {
                g.in_cell = true;
                g.cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                g.in_cell = false;
                cur.push(std::mem::take(&mut g.cell));
            }
            Event::End(TagEnd::Table) => {
                if let Some(r) = rows.take() {
                    g.flush_table(Table { aligns: std::mem::take(&mut aligns), rows: r });
                }
            }
            Event::SoftBreak => {
                if !code_block {
                    g.raw(" ");
                }
            }
            Event::HardBreak => g.raw(if code_block { "\\par " } else { "\\line " }),
            Event::Rule => {
                g.raw(&format!("\\pard\\plain\\fi0\\li0 \\sb240\\sa240 \\brdrb\\brdrs\\brdrw10\\brsp20 \\cf{C_RULE}\\par\\cf1 \\n"));
                g.para_open = false;
            }
            Event::Text(t) => {
                if code_block {
                    for (i, l) in t.split('\n').enumerate() {
                        if i > 0 {
                            g.raw(&format!("\\par \\f1\\fs{} \\chshdng100\\chcbpat{C_CODEBG} \\cf{C_CODE} ", g.fs() - 2));
                        }
                        g.text(l);
                    }
                } else {
                    g.styled(&t);
                }
            }
            Event::Code(c) => {
                g.code = true;
                g.styled(&c);
                g.code = false;
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                // 第一版不解析内嵌 HTML：按正文转义，保证不破坏 RTF 结构
                let s = h.trim();
                if !s.is_empty() {
                    g.styled(s);
                }
            }
            Event::TaskListMarker(done) => {
                g.ensure_para();
                g.raw(if done { "\\u9746? " } else { "\\u9744? " });
            }
            Event::FootnoteReference(id) => {
                g.ensure_para();
                g.raw(&format!("\\super \\cf{C_LINK}[^"));
                g.text(&id);
                g.raw(&format!("]\\nosuper \\cf{C_TEXT} "));
            }
            Event::InlineMath(m) | Event::DisplayMath(m) => g.styled(&m),
            Event::Start(_) | Event::End(_) => {}
        }
    }
    g.end_para();
    g.raw("\n}");
    Out { rtf: g.b, nodes: g.nodes }
}

/// URL 出现在 \fldinst 的字符串字面量内：转义引号/大括号/反斜杠，控制字符换空格
fn escape_url(u: &str) -> String {
    let mut o = String::with_capacity(u.len());
    for ch in u.chars() {
        match ch {
            '"' | '{' | '}' | '\\' => {
                o.push('\\');
                o.push(ch);
            }
            c if (c as u32) < 0x20 => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

pub fn url_escape(u: &str) -> String {
    escape_url(u)
}
