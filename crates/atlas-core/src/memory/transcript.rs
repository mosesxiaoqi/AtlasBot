// atlas-core/src/memory/transcript.rs
//
// 对话历史记录管理（Transcript）
// 负责维护会话的消息列表，并提供上下文构建能力。
// 当前为占位实现，数据仅保存在内存中。

use crate::errors::Result;
use crate::protocol::{Message, Role, ToolResult};

/// 对话记录管理器
///
/// 维护单个会话的完整对话历史，支持：
/// - 添加用户/助手消息
/// - 记录工具调用结果
/// - 构建发送给 LLM 的上下文（含历史压缩、RAG 检索等）
pub struct Transcript {
    /// 会话 ID（后续持久化时使用）
    #[allow(dead_code)]
    session_id: String,

    /// 对话消息历史
    messages: Vec<Message>,
}

impl Transcript {
    /// 创建新的对话记录
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            messages: Vec::new(),
        }
    }

    /// 添加一条消息到历史记录
    ///
    /// TODO: 持久化到 SQLite / LanceDB
    pub async fn add_message(&mut self, role: Role, content: String) -> Result<()> {
        self.messages.push(Message::new(role, content));
        Ok(())
    }

    /// 记录工具执行结果
    ///
    /// 将工具返回值作为 Tool 角色的消息写入历史，
    /// 以便下一轮推理时 LLM 能看到执行结果。
    pub async fn add_tool_result(&mut self, call_id: String, result: ToolResult) -> Result<()> {
        self.messages
            .push(Message::tool_result(call_id, result.output));
        Ok(())
    }

    /// 构建发送给 LLM 的上下文消息列表
    ///
    /// TODO: 实现以下能力：
    /// - 从 Vector Store 检索相关记忆 (RAG)
    /// - 压缩过长的历史记录 (Compaction)
    /// - 注入 System Prompt（包含工具定义）
    pub async fn build_context(&self) -> Result<Vec<Message>> {
        // 占位：直接返回完整历史
        Ok(self.messages.clone())
    }
}
