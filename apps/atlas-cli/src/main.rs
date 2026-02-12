use std::io::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, Level};

use atlas_core::execution::orchestrator::Orchestrator;
use atlas_core::execution::tool_registry::ToolRegistry;
use atlas_core::execution::tool_router::{NoOpSandbox, ToolRouter};
use atlas_core::model::{ModelProvider, ModelStream, StreamChunk};
use atlas_core::protocol::Message;
use atlas_protocol::{InboundMessage, OutboundMessage};

// =================================================================
// 1. 实现一个简单的 OpenAI 模型适配器
//    (由于 atlas-core/src/model 中只有接口，我们在 CLI 里实现具体逻辑)
// =================================================================
pub struct OpenAIProvider {
    pub api_key: String,
    pub model_name: String,
    pub client: reqwest::Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model_name: String) -> Self {
        Self {
            api_key,
            model_name,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl ModelProvider for OpenAIProvider {
    async fn chat_stream(&self, context: Vec<Message>) -> atlas_core::errors::Result<ModelStream> {
        // 这里简化演示：暂时只返回一段固定的文本流
        // 真正对接 OpenAI 需要处理 context 中的 messages 转 JSON，并解析 SSE 流
        // 下面是一个模拟流，让你先跑通 Orchestrator 的 Think-Act 循环

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            // 简单的关键词检测模拟 LLM 决策
            let last_msg = context.last().map(|m| m.content.to_lowercase()).unwrap_or_default();

            if last_msg.contains("天气") || last_msg.contains("weather") {
                let _ = tx.send(StreamChunk::Text("收到，正在查询天气...".to_string())).await;

                // 模拟 LLM 输出 Tool Call
                let _ = tx.send(StreamChunk::ToolCallStart("get_current_weather".to_string())).await;
                let _ = tx.send(StreamChunk::ToolArgsDelta("{\"city\": \"Beijing\"}".to_string())).await;
                let _ = tx.send(StreamChunk::ToolCallEnd).await;
            } else {
                // 普通对话
                let _ = tx.send(StreamChunk::Text("我是 Atlas AI。".to_string())).await;
                let _ = tx.send(StreamChunk::Text("您可以问我关于天气的问题，我会尝试调用工具。".to_string())).await;
            }
            // tx 被 drop 后 rx.recv() 返回 None，流自动结束
        });

        Ok(ModelStream::new(rx))
    }
}

// =================================================================
// 2. Main 入口：组装 Agent 并启动循环
// =================================================================
#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化日志
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // 2. 加载环境变量 (API KEY)
    dotenv::dotenv().ok();
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "sk-mock-key".to_string());

    println!(">>> Atlas CLI v0.1.0 <<<");
    println!(">>> Type 'quit' to exit.");

    // 3. 准备组件
    // A. 工具注册表 (使用 core 中 mod.rs 提供的 setup_registry)
    let registry = ToolRegistry::new();

    // B. 工具路由器 (本地 + NoOpSandbox)
    let router = ToolRouter::new(registry, Arc::new(NoOpSandbox));

    // C. 模型提供者
    let model = Box::new(OpenAIProvider::new(api_key, "gpt-4o".to_string()));

    // D. 输出通道 (用于接收 Agent 的实时回复)
    let (tx, mut rx) = mpsc::channel(100);

    // 4. 创建 Orchestrator
    let mut agent = Orchestrator::new(
        "session-001".to_string(), // session_id
        model,
        router,
        tx,
    );

    // 5. 启动后台任务处理 Agent 的输出
    // 这样主线程可以专心处理输入，后台线程打印输出
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                OutboundMessage::TextDelta(text) => {
                    print!("{}", text);
                    io::stdout().flush().unwrap();
                }
                OutboundMessage::Done => {
                    println!("\n[Done]");
                    print!("> "); // 提示符
                    io::stdout().flush().unwrap();
                }
                OutboundMessage::Error(e) => {
                    println!("\n[Error]: {}", e);
                    print!("> ");
                    io::stdout().flush().unwrap();
                }
            }
        }
    });

    // 6. 主循环：读取用户输入 -> 发送给 Agent
    print!("> ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        input.clear();
        stdin.read_line(&mut input)?;
        let query = input.trim();

        if query == "quit" || query == "exit" {
            break;
        }

        if query.is_empty() {
            print!("> ");
            io::stdout().flush()?;
            continue;
        }

        // 构造消息并发送给 Orchestrator
        // 假设 InboundMessage 结构如下 (根据 Orchestrator 推断)
        let msg = InboundMessage {
            content: query.to_string(),
            // 可能还有其他字段，如 role, attachments 等，视 atlas-protocol 定义而定
            ..Default::default()
        };

        // 核心调用！
        if let Err(e) = agent.handle_user_message(msg).await {
            error!("Agent 处理失败: {}", e);
        }
    }

    Ok(())
}
