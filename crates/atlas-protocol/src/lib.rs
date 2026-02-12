// atlas-protocol/src/lib.rs
//
// 协议层：定义 Agent 系统中各模块间通信的核心数据结构。
// 仅包含纯数据定义，不含业务逻辑。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================
// Role — 消息角色
// ============================================================

/// 对话消息的角色标识，兼容 OpenAI / Claude / Gemini 等主流 LLM 协议
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 系统指令（System Prompt）
    System,
    /// 用户输入
    User,
    /// 模型/助手回复
    Assistant,
    /// 工具执行结果
    Tool,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

// ============================================================
// Message — 对话消息
// ============================================================

/// 单条对话消息，兼容 OpenAI ChatCompletion 格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: Role,

    /// 文本内容
    pub content: String,

    /// 本条消息中包含的工具调用列表（仅 Assistant 消息可能携带）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// 当 role == Tool 时，标记本条消息对应的 tool_call id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// 创建一条纯文本消息
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建一条携带工具调用的 Assistant 消息
    pub fn with_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// 创建一条工具结果消息
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

// ============================================================
// ToolCall — 工具调用请求
// ============================================================

/// 全局自增 ID 计数器，用于生成唯一的 tool_call id
static TOOL_CALL_ID_SEQ: AtomicU64 = AtomicU64::new(1);

/// LLM 发出的工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 调用唯一标识（如 "call_1"），用于将 ToolResult 与 ToolCall 配对
    pub id: String,

    /// 工具名称（如 "bash"、"weather_lookup"）
    pub name: String,

    /// 原始参数 JSON 字符串（流式接收时逐步拼接，用于 Token 计数等）
    pub arguments: String,

    /// 解析后的参数（供工具执行使用）
    #[serde(default)]
    pub args: Value,
}

impl ToolCall {
    /// 创建新的工具调用（自动分配唯一 id）
    /// 通常在流式接收到 ToolCallStart 时调用
    pub fn new(name: impl Into<String>) -> Self {
        let seq = TOOL_CALL_ID_SEQ.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!("call_{}", seq),
            name: name.into(),
            arguments: String::new(),
            args: Value::Null,
        }
    }

    /// 流式追加参数片段
    /// 在收到 ToolArgsDelta 时调用，拼接原始 JSON 字符串
    pub fn append_args(&mut self, fragment: &str) {
        self.arguments.push_str(fragment);
    }

    /// 将累积的 arguments 字符串解析为 JSON Value 并写入 args 字段
    /// 通常在 ToolCallEnd 后、执行前调用
    pub fn parse_args(&mut self) -> Result<(), serde_json::Error> {
        self.args = serde_json::from_str(&self.arguments)?;
        Ok(())
    }
}

// ============================================================
// ToolResult — 工具执行结果
// ============================================================

/// 工具执行后的返回结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 对应的 tool_call id
    pub call_id: String,

    /// 执行输出内容
    pub output: String,

    /// 是否执行成功
    pub is_error: bool,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            output: output.into(),
            is_error: false,
        }
    }

    /// 创建失败结果
    pub fn error(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            output: output.into(),
            is_error: true,
        }
    }
}

// ============================================================
// InboundMessage — 客户端 → Agent 的入站消息
// ============================================================

/// 客户端发送给 Agent 的消息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InboundMessage {
    /// 用户输入的文本内容
    pub content: String,
}

impl InboundMessage {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

// ============================================================
// OutboundMessage — Agent → 客户端的出站消息
// ============================================================

/// Agent 发送给客户端的实时消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboundMessage {
    /// 流式文本片段（逐 token 推送到前端）
    TextDelta(String),

    /// 本轮对话完成
    Done,

    /// 发生错误
    Error(String),
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_display() {
        assert_eq!(Role::System.to_string(), "system");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
        assert_eq!(Role::Tool.to_string(), "tool");
    }

    #[test]
    fn test_message_new() {
        let msg = Message::new(Role::User, "hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "hello");
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_message_tool_result() {
        let msg = Message::tool_result("call_1", "result data");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id, Some("call_1".to_string()));
    }

    #[test]
    fn test_tool_call_new_and_append() {
        let mut call = ToolCall::new("weather_lookup");
        assert!(call.id.starts_with("call_"));
        assert_eq!(call.name, "weather_lookup");
        assert!(call.arguments.is_empty());

        call.append_args("{\"city\":");
        call.append_args("\"Beijing\"}");
        assert_eq!(call.arguments, "{\"city\":\"Beijing\"}");

        call.parse_args().unwrap();
        assert_eq!(call.args["city"], "Beijing");
    }

    #[test]
    fn test_tool_call_unique_ids() {
        let c1 = ToolCall::new("tool_a");
        let c2 = ToolCall::new("tool_b");
        assert_ne!(c1.id, c2.id);
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("call_1", "sunny");
        assert_eq!(result.call_id, "call_1");
        assert_eq!(result.output, "sunny");
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("call_2", "timeout");
        assert_eq!(result.call_id, "call_2");
        assert!(result.is_error);
    }

    #[test]
    fn test_message_clone() {
        let msg = Message::new(Role::Assistant, "thinking...");
        let cloned = msg.clone();
        assert_eq!(cloned.content, "thinking...");
    }

    #[test]
    fn test_tool_call_clone() {
        let call = ToolCall::new("bash");
        let cloned = call.clone();
        assert_eq!(cloned.id, call.id);
        assert_eq!(cloned.name, "bash");
    }
}
