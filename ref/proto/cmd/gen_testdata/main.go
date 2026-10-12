package main

// 生成 P1 测试集：尺寸 × 三种内容分布。中文环境 utf-8。
// 用法: go run ./cmd/gen_testdata [输出目录]

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	en = "the quick brown fox jumps over the lazy dog and a simple markdown document with links code and table cell content for wrapping tests"
	zh = "这是一个用于中文长文档排版测试的段落，包含一些链接、代码和表格单元格内容，用于验证分行与换行行为"
)

func norm() string {
	return normPara(18, 6)
}

func normPara(words int, para int) string {
	enW := strings.Split(en, " ")
	zhW := strings.Split(zh, "")
	var b strings.Builder
	for p := 0; p < para; p++ {
		if p%2 == 0 {
			b.WriteString(strings.Join(enW[:words%len(enW)], " "))
		} else {
			b.WriteString(strings.Join(zhW, ""))
		}
		b.WriteString("\n\n")
	}
	return b.String()
}

func structDense() string {
	var b strings.Builder
	for i := 0; i < 200; i++ {
		fmt.Fprintf(&b, "## 章节 %d\n\n", i)
		b.WriteString("- 项目甲 描述文本\n- 项目乙 更多描述\n- 项目丙\n  - 子项一\n  - 子项二\n\n")
		b.WriteString("> 引用块内容用于测试，有中文和英文混合 English mixed in。\n\n")
		b.WriteString("| 列A | 列B | 列C |\n| --- | --- | --- |\n| a1 | b1 | c1 |\n| a2 | [链](https://e.com) | 中文 |\n\n")
		b.WriteString("[外部链接](https://example.com/p) 一行链接文本。\n\n")
	}
	return b.String()
}

func codeDense() string {
	var b strings.Builder
	for i := 0; i < 300; i++ {
		b.WriteString("```go\n")
		for j := 0; j < 40; j++ {
			fmt.Fprintf(&b, "func fn_%d_%d(x int, s string) error {\n\tif x > %d { return fmt.Errorf(\"bad %%d\", x) }\n\t_ = s\n\treturn nil\n}\n", i, j, j)
		}
		b.WriteString("```\n\n```python\n")
		for j := 0; j < 15; j++ {
			fmt.Fprintf(&b, "def foo_%d(a, b):\n    return a + b * %d  # comment line for tokenizer\n", j, j)
		}
		b.WriteString("```\n\n")
	}
	return b.String()
}

var dists = map[string]func() string{
	"norm":   norm,
	"struct": structDense,
	"code":   codeDense,
}

func main() {
	outDir := "testdata/large"
	if len(os.Args) > 1 {
		outDir = os.Args[1]
	}
	os.MkdirAll(outDir, 0o755)
	for _, size := range []int{1, 10, 32, 64, 128} {
		for name, gen := range dists {
			target := int64(size) * 1024 * 1024
			fn := filepath.Join(outDir, fmt.Sprintf("md_%dMiB_%s.md", size, name))
			f, err := os.Create(fn)
			if err != nil {
				panic(err)
			}
			var wrote int64
			for wrote < target {
				b := gen()
				n, _ := f.WriteString(b)
				wrote += int64(n)
			}
			f.Close()
			fmt.Printf("wrote %s (%.1f MiB)\n", fn, float64(wrote)/1024/1024)
		}
	}
}