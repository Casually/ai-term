# 🤖 ai-term

> 一个轻量级、高性能的 Rust CLI 程序，为您的 macOS 和 Linux 终端注入大语言模型 (LLM) 的灵魂。用自然语言驱动您的日常运维操作。

[![Written in Rust](https://img.shields.io/badge/Written_in-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

---

## ✨ 核心特性

- 🗣 **自然语言转命令**：无需记忆繁琐的 `tar`, `find`, `sed` 参数，输入人类语言，直接生成绝对准确的 Shell 命令。
- 🧠 **Autonomous Agent (自动执行)**：AI 会根据命令的 `stdout`/`stderr` 结果自我纠错和连续执行，直至目标达成。
- 🛡️ **敏感操作安全拦截**：对增、删、改、查等改变系统状态的命令自动挂起，强制等待用户 `[y/N]` 确认，安全第一。
- 🌐 **内置 SSH 远程联控**：输入 `/ssh user@host` 瞬间切换上下文，将 AI 的思考与执行战场无缝转移至远程服务器。
- 🚀 **极致 Rust 性能**：零运行时依赖，单文件极速启动。

## 💡 为什么需要 ai-term？

原生的 `bash` 或 `zsh` 极难完美拦截类似 `Shift+Enter` 的快捷键并注入多行输出。
`ai-term` 采用 **REPL (Read-Eval-Print Loop)** 架构作为一个“套壳终端”运行。你只需在原有终端中启动它，即可获得一个全功能的、带有 AI 助手的子终端环境，完全不影响您现有的别名和环境变量。

---

## 🚀 快速开始

### 1. 下载或编译安装

你可以通过克隆仓库并使用 Cargo 自行编译：

```bash
# 克隆仓库
git clone https://github.com/Casually/ai-term.git
cd ai-term

# 编译并运行
cargo run --release
```

### 2. 配置文件配置

首次运行程序时，它会在你的家目录下自动生成全局配置文件：`~/.ai-term/config.toml`。（程序同样支持读取当前运行目录下的 `config.toml` 进行覆盖）。

请使用文本编辑器打开它，并配置你喜欢的 LLM 驱动：

#### 推荐方案：使用聚合 API (丰小子 OpenRouter)
国内免梯直连，极速访问全球顶尖模型。
👉 [前往获取 API Key](https://openrouter.fengxiaozi.net/)

```toml
[llm]
provider = "openai"
api_key = "sk-你的丰小子API_KEY"
base_url = "https://openrouter.fengxiaozi.net/v1"
model = "anthropic/claude-3.5-sonnet"
```

#### 备选方案 A：Gemini 原生接口
```toml
[llm]
provider = "gemini"
api_key = "你的_GEMINI_API_KEY"
base_url = "https://generativelanguage.googleapis.com"
model = "gemini-2.5-flash"
```

#### 备选方案 B：本地大模型 (如 Ollama)
```toml
[llm]
provider = "openai"
api_key = "ollama"
base_url = "http://localhost:11434/v1"
model = "llama3"
```

---

## 🎮 使用方法

进入 `ai-term ❯` 提示符后，您的终端即已获得超能力。

1. **输入意图**：输入自然语言，例如：`帮我找出当前目录下体积最大的前3个文件`
2. **召唤 AI**：按下 `Shift + Enter`。（*注意：部分老式终端模拟器如 macOS 默认的 Terminal.app 不区分 Enter 和 Shift+Enter，此时请使用备用快捷键 `Ctrl + G`*）。
3. **Agent 自动执行**：程序会进入自动规划模式：
   - 打印 AI 的思考过程（暗色显示）。
   - 打印要执行的命令。
   - 如果是普通查询命令，自动执行并分析结果；如果是敏感命令，会提示 `⚠️ 这是一个敏感命令。是否允许执行？[y/N]:`，等待您的授权。
4. **完成总结**：任务达成后，AI 会给出一句友好的总结并退回普通输入模式。

### 强大的内置指令
- `/ssh <user>@<host>[:port]`：连接到远程服务器。此后的所有普通命令和 AI Agent 操作都将针对该远程服务器执行。
- `/exit`：断开 SSH 连接（如果在远程模式）或退出 `ai-term`。
- `上下方向键`：浏览历史命令。

---

## 📦 一键打包与交叉编译

项目中内置了 `build.sh` 脚本，支持在 macOS 上一键编译出适用于 macOS 和 Linux 架构的二进制文件。

```bash
bash build.sh
```

- 该脚本依赖本地 Homebrew 安装 `x86_64-linux-musl-gcc` 工具链。
- 编译产物会存放在项目根目录的 `dist/` 文件夹下。
- 若需清理编译缓存，可执行 `cargo clean` 或 `rm -rf target/ dist/`。

---

## 📄 License

MIT License. Crafted with ❤️ and Rust.