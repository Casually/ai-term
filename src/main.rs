use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::io::{stdout, Write};
use std::process::Command;
use std::path::Path;
use ssh2::Session;
use std::net::TcpStream;
use std::io::Read;

/// 执行环境模式
enum ExecutionMode {
    Local,
    Remote {
        session: Session,
        prompt: String,
    },
}

impl ExecutionMode {
    fn prompt(&self) -> &str {
        match self {
            ExecutionMode::Local => "🤖 ai-term ❯ ",
            ExecutionMode::Remote { prompt, .. } => prompt,
        }
    }
}

/// 建立 SSH 连接
fn connect_ssh(target: &str) -> Result<Session, String> {
    let mut parts = target.split('@');
    let user = parts.next().unwrap_or("");
    let host_port = parts.next().unwrap_or("");
    
    let user = if host_port.is_empty() {
        return Err("格式错误，需为 user@host[:port]".to_string());
    } else {
        user
    };
    
    let mut hp_parts = host_port.split(':');
    let host = hp_parts.next().unwrap_or("");
    let port = hp_parts.next().unwrap_or("22");
    
    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect(&addr).map_err(|e| format!("TCP连接失败: {}", e))?;
    
    let mut sess = Session::new().map_err(|e| format!("创建SSH Session失败: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH握手失败: {}", e))?;
    
    // Auth
    // Try agent
    if sess.userauth_agent(user).is_ok() {
        return Ok(sess);
    }
    
    // Try pubkey
    let mut auth_ok = false;
    if let Some(mut home) = dirs::home_dir() {
        home.push(".ssh");
        let id_rsa = home.join("id_rsa");
        let id_ed25519 = home.join("id_ed25519");
        
        if id_rsa.exists() && sess.userauth_pubkey_file(user, None, &id_rsa, None).is_ok() {
            auth_ok = true;
        } else if id_ed25519.exists() && sess.userauth_pubkey_file(user, None, &id_ed25519, None).is_ok() {
            auth_ok = true;
        }
    }
    
    if auth_ok {
        Ok(sess)
    } else {
        Err("SSH 认证失败（暂仅支持 ssh-agent 和默认无密码私钥 ~/.ssh/id_rsa, ~/.ssh/id_ed25519）".to_string())
    }
}

/// 统一执行命令，返回 (stdout, stderr)
fn execute_command(mode: &ExecutionMode, cmd: &str) -> Result<(String, String), String> {
    match mode {
        ExecutionMode::Local => {
            let output = Command::new("sh").arg("-c").arg(cmd).output().map_err(|e| e.to_string())?;
            let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((stdout_str, stderr_str))
        }
        ExecutionMode::Remote { session, .. } => {
            let mut channel = session.channel_session().map_err(|e| e.to_string())?;
            channel.exec(cmd).map_err(|e| e.to_string())?;
            
            let mut stdout_str = String::new();
            channel.read_to_string(&mut stdout_str).unwrap_or(0);
            
            let mut stderr_str = String::new();
            channel.stderr().read_to_string(&mut stderr_str).unwrap_or(0);
            
            channel.wait_close().map_err(|e| e.to_string())?;
            
            Ok((stdout_str, stderr_str))
        }
    }
}

/// 配置文件对应的数据结构
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    /// LLM 服务商标识，支持 "gemini" 或 "openai"
    pub provider: String,
    /// LLM 接口地址 (Base URL)
    pub api_url: String,
    /// 接口访问密钥 (Key)
    pub api_key: String,
    /// 调用的模型名称
    pub model: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AgentResponse {
    thought: String,
    command: String,
    is_sensitive: bool,
    is_done: bool,
    #[serde(default)]
    summary: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

/// 从本地目录或系统配置目录加载配置文件
fn load_or_create_config() -> Result<Config, String> {
    // 1. 优先尝试读取当前工作目录下的 config.toml (方便本地调试与分发)
    let local_config_path = Path::new("config.toml");
    if local_config_path.exists() {
        let content = std::fs::read_to_string(local_config_path)
            .map_err(|e| format!("读取本地 config.toml 失败: {}", e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("解析本地 config.toml 失败，请检查语法。错误: {}", e))?;
        
        if config.api_key == "YOUR_API_KEY_HERE" || config.api_key.trim().is_empty() {
            return Err("检测到本地 config.toml。请先在其中填入真实的 API Key 再试一次！".to_string());
        }
        return Ok(config);
    }

    // 2. 如果本地没有，则尝试使用系统全局配置目录 (例如 ~/.config/ai-term/config.toml)
    let mut global_config_path = dirs::config_dir()
        .ok_or_else(|| "无法获取当前系统的用户配置目录".to_string())?;
    
    global_config_path.push("ai-term");
    std::fs::create_dir_all(&global_config_path).map_err(|e| format!("无法创建配置目录: {}", e))?;
    global_config_path.push("config.toml");

    // 如果全局配置文件也不存在，则自动生成一份默认模板文件
    if !global_config_path.exists() {
        let default_config = Config {
            provider: "gemini".to_string(),
            api_url: "https://generativelanguage.googleapis.com".to_string(),
            api_key: "YOUR_API_KEY_HERE".to_string(),
            model: "gemini-2.5-flash".to_string(),
        };
        
        let toml_str = toml::to_string_pretty(&default_config)
            .map_err(|e| format!("序列化默认配置失败: {}", e))?;
        
        std::fs::write(&global_config_path, toml_str)
            .map_err(|e| format!("写入配置文件失败: {}", e))?;
        
        return Err(format!(
            "首次运行：未检测到本地或全局配置。已自动生成默认全局配置文件，请编辑该文件并填入您的 API 密钥与地址！\n👉 配置文件路径: {}",
            global_config_path.to_string_lossy()
        ));
    }

    // 读取并解析已有的全局配置文件
    let content = std::fs::read_to_string(&global_config_path)
        .map_err(|e| format!("读取全局配置文件失败: {}", e))?;
    
    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("解析全局 config.toml 失败，请检查 TOML 语法格式。错误信息: {}", e))?;

    if config.api_key == "YOUR_API_KEY_HERE" || config.api_key.trim().is_empty() {
        return Err(format!(
            "请在全局配置文件中填入真实的 API Key 再试一次:\n👉 {}",
            global_config_path.to_string_lossy()
        ));
    }

    Ok(config)
}

/// 根据配置调用对应的 LLM 接口，使用 Agent 模式
async fn ask_llm_agent(client: &Client, config: &Config, history: &[Message]) -> Result<AgentResponse, String> {
    let system_prompt = r#"你是一个 macOS/Linux 终端 Agent。用户将提供一个最终目标，你需要根据目标一步步规划并提供 Shell 命令执行。
规则：
1. 你的返回必须是合法的 JSON 格式。非常重要：在 JSON 字符串字段中，严禁包含未经转义的双引号（"）和换行符（\n）。所有的双引号必须转义为 \"，或者改用单引号。
2. 每次只提供一个最合适的命令，我会执行它并把输出返回给你。
3. 如果目标已经完成，设置 is_done 为 true，command 留空，并在 summary 字段中提供最终的任务总结或结果汇报给用户。
4. 极其重要：如果命令涉及到增、删、改、查等任何可能改变系统状态或读取数据的操作，必须将 is_sensitive 设置为 true。这包括但不限于：
   - 增 (创建): mkdir, touch, cp, curl/wget 下载文件等
   - 删 (删除): rm, rmdir, 卸载软件等
   - 改 (修改): mv, vim/nano/echo 等写入文件, chmod, chown, 安装软件等
   - 查 (查询): cat/less/grep 查看文件内容, ls/find 浏览目录, ps/top 查看进程等
   注意：由于要求严格，绝大多数命令（除非是极其简单的无副作用内部命令，如 echo "hello"）都应该被视为敏感命令，需要手动确认。
返回 JSON 格式如下：
{
  "thought": "你当前步骤的思考过程和接下来的计划（切记内部不要有未经转义的双引号！）",
  "command": "当前需要执行的单个完整的 Shell 命令（如果已完成则留空）",
  "is_sensitive": false,
  "is_done": false,
  "summary": "当 is_done 为 true 时，在这里填写最终任务完成的总结说明，否则留空"
}
注意：只返回 JSON，不要包含任何前缀或 Markdown 代码块（不要输出 ```json）。"#;

    let provider = config.provider.to_lowercase();
    
    let text = if provider == "gemini" {
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            config.api_url.trim_end_matches('/'),
            config.model,
            config.api_key
        );
        let mut contents = Vec::new();
        for (i, msg) in history.iter().enumerate() {
            let mut content = msg.content.clone();
            if i == 0 && msg.role == "user" {
                content = format!("{}\n\n用户目标: {}", system_prompt, content);
            }
            let role = if msg.role == "assistant" { "model" } else { "user" };
            contents.push(json!({
                "role": role,
                "parts": [{"text": content}]
            }));
        }
        let body = json!({
            "contents": contents
        });

        let res = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;
        if res.status().is_success() {
            let json_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            if let Some(t) = json_res["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                t.to_string()
            } else {
                return Err("Gemini 返回格式解析失败".to_string());
            }
        } else {
            return Err(format!("API 请求失败 (HTTP {}): {:?}", res.status(), res.text().await));
        }
    } else {
        let url = format!("{}/v1/chat/completions", config.api_url.trim_end_matches('/'));
        let mut messages = vec![json!({"role": "system", "content": system_prompt})];
        for msg in history {
            messages.push(json!({"role": msg.role, "content": msg.content}));
        }
        let body = json!({
            "model": config.model,
            "messages": messages,
            "temperature": 0.1
        });

        let res = client.post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let json_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            if let Some(t) = json_res["choices"][0]["message"]["content"].as_str() {
                t.to_string()
            } else {
                return Err("OpenAI 兼容格式解析失败".to_string());
            }
        } else {
            return Err(format!("API 请求失败 (HTTP {}): {:?}", res.status(), res.text().await));
        }
    };

    let mut clean_text = text.replace("```json", "").replace("```", "").trim().to_string();
    
    // 尝试简单的修复：如果大模型在 JSON 内部使用了未经转义的换行符
    clean_text = clean_text.replace("\n", " ");
    
    // 如果无法直接解析，尝试使用简单的正则或容错方式提取字段
    let agent_res: AgentResponse = match serde_json::from_str(&clean_text) {
        Ok(res) => res,
        Err(e) => {
            // 提供容错降级：尝试使用字符串查找来提取
            let extract_field = |key: &str| -> Option<String> {
                let key_str = format!("\"{}\":", key);
                let start = clean_text.find(&key_str)? + key_str.len();
                let substr = &clean_text[start..];
                let first_quote = substr.find('"')?;
                let mut end_quote = first_quote + 1;
                while end_quote < substr.len() {
                    if substr[end_quote..].starts_with('"') && !substr[..end_quote].ends_with('\\') {
                        break;
                    }
                    end_quote += 1;
                }
                Some(substr[first_quote+1..end_quote].to_string())
            };

            let extract_bool = |key: &str| -> Option<bool> {
                let key_str = format!("\"{}\":", key);
                let start = clean_text.find(&key_str)? + key_str.len();
                let substr = clean_text[start..].trim_start();
                if substr.starts_with("true") {
                    Some(true)
                } else if substr.starts_with("false") {
                    Some(false)
                } else {
                    None
                }
            };

            let thought = extract_field("thought").unwrap_or_else(|| "无法解析 thought 字段".to_string());
            let command = extract_field("command").unwrap_or_default();
            let summary = extract_field("summary").unwrap_or_default();
            let is_sensitive = extract_bool("is_sensitive").unwrap_or(true); // 默认认为是敏感的以保证安全
            let is_done = extract_bool("is_done").unwrap_or(false);

            // 只有当至少提取到了 command 或者 is_done 时，我们才认为降级提取有效
            if !command.is_empty() || is_done {
                AgentResponse {
                    thought,
                    command,
                    is_sensitive,
                    is_done,
                    summary,
                }
            } else {
                return Err(format!("解析 Agent JSON 失败: {}\n原始内容: {}", e, text));
            }
        }
    };
    
    Ok(agent_res)
}

/// 重绘当前输入行
fn render_line(prompt_text: &str, input_buffer: &str) -> std::io::Result<()> {
    let mut stdout = stdout();
    execute!(
        stdout,
        Clear(ClearType::CurrentLine),
        cursor::MoveToColumn(0),
        SetForegroundColor(Color::Green),
        Print(prompt_text),
        ResetColor,
        Print(input_buffer)
    )?;
    stdout.flush()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化并读取配置
    let config = match load_or_create_config() {
        Ok(cfg) => cfg,
        Err(msg) => {
            eprintln!("\n{}", "-".repeat(60));
            eprintln!("{}", msg);
            eprintln!("{}\n", "-".repeat(60));
            std::process::exit(1);
        }
    };

    let client = Client::new();
    let mut input_buffer = String::new();
    let mut cmd_history: Vec<String> = Vec::new();
    let mut history_index: Option<usize> = None;
    let mut execution_mode = ExecutionMode::Local;

    println!("{}", "=".repeat(60));
    println!("🚀 AI 终端增强已启动 (OS: {})", env::consts::OS);
    println!("⚙️  当前配置: [{}] -> {}", config.provider, config.model);
    println!("💡 提示: 输入自然语言需求，按下 Shift+Enter 或 Ctrl+G 触发 AI");
    println!("🔌 提示: 输入 /ssh user@host[:port] 切换到远程服务器模式，/exit 退出");
    println!("退出请按 Ctrl+C");
    println!("{}\n", "=".repeat(60));

    enable_raw_mode()?;
    render_line(execution_mode.prompt(), &input_buffer)?;

    loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                // 退出键：Ctrl+C
                if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                    disable_raw_mode()?;
                    println!("\n再见！");
                    break;
                }

                // 触发 AI 的快捷键：Shift+Enter 或 Ctrl+G
                let is_ai_trigger = (code == KeyCode::Enter && modifiers.contains(KeyModifiers::SHIFT))
                    || (code == KeyCode::Char('g') && modifiers.contains(KeyModifiers::CONTROL));

                if is_ai_trigger {
                    if input_buffer.trim().is_empty() { continue; }

                    disable_raw_mode()?;
                    print!("\r\n");
                    execute!(stdout(), cursor::MoveToColumn(0), SetForegroundColor(Color::Cyan), Print("🚀 开启 Agent 自动执行模式..."), ResetColor)?;
                    stdout().flush()?;
                    print!("\r\n");

                    let mut history = vec![Message { role: "user".to_string(), content: input_buffer.clone() }];

                    loop {
                        execute!(stdout(), cursor::MoveToColumn(0), SetForegroundColor(Color::Cyan), Print("⏳ AI 正在思考..."), ResetColor)?;
                        stdout().flush()?;

                        match ask_llm_agent(&client, &config, &history).await {
                            Ok(res) => {
                                // 清除“AI 正在思考...”
                                execute!(stdout(), Clear(ClearType::CurrentLine), cursor::MoveToColumn(0))?;
                                
                                // 用暗色 (DarkGrey) 显示 AI 的思考过程
                                execute!(stdout(), SetForegroundColor(Color::DarkGrey), Print(format!("🧠 思考: {}\r\n", res.thought)), ResetColor)?;
                                
                                if res.is_done {
                                    execute!(stdout(), SetForegroundColor(Color::Green), Print("\r\n✅ 目标已完成！\r\n"), ResetColor)?;
                                    if !res.summary.is_empty() {
                                        execute!(stdout(), SetForegroundColor(Color::Cyan), Print(format!("✨ 总结: {}\r\n\r\n", res.summary)), ResetColor)?;
                                    }
                                    break;
                                }

                                if res.command.trim().is_empty() {
                                    execute!(stdout(), SetForegroundColor(Color::Yellow), Print("⚠️ AI 没有返回命令，结束循环。\r\n"), ResetColor)?;
                                    break;
                                }

                                // 敏感命令使用警告颜色显示
                                if res.is_sensitive {
                                    execute!(stdout(), SetForegroundColor(Color::Red), Print(format!("⚠️ 敏感命令: {}\r\n", res.command)), ResetColor)?;
                                } else {
                                    execute!(stdout(), SetForegroundColor(Color::Magenta), Print(format!("💻 命令: {}\r\n", res.command)), ResetColor)?;
                                }

                                if res.is_sensitive {
                                    execute!(stdout(), SetForegroundColor(Color::Yellow), Print("🛑 这是一个敏感命令。是否允许执行？[y/N]: "), ResetColor)?;
                                    stdout().flush()?;
                                    
                                    enable_raw_mode()?;
                                    let mut confirm = false;
                                    loop {
                                        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
                                            if let Event::Key(KeyEvent { code, .. }) = event::read().unwrap() {
                                                match code {
                                                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                                                        confirm = true;
                                                        break;
                                                    }
                                                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                                                        break;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                    disable_raw_mode()?;
                                    println!("\r");
                                    
                                    if !confirm {
                                        println!("🛑 用户已取消执行。\r");
                                        history.push(Message { role: "assistant".to_string(), content: serde_json::to_string(&res).unwrap_or_default() });
                                        history.push(Message { role: "user".to_string(), content: "用户拒绝执行该敏感命令，请重新规划或结束任务。".to_string() });
                                        continue;
                                    }
                                }

                                // 执行命令
                                match execute_command(&execution_mode, &res.command) {
                                    Ok((out_str, err_str)) => {
                                        if !out_str.is_empty() {
                                            println!("{}\r", out_str.replace('\n', "\r\n"));
                                        }
                                        if !err_str.is_empty() {
                                            eprintln!("{}\r", err_str.replace('\n', "\r\n"));
                                        }
                                        
                                        history.push(Message { role: "assistant".to_string(), content: serde_json::to_string(&res).unwrap_or_default() });
                                        
                                        let mut feedback = String::new();
                                        if !out_str.is_empty() {
                                            feedback.push_str(&format!("标准输出:\n{}\n", out_str));
                                        }
                                        if !err_str.is_empty() {
                                            feedback.push_str(&format!("标准错误:\n{}\n", err_str));
                                        }
                                        if feedback.is_empty() {
                                            feedback.push_str("命令执行成功，无输出内容。");
                                        }
                                        
                                        if feedback.len() > 2000 {
                                            feedback = format!("...{}", &feedback[feedback.len()-2000..]);
                                        }

                                        history.push(Message { role: "user".to_string(), content: feedback });
                                    }
                                    Err(e) => {
                                        let err_msg = format!("执行命令失败: {}", e);
                                        eprintln!("❌ {}\r", err_msg);
                                        
                                        history.push(Message { role: "assistant".to_string(), content: serde_json::to_string(&res).unwrap_or_default() });
                                        history.push(Message { role: "user".to_string(), content: err_msg });
                                    }
                                }
                            }
                            Err(e) => {
                                execute!(stdout(), Clear(ClearType::CurrentLine), cursor::MoveToColumn(0), SetForegroundColor(Color::Red), Print(format!("❌ AI 请求错误: {}\r\n", e)), ResetColor)?;
                                break;
                            }
                        }
                    }

                    input_buffer.clear();
                    enable_raw_mode()?;
                    render_line(execution_mode.prompt(), &input_buffer)?;
                    continue;
                }

                match code {
                    KeyCode::Enter => {
                        if input_buffer.trim().is_empty() {
                            println!("\r");
                            render_line(execution_mode.prompt(), &input_buffer)?;
                            continue;
                        }

                        // 处理特殊内置命令
                        if input_buffer.starts_with("/ssh ") {
                            let target = input_buffer.trim_start_matches("/ssh ").trim();
                            disable_raw_mode()?;
                            println!("\r\n⏳ 正在连接 {}...", target);
                            match connect_ssh(target) {
                                Ok(session) => {
                                    execution_mode = ExecutionMode::Remote {
                                        session,
                                        prompt: format!("🌐 {} ❯ ", target),
                                    };
                                    println!("✅ 连接成功！\r");
                                }
                                Err(e) => {
                                    println!("❌ 连接失败: {}\r", e);
                                }
                            }
                            input_buffer.clear();
                            enable_raw_mode()?;
                            render_line(execution_mode.prompt(), &input_buffer)?;
                            continue;
                        } else if input_buffer.trim() == "/exit" {
                            if let ExecutionMode::Remote { .. } = execution_mode {
                                disable_raw_mode()?;
                                println!("\r\n✅ 已断开远程连接\r");
                                execution_mode = ExecutionMode::Local;
                                input_buffer.clear();
                                enable_raw_mode()?;
                                render_line(execution_mode.prompt(), &input_buffer)?;
                                continue;
                            }
                        }

                        // 保存到历史记录中 (避免连续重复记录)
                        if cmd_history.last() != Some(&input_buffer) {
                            cmd_history.push(input_buffer.clone());
                        }
                        history_index = None;

                        disable_raw_mode()?;
                        println!("\r");

                        match execute_command(&execution_mode, &input_buffer) {
                            Ok((out, err)) => {
                                if !out.is_empty() {
                                    print!("{}", out.replace('\n', "\r\n"));
                                }
                                if !err.is_empty() {
                                    eprint!("{}", err.replace('\n', "\r\n"));
                                }
                            }
                            Err(e) => println!("❌ 执行命令失败: {}\r", e),
                        }

                        input_buffer.clear();
                        enable_raw_mode()?;
                        render_line(execution_mode.prompt(), &input_buffer)?;
                    }
                    KeyCode::Up => {
                        if cmd_history.is_empty() {
                            continue;
                        }
                        let new_index = match history_index {
                            Some(idx) => if idx > 0 { idx - 1 } else { 0 },
                            None => cmd_history.len() - 1,
                        };
                        history_index = Some(new_index);
                        input_buffer = cmd_history[new_index].clone();
                        render_line(execution_mode.prompt(), &input_buffer)?;
                    }
                    KeyCode::Down => {
                        if cmd_history.is_empty() {
                            continue;
                        }
                        if let Some(idx) = history_index {
                            if idx + 1 < cmd_history.len() {
                                history_index = Some(idx + 1);
                                input_buffer = cmd_history[idx + 1].clone();
                            } else {
                                history_index = None;
                                input_buffer.clear();
                            }
                            render_line(execution_mode.prompt(), &input_buffer)?;
                        }
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        render_line(execution_mode.prompt(), &input_buffer)?;
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        render_line(execution_mode.prompt(), &input_buffer)?;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    Ok(())
}
