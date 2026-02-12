// atlas-core/src/model/adapter.rs
//
// 模型接入层（Model Adapter）
// 定义与 LLM 通信的核心 Trait 和流式响应类型。
// 当前为占位定义，具体适配（OpenAI / Claude / Gemini）后续实现。

use crate::errors::Result;
use crate::protocol::Message;
use async_trait::async_trait;
use tokio::sync::mpsc;

// ============================================================
// StreamChunk — 流式响应片段
// ============================================================

/// LLM 流式推理返回的数据块
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// 文本片段（逐 token 推送）
    Text(String),

    /// 工具调用开始（携带工具名称）
    ToolCallStart(String),

    /// 工具调用参数片段（JSON 逐步拼接）
    ToolArgsDelta(String),

    /// 工具调用结束
    ToolCallEnd,

    /// 推理过程中发生错误
    Error(String),
}

// ============================================================
// ModelStream — 模型响应流
// ============================================================

/// 包装 tokio mpsc Receiver 的模型响应流
///
/// 通过 `next().await` 逐块获取 LLM 的流式输出。
pub struct ModelStream {
    rx: mpsc::Receiver<StreamChunk>,
}

impl ModelStream {
    /// 从 mpsc Receiver 创建
    pub fn new(rx: mpsc::Receiver<StreamChunk>) -> Self {
        Self { rx }
    }

    /// 获取下一个流式片段，返回 None 表示流结束
    pub async fn next(&mut self) -> Option<StreamChunk> {
        self.rx.recv().await
    }
}

// ============================================================
// ModelProvider — 模型提供商 Trait
// ============================================================

/// 模型提供商接口
///
/// 所有 LLM 适配器（OpenAI、Claude、Gemini 等）都需要实现此 Trait。
/// 通过 `Box<dyn ModelProvider + Send + Sync>` 在运行时动态分发。
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// 发起流式推理
    ///
    /// 接收上下文消息列表，返回一个可逐块读取的响应流。
    async fn chat_stream(&self, context: Vec<Message>) -> Result<ModelStream>;
}
