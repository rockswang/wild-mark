# AGENTS.md

## 项目标识

- **正式名称**：**wild-markdown**（Markdown 编辑器 / 查看器）
- **Go module**：`github.com/rockswang/wild-markdown`
- **远程仓库**：https://github.com/rockswang/wild-markdown （默认分支 `main`）
- **本地工作目录**：`D:\work\wild-edit`（目录名沿用历史，勿据此改项目名）
- 曾用名 `mdviewer` / `wild-edit` 已废弃，新代码、构建产物、文档一律使用 **wild-markdown**。

### 仓库结构约定

| 路径 | 说明 |
|---|---|
| `./` | **正式版** Go module（`github.com/rockswang/wild-markdown`），根目录保持整洁 |
| `docs/` | 需求 `input.md`、研究报告 `P0/P1/P2/IME_report.md`、决策 `B_decisions.md`、交接 `HANDOFF.md` |
| `testdata/` | 测试数据；`large/` 为可再生的超大文件（不入库） |
| `ref/` | **参考与原型归档**，正式版不从这里 import。见 `ref/README.md` |
| `ref/proto/` | 冻结的原型模块（独立 go.mod），经根 `go.work` 一起构建 |
| `temp/` | 本地临时文件（不入库，勿用系统临时目录） |

### 依赖与版本墙

- `go-edit v0.11.0` 锁定 `go-gui v0.78.0` + `go-glyph v1.25.2`；go-gui latest（v0.87.0）会因 `Border` API 变化编译失败。
- 本地已给 go-gui / go-edit 打补丁（IME 目标判定、编辑器最大行宽缓存），补丁源码不在库内，需按 `ref/README.md` 重建。
- 正式版开发**不得直接修改供应商源码**；如需补丁，经 `ref/` + `go.work replace` 注入。

----------

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

### Python偏好
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
* Token：见本地全局 `~/.pi/agent/AGENTS.md` 的「个人凭据」节，或环境变量 `GH_TOKEN`；**不得写入本仓库任何文件**。
* 注意：该 token 为仓库所有者明文授权的凭据，仅用于 rockswang 相关仓库的 git 推送/API 调用；不要向任何第三方暴露。
* 推送方式（避免 token 落盘到 `.git/config`）：
  ```bash
  git push "https://rockswang:$GH_TOKEN@github.com/rockswang/wild-markdown.git" main:main
  ```
