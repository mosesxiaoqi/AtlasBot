// atlas-core/src/errors.rs
//
// 统一错误定义
// 所有从 core 层对外暴露的错误都通过 CoreError 枚举表示，
// 上层（CLI / Daemon）可根据变体做精细化的错误处理与用户提示。

use thiserror::Error;

// ============================================================
// CoreError — 核心错误枚举
// ============================================================

/// atlas-core 统一错误类型
#[derive(Debug, Clone, Error)]
pub enum CoreError {
    // ---- 模型层 ----

    /// 模型调用相关错误（连接失败、API 返回异常、流式中断等）
    #[error("Model error: {0}")]
    ModelError(String),

    /// 模型响应解析失败（非法 JSON、缺少必要字段等）
    #[error("Model response parse error: {0}")]
    ModelParseError(String),

    // ---- 记忆层 ----

    /// 记忆/历史管理错误（持久化失败、RAG 检索失败等）
    #[error("Memory error: {0}")]
    MemoryError(String),

    // ---- 上下文层 ----

    /// 上下文构建错误（Token 超限、Prompt 模板错误等）
    #[error("Context error: {0}")]
    ContextError(String),

    /// Token 预算超出限制
    #[error("Token budget exceeded: used {used}, limit {limit}")]
    TokenBudgetExceeded { used: usize, limit: usize },

    // ---- 工具层 ----

    /// 工具执行错误（调用失败、参数解析失败、执行超时等）
    #[error("Tool error: {0}")]
    ToolError(String),

    /// 工具未找到
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// 工具参数校验失败
    #[error("Tool argument error for '{tool}': {reason}")]
    ToolArgumentError { tool: String, reason: String },

    /// 工具执行超时
    #[error("Tool execution timed out: '{tool}' after {elapsed_ms}ms")]
    ToolTimeout { tool: String, elapsed_ms: u64 },

    // ---- 安全层 ----

    /// 安全拦截错误（Prompt Injection、敏感路径访问、策略违规等）
    #[error("Security violation: {0}")]
    SecurityError(String),

    /// Prompt Injection 检测
    #[error("Prompt injection detected: {0}")]
    PromptInjection(String),

    // ---- 沙盒层 ----

    /// 沙盒通信错误（Docker/WASM 容器连接/执行失败等）
    #[error("Sandbox error: {0}")]
    SandboxError(String),

    /// 沙盒不可用
    #[error("Sandbox unavailable: {0}")]
    SandboxUnavailable(String),

    // ---- 通信层 ----

    /// 内部通道通信错误（mpsc channel 发送失败等）
    #[error("Channel error: {0}")]
    ChannelError(String),

    // ---- 配置层 ----

    /// 配置/初始化错误
    #[error("Config error: {0}")]
    ConfigError(String),

    // ---- 序列化层 ----

    /// 序列化/反序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),

    // ---- 通用 ----

    /// 调度器达到最大循环次数
    #[error("Max turns exceeded: limit {0}")]
    MaxTurnsExceeded(usize),

    /// 会话已终止，无法继续操作
    #[error("Session terminated: {0}")]
    SessionTerminated(String),

    /// 其他内部错误（兜底）
    #[error("Internal error: {0}")]
    Internal(String),
}

// ============================================================
// Result — 统一 Result 类型别名
// ============================================================

/// atlas-core 统一 Result 类型
pub type Result<T> = std::result::Result<T, CoreError>;

// ============================================================
// From 实现：支持常见错误类型的自动转换
// ============================================================

/// 从 `anyhow::Error` 转换（兜底，便于渐进式迁移）
impl From<anyhow::Error> for CoreError {
    fn from(err: anyhow::Error) -> Self {
        CoreError::Internal(err.to_string())
    }
}

/// 从 `serde_json::Error` 转换
impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::SerializationError(err.to_string())
    }
}

/// 从 `tokio::sync::mpsc::error::SendError<T>` 转换
impl<T> From<tokio::sync::mpsc::error::SendError<T>> for CoreError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        CoreError::ChannelError(err.to_string())
    }
}

/// 从 `std::io::Error` 转换
impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Internal(format!("IO error: {}", err))
    }
}

// ============================================================
// CoreError 辅助方法
// ============================================================

impl CoreError {
    /// 判断错误是否可重试
    /// 某些瞬态错误（网络超时、模型过载）可以通过重试恢复
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CoreError::ModelError(_)
                | CoreError::SandboxError(_)
                | CoreError::SandboxUnavailable(_)
                | CoreError::ChannelError(_)
                | CoreError::ToolTimeout { .. }
        )
    }

    /// 判断是否为安全相关错误
    pub fn is_security_error(&self) -> bool {
        matches!(
            self,
            CoreError::SecurityError(_) | CoreError::PromptInjection(_)
        )
    }

    /// 判断是否为工具相关错误
    pub fn is_tool_error(&self) -> bool {
        matches!(
            self,
            CoreError::ToolError(_)
                | CoreError::ToolNotFound(_)
                | CoreError::ToolArgumentError { .. }
                | CoreError::ToolTimeout { .. }
        )
    }

    /// 生成面向用户的友好错误消息
    /// 隐藏内部实现细节，只暴露必要信息
    pub fn user_facing_message(&self) -> String {
        match self {
            CoreError::ModelError(_) => "AI 模型暂时无法响应，请稍后重试。".to_string(),
            CoreError::ModelParseError(_) => "AI 模型返回了异常数据，请重试。".to_string(),
            CoreError::MemoryError(_) => "记忆系统异常，请重试。".to_string(),
            CoreError::ContextError(_) => "上下文构建失败，请缩短对话后重试。".to_string(),
            CoreError::TokenBudgetExceeded { .. } => {
                "对话上下文过长，请开启新会话或清理历史。".to_string()
            }
            CoreError::ToolError(msg) => format!("工具执行失败: {}", msg),
            CoreError::ToolNotFound(name) => format!("工具 '{}' 不存在。", name),
            CoreError::ToolArgumentError { tool, reason } => {
                format!("工具 '{}' 参数错误: {}", tool, reason)
            }
            CoreError::ToolTimeout { tool, .. } => format!("工具 '{}' 执行超时，请重试。", tool),
            CoreError::SecurityError(_) | CoreError::PromptInjection(_) => {
                "该操作被安全策略拦截。".to_string()
            }
            CoreError::SandboxError(_) | CoreError::SandboxUnavailable(_) => {
                "沙盒环境异常，请检查配置后重试。".to_string()
            }
            CoreError::ChannelError(_) => "内部通信异常，请重启会话。".to_string(),
            CoreError::ConfigError(msg) => format!("配置错误: {}", msg),
            CoreError::SerializationError(_) => "数据解析异常，请重试。".to_string(),
            CoreError::MaxTurnsExceeded(_) => {
                "Agent 思考循环次数过多，已自动终止。".to_string()
            }
            CoreError::SessionTerminated(_) => "当前会话已结束。".to_string(),
            CoreError::Internal(_) => "发生内部错误，请联系管理员。".to_string(),
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_error_display() {
        let err = CoreError::ModelError("connection timeout".to_string());
        assert_eq!(err.to_string(), "Model error: connection timeout");
    }

    #[test]
    fn test_core_error_retryable() {
        assert!(CoreError::ModelError("timeout".into()).is_retryable());
        assert!(CoreError::SandboxError("down".into()).is_retryable());
        assert!(CoreError::ToolTimeout {
            tool: "bash".into(),
            elapsed_ms: 30000
        }
        .is_retryable());

        assert!(!CoreError::SecurityError("injection".into()).is_retryable());
        assert!(!CoreError::ConfigError("missing key".into()).is_retryable());
    }

    #[test]
    fn test_core_error_is_security() {
        assert!(CoreError::SecurityError("blocked".into()).is_security_error());
        assert!(CoreError::PromptInjection("detected".into()).is_security_error());
        assert!(!CoreError::ModelError("fail".into()).is_security_error());
    }

    #[test]
    fn test_core_error_is_tool() {
        assert!(CoreError::ToolNotFound("weather".into()).is_tool_error());
        assert!(CoreError::ToolError("exec fail".into()).is_tool_error());
        assert!(!CoreError::ModelError("fail".into()).is_tool_error());
    }

    #[test]
    fn test_user_facing_message() {
        let err = CoreError::TokenBudgetExceeded {
            used: 130000,
            limit: 128000,
        };
        let msg = err.user_facing_message();
        assert!(msg.contains("对话上下文过长"));
    }

    #[test]
    fn test_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let core_err: CoreError = anyhow_err.into();
        assert!(matches!(core_err, CoreError::Internal(_)));
        assert!(core_err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let core_err: CoreError = json_err.into();
        assert!(matches!(core_err, CoreError::SerializationError(_)));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let core_err: CoreError = io_err.into();
        assert!(matches!(core_err, CoreError::Internal(_)));
        assert!(core_err.to_string().contains("IO error"));
    }

    #[test]
    fn test_clone() {
        let err = CoreError::ToolArgumentError {
            tool: "bash".into(),
            reason: "missing command".into(),
        };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
