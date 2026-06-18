AI-Term (Rust 智能终端增强)

一个轻量级的 Rust CLI 程序，为您的 macOS 和 Linux 终端注入 LLM 灵魂，通过自然语言自动补全运维操作。

核心痛点与解决方案

原生的 bash 或 zsh 拦截 Shift+Enter 是极其困难的。本程序采用 REPL (Read-Eval-Print Loop) 设计架构，作为一个套壳 Shell 运行。你只需在终端启动它，即可获得一个拥有 AI 增强的子终端环境。

配置文件 (NEW ✨)

程序支持 本地优先 配置，也可以选择全局配置：

本地配置：直接在项目根目录下创建 config.toml（推荐，便于单项目调试）。

全局配置：程序会在用户家目录下自动创建并读取：~/.config/ai-term/config.toml。

1. 采用 Gemini 原生接口配置

provider = "gemini"
api_url = "https://generativelanguage.googleapis.com"
api_key = "你的_GEMINI_API_KEY"
model = "gemini-2.5-flash"


2. 采用 OpenAI 兼容格式配置 (如 DeepSeek, OneAPI, Ollama 本地模型)

provider = "openai"
api_url = "https://api.deepseek.com" # 或本地 "http://localhost:11434"
api_key = "你的_DEEPSEEK_API_KEY"   # 或本地任意字符
model = "deepseek-chat"             # 或本地 "llama3" 等


编译与运行

首次直接运行程序：

cargo run --release


如果未检测到本地或全局配置，终端会输出配置文件所在的精确物理路径。请打开它，并将您首选的 LLM 信息配置好。

再次运行 cargo run --release 即可开启全新的 AI 终端体验！

使用示例

进入 ai-term ❯ 后：

输入自然语言：例如输入 解压当前目录下的 test.tar.gz 并显示进度。

触发 AI：按下 Shift + Enter。（重要提示：诸如 macOS 默认的 Terminal.app 等部分旧终端模拟器不区分 Enter 和 Shift+Enter，如果失效，请按下备用快捷键 Ctrl + G）。

确认命令：AI 思考后，输入框中的中文会被自动替换为：tar -zxvf test.tar.gz。

执行：按下普通的 Enter 键执行该命令，执行完毕后继续等待下一条指令。
