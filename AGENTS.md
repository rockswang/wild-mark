# AGENTS.md

## 项目标识

- **正式名称**：**wild-mark**（Windows 原生轻量 Markdown 编辑器 / 查看器）
- **技术路线**：Rust + Win32 原生窗口 + Scintilla（编辑）+ RichEdit（预览）；**不使用** WebView2 / Chromium / 跨平台 GUI 框架 / Go
- **仓库**：https://github.com/rockswang/wild-mark （默认分支 `main`）
- **本地工作目录**：`D:\work\wild-edit`（目录名沿用历史，勿据此改项目名）
- 历史 Go 路线（`wild-markdown` 原型）已归档在 `ref/`，**不再维护，不得作为实现参考**。

### 仓库结构约定

| 路径 | 说明 | 是否入库 |
|---|---|---|
| `./` | 正式版 Rust crate（`Cargo.toml` + `src/`），根目录保持整洁 | ✅ |
| `src/` | `win.rs`(Win32) `metrics.rs`(自采样) `harness.rs`(场景) `scintilla.rs` `md2rtf.rs` `richedit.rs` + `bin/` 探针 | ✅ |
| `tests/` | RTF 转义与结构回归测试 | ✅ |
| `scripts/` | `gen_sci_const.py`(常量生成) `run_task.py`(交互桌面投递) `bmp_stats.py` `rtf_check.py` | ✅ |
| `docs/` | 需求/报告；**默认全部排除，白名单逐篇确认后提交** | ⚠️ 白名单 |
| `native/` | Scintilla/Lexilla 源码与构建产物，`build_native.bat` 可再生 | ❌ |
| `ref/` | Go 路线归档、上游参考源码与补丁 | ❌ |
| `testdata/` | 测试数据；`large/` 可再生 | ❌ |
| `temp/` | 本地临时文件（不入库，勿用系统临时目录） | ❌ |
| `artifacts/` | 测量产物（JSONL / 截图 / 模块清单） | ❌ |

### 依赖与版本墙

- `Scintilla 5.15` + `Lexilla 5.30` 由 `build_native.bat`（MSVC nmake 官方 makefile）构建，**不要直接改 `native/` 下上游源码**；需要补丁时另存补丁脚本并在报告中记录。
- `windows` crate 锁 0.58（0.6x 系列 API 有破坏性调整）；`pulldown-cmark` 锁 0.13。
- `Msftedit.dll`（RichEdit 4.1）为系统组件，预览控件**只读**使用。

### Scintilla 集成要点（原型期已踩坑）

1. Scintilla 5.x 核心**无内置 lexer**：只能用 `SCI_SETILEXER(0, Lexilla::CreateLexer("markdown"))` 或容器自染（`SCN_STYLENEEDED` + `SCI_SETSTYLING`）；`SCI_SETLEXER` 消息在 5.x 已不存在。
2. 64 位下 `ScintillaDirect2Call` 不可用，FFI 统一 `SendMessageW`；`Scintilla.dll` 的 `DllMain` 会自注册窗口类 `Scintilla`。
3. 消息号常量一律由 `scripts/gen_sci_const.py` 从头文件生成到 `src/sci_const.rs`（**禁止手抄**）；升级 Scintilla 后重跑该脚本。
4. TSF 中文输入**不产生 `WM_CHAR`**，输入处理请基于 `SCN_MODIFIED` / `SCN_CHARADDED`。

### 预览链路要点

- RTF 是**内存中间产物**：中文默认写 `\ansicpg936` + GBK 原始字节（体积 x1.10，生成快 3.9 倍），异常字符回退 `\u` 转义（`\uN?`，非 BMP 拆代理对）。转义全部收口在 `md2rtf.rs` 的 `text()`，**禁止把原文直接拼进控制串**。
- 整篇 `EM_STREAMIN` 刷新会**重置光标/选区**（滚动位置正常保持）→ 刷新前后必须 `EM_EXGETSEL` / `EM_EXSETSEL`。
- RichEdit 首次灌入有 ~200 ms 一次性开销 → 启动时预热控件，不要每次打开文件重建。
- 预览阈值默认 8 MiB：超阈值关闭实时预览（延迟解析 + 手动刷新）。

### 验证纪律（重要）

- **所有 GUI 测量必须投递到交互桌面执行**（`python scripts/run_task.py <exe> [args]`）。本 agent 的 bash 位于 session 0（非交互窗口站），在那里 `CreateWindowExW` 不会真正绘制，测出的内存/CPU/GDI 全部失真（实测空窗口私有工作集 0.6 MiB vs 41.9 MiB）。
- 内存口径统一报 **PrivateWorkingSetSize（私有工作集）**，并同时给出 WorkingSet；评价"轻量"时注意交互桌面会注入第三方输入法（如搜狗 IME 约 27 MiB 映像）与系统 UI 栈，属环境成本而非程序成本。
- 长任务（≥10 min 稳定性、≥1 GiB 文件）必须给出实测基线，不要用"应该可以"代替数据。
- 性能回归使用固定测试机、`--release`、重复测量；关键数据落盘到 `artifacts/*.summary.json` 与报告。

### 常用命令

```bash
build_native.bat                                    # Scintilla/Lexilla（MSVC nmake）
python scripts/gen_sci_const.py                     # 生成 src/sci_const.rs
cargo build --release
cargo test --release                                # RTF 回归（15 项）
target/release/gen_testdata.exe --large             # 生成 1~128 MiB 测试数据
python scripts/run_task.py target/release/p0_blank.exe --idle 60 --min 60 --drag 30
python scripts/run_task.py target/release/p1_scintilla.exe --bench-sizes 1,10,32,64,128
python scripts/run_task.py target/release/p2_preview.exe --bench-sizes 1,10,32,64,128 --compare-cp --stress 20
python scripts/run_task.py target/release/mem_probe.exe --mods --preview
python scripts/bmp_stats.py artifacts/p2.bmp        # 渲染像素级校验
```

---

## 工具

* 运行环境：中文 Windows 10 64位系统
* 除非特殊要求，严禁使用 Powershell cmdlet，包括powershell.exe和pwsh.exe，以避免字符编码、管道重定向等错误。
* 工具 bash 实际执行环境是 git-bash(MSYS2)，命令一律用 bash 语法，勿用 cmd 内部命令（如 `ver`、`%VAR%` 展开）；路径可用 `/c/...` 或 `C:/...`。
* 涉及 `cmd.exe //c` 调用时（如 reg query/add/delete）为避免中文乱码。**所有 cmd.exe //c 调用必须先执行 `chcp 65001 >nul` 切到 UTF-8 代码页**，即统一写成 `cmd.exe //c "chcp 65001>nul && 实际命令..."`。
* 使用 Windows 自带的 `C:/Windows/System32/curl.exe`，**不要使用 git-bash 自带的 curl**
* 使用 D:\dev\Git\cmd\ 目录中常用cli工具，而不是 Windows/bash 自带等效命令。
  - rg.exe: ripgrep 文件内查找
  - fd.exe: 文件查找
  - jq.exe: json处理
  - treex.exe: 目录树
  - rclone.exe: 目录复制
  - uv.exe: 管理python环境和模块
* JSON 读写优先用 jq.exe，不要习惯性起 python：**凡是「读字段/改字段/插列表项/格式化」这类单步 JSON 操作，一条 jq 即可，禁止用 python 脚本做 jq 能一行完成的事**。
  few-shot 示例（往 provider 的 models 数组追加一项，含备份）：
  ```bash
  cp models.json models.json.bak
  D:/dev/Git/cmd/jq.exe '.providers["wild-work"].models += [{"id":"qodercn/glm-5.3-flash","name":"GLM-5.3 Flash 0.1x","reasoning":true}]' models.json > models.json.tmp && mv models.json.tmp models.json
  D:/dev/Git/cmd/jq.exe -r '.providers["wild-work"].models[].id' models.json   # 回读验证
  ```
  仅当逻辑超出 jq 表达能力（复杂聚合/多文件事务/调库）时才用 python（venv sys312）。
* 本机有 python(uv), node.js(nvm), rust, go, msvc(仅构建工具链) 等编程语言/工具链。
* 除非特殊要求，编写快速验证、测试的脚本应使用 D:\dev\uv\ve\sys312 的 Python 3.12 虚拟环境编写、执行（即 `D:/dev/uv/ve/sys312/Scripts/python.exe`）。
* 第三方模块应使用 uv pip install 安装在上述虚拟环境。
* Python 脚本 stdout 涉及中文时，执行前先设 `PYTHONIOENCODING=utf-8`，否则 git-bash 下直接 print 中文会乱码；重要交付内容一律写入 UTF-8 文件，不依赖 stdout。
* 超过5行，或包含多层单/双引号，包含中文的脚本，应使用内置 write 工具落盘到本地再执行，而不是用 heredoc/echo 等方式。
* 如需访问海外资源，尽量使用国内镜像，或使用本地代理 http://127.0.0.1:10809
* 临时文件一律创建在当前工作目录的 `temp` 子目录（不存在则创建），**不要使用操作系统临时目录**（%TEMP%/C:/Users/.../AppData/Local/Temp、/tmp 等），避免访问工作区外文件触发频繁授权确认。此规则覆盖skill中的临时文件位置要求。

----------

## 编程风格

* 注意：本风格仅适用于面向生产交付的源代码、脚本、测试用例等，对于用于快速验证的脚本，不做强制要求，仅做风格参考。

### 不限语言编程偏好
* 工程实用主义，代码以解决具体数据解析/转换问题为中心，不追求过度抽象。
* 把核心逻辑写成较少的直接函数，流程清晰、线性推进，避免层层封装。
* 对性能和内存占用比较敏感，倾向使用 Rust 实现核心解析逻辑，再通过 Arrow 暴露给数据分析侧使用。
* 直接使用标准库和轻量级成熟模块，不轻易引入复杂、重量级框架。
* 公开 API 保持少而明确，内部辅助函数可保持私有。
* 核心实现和语言绑定分离：核心逻辑放在实现模块中，Python/PyO3 绑定放在 `lib.rs`。
* 注释和打印输出主要使用中文。
* 对关键、难点逻辑在行末直接写行内注释进行备忘。
* 用单行注释解释后续若干行的逻辑、数据结构、接口限制和特殊兼容逻辑。
* 用单行注释在复杂数据结构前说明形状，例如列表中元素、差分结果结构。
* 局部变量应简短、使用缩写简写，可通过行内注释标注用途。
* 常量使用大写，例如 HTTP_OPT、IBASE_OPT、UNSAFE_NODES。
* 提高代码的紧凑程度，在一行中编写较多的逻辑，通过提升代码密度实现编辑器可视范围内看到更多的逻辑以提升可理解性。
* 避免不必要的数据拷贝。
* Release 配置重视性能和体积，例如 `lto = true`、`codegen-units = 1`、`strip = true`、`panic = "abort"`。
* 对已知问题直接写 TODO 或说明实际限制。
* 对部分非关键解析失败会容忍重试或吞掉异常，保障主流程继续尝试。
* Fail Early，除非特别要求，不随意编写兜底逻辑，导致错误被掩盖或延后。

### Rust 偏好
* FFI 边界保持薄：结构体与消息号常量自动生成（见 `scripts/gen_sci_const.py`），手写部分只做类型安全封装。
* 出错路径优先 `Result` + `?`；原型探针可 panic，正式代码不得静默吞错。
* 热路径避免分配：写文本/RTF 一律往预分配 `Vec<u8>` 追加，不要拼 `String` 中间量。
* `unsafe` 块必须附中文注释说明前置条件（指针来源、生命周期、线程假设）。

### Python 偏好
* 函数式组织，除非需要保存状态或封装通用能力，否则不主动引入复杂类层次。
* 通用状态能力才抽成类，例如 TaskManager、Timer、Reqx。
* 对性能比较敏感，倾向批量处理、集合运算、分页拉取、多线程请求、基准测试。
* 使用 f-string、列表/字典/集合推导式、三元表达式、`:=`海象运算符来减少临时变量。
* 使用紧凑的代码，例如单行 if、单行 return、单行 except ValueError: pass。必要时用`# noqa`局部压制 lint。
* 使用列表/集合/字典推导式以及生成器表达式，取代简单的循环。
* 推导式中可使用赋值表达式，伪逗号表达式（通过元组实现）来进行局部变量的赋值，提升推导式的表达能力，如 `[v for k in lst if (v := foo(k),)]`。
* 局部逻辑可使用内部函数封装，例如数据标准化、行过滤、差异判断。
* 简单数据容器倾向使用 @dataclass，namedtuple等。
* Docstring 不追求全覆盖，只在通用函数或复杂函数上补充参数和返回值说明。

-----------

## 个人凭据

* 敏感数据，仅可使用，除非要求，否则勿落盘文档；**严禁提交进本仓库**（GitHub Push Protection 会拦截）。

### Github
* 账号：rockswang
* Token：见本地全局 `~/.pi/agent/AGENTS.md` 的「个人凭据」节；**不得写入本仓库任何文件**（Push Protection 会拦截）。
* 注意：此 token 为仓库所有者明文授权的凭据，仅用于 rockswang 相关仓库的 git 推送/API 调用；不要向任何第三方暴露。
* **推送**：已配置到本地 `.git/config` 的 origin URL（不在版本控制内，不会被扫描），直接 `git push` / `git fetch` 即可，无需手动带 token。
  - 查看：`git remote -v`；改回纯净 URL：`git remote set-url origin https://github.com/rockswang/wild-mark.git`
