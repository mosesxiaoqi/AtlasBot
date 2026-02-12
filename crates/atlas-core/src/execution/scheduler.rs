pub enum AgentState {
    /// 待机状态
    Idle,
    /// 正在调用 LLM 进行思考
    Thinking {
        model_id: String,
        start_time: std::time::Instant,
    },
    /// 思考完成，正在调度工具执行
    Executing { tool_name: String, call_id: String },
    /// 对话压缩中（自我整理记忆）
    Compacting,
    /// 发生致命错误
    Error(crate::errors::CoreError),
}
