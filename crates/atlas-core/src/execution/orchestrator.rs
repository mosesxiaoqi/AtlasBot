use crate::errors::{CoreError, Result};
use crate::guard::SecurityGuard;
use crate::memory::transcript::Transcript;
use crate::model::{ModelProvider, StreamChunk};
use crate::protocol::{InboundMessage, OutboundMessage, Role, ToolCall, ToolResult};
use crate::execution::tool_router::ToolRouter; // 假设这是连接沙盒的接口 // 安全卫士

use tokio::sync::mpsc;
use tracing::{info, instrument, warn};

/// Agent 的生命周期状态机
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    Idle,
    Thinking,              // 正在等待 LLM 推理
    ExecutingTool(String), // 正在执行工具 (Tool Name)
    Terminated,            // 会话结束
}

/// 核心调度器结构体
pub struct Orchestrator {
    /// 会话唯一标识
    pub session_id: String,

    /// 记忆管理（包含对话历史 + RAG 检索能力）
    transcript: Transcript,

    /// 模型提供商接口 (OpenAI/Gemini/Claude)
    model: Box<dyn ModelProvider + Send + Sync>,

    /// 工具路由器（负责将指令分发给本地或沙盒）
    tool_router: ToolRouter,

    /// 安全卫士（负责输入/输出审计）
    security: SecurityGuard,

    /// 当前状态
    state: AgentState,

    /// 向前端/客户端发送实时消息的通道
    outbound_tx: mpsc::Sender<OutboundMessage>,
}

impl Orchestrator {
    /// 创建一个新的调度器实例
    pub fn new(
        session_id: String,
        model: Box<dyn ModelProvider + Send + Sync>,
        tool_router: ToolRouter,
        outbound_tx: mpsc::Sender<OutboundMessage>,
    ) -> Self {
        Self {
            session_id: session_id.clone(),
            transcript: Transcript::new(&session_id),
            model,
            tool_router,
            security: SecurityGuard::default(), // TODO: 加载配置
            state: AgentState::Idle,
            outbound_tx,
        }
    }

    /// 核心入口：处理用户输入
    #[instrument(skip(self))]
    pub async fn handle_user_message(&mut self, message: InboundMessage) -> Result<()> {
        info!("收到用户消息: {:?}", message);

        // 1. 安全检查：输入防御 (Prompt Injection 检测)
        // TODO: 实现 guard.check_input，如果包含恶意指令直接拒绝
        if let Err(e) = self.security.check_input(&message) {
            self.send_error(format!("安全拦截: {}", e)).await?;
            return Ok(());
        }

        // 2. 写入记忆：将用户消息添加到历史记录
        // TODO: 实现 transcript.add_message，确保持久化到 SQLite/LanceDB
        self.transcript
            .add_message(Role::User, message.content.clone())
            .await?;

        // 3. 启动思考循环 (Think-Act Loop)
        self.run_think_loop().await
    }

    /// 思考循环：Agent 的主循环 (Think -> Act -> Observe)
    async fn run_think_loop(&mut self) -> Result<()> {
        let max_turns = 10; // 防止无限循环
        let mut turn_count = 0;

        while turn_count < max_turns {
            turn_count += 1;
            self.state = AgentState::Thinking;

            // 4. 构建上下文 (Context Construction)
            // TODO: 实现 build_context:
            //   - 从 Vector Store 检索相关记忆 (RAG)
            //   - 压缩过长的历史记录 (Compaction)
            //   - 注入 System Prompt (包含工具定义)
            let context = self.transcript.build_context().await?;

            // 5. 调用模型 (流式推理)
            info!("开始推理 (Turn {})", turn_count);
            let mut stream = self.model.chat_stream(context).await?;

            // 用于收集完整的回复或工具调用
            let mut current_text_response = String::new();
            let mut current_tool_call: Option<ToolCall> = None;

            // 6. 处理流式响应
            while let Some(chunk) = stream.next().await {
                match chunk {
                    StreamChunk::Text(text) => {
                        // 实时推送到前端
                        self.outbound_tx
                            .send(OutboundMessage::TextDelta(text.clone()))
                            .await?;
                        current_text_response.push_str(&text);
                    }
                    StreamChunk::ToolCallStart(tool_name) => {
                        // TODO: 在这里可以做流式安全拦截，如果工具名是高危的
                        current_tool_call = Some(ToolCall::new(tool_name));
                    }
                    StreamChunk::ToolArgsDelta(args_fragment) => {
                        if let Some(tool) = &mut current_tool_call {
                            tool.append_args(&args_fragment);
                        }
                    }
                    StreamChunk::ToolCallEnd => {
                        // 工具参数接收完毕
                        break; // 跳出流接收，准备执行
                    }
                    StreamChunk::Error(e) => {
                        return Err(CoreError::ModelError(e));
                    }
                }
            }

            // 7. 决策分支
            if let Some(tool_call) = current_tool_call {
                // === 分支 A: 模型想要执行工具 ===

                // 将 Agent 的思考过程（如果有）写入历史
                if !current_text_response.is_empty() {
                    self.transcript
                        .add_message(Role::Assistant, current_text_response)
                        .await?;
                }

                // 执行工具逻辑
                self.handle_tool_execution(tool_call).await?;

                // 工具执行完后，Loop 继续，将结果喂给模型再次思考
                continue;
            } else {
                // === 分支 B: 模型只回复了文本 (结束本次对话) ===
                self.transcript
                    .add_message(Role::Assistant, current_text_response)
                    .await?;
                self.state = AgentState::Idle;
                self.outbound_tx.send(OutboundMessage::Done).await?;
                return Ok(());
            }
        }

        warn!("达到最大循环次数限制");
        Ok(())
    }

    /// 执行工具的逻辑
    async fn handle_tool_execution(&mut self, call: ToolCall) -> Result<()> {
        self.state = AgentState::ExecutingTool(call.name.clone());
        info!("准备执行工具: {}", call.name);

        // 8. 工具安全审计
        // TODO: 实现 security.audit_tool_call
        // 检查参数是否包含敏感路径 (如 /etc/passwd)，是否符合沙盒策略
        self.security.audit_tool_call(&call)?;

        // 9. 路由与执行
        // TODO: 调用 atlas-sandbox 模块
        // 如果是 'bash' -> 发送到 Docker 容器
        // 如果是 'search_memory' -> 本地执行
        let result: ToolResult = self.tool_router.dispatch(call.clone()).await?;

        // 10. 记录结果
        // 将工具的执行结果（Observe）写回记忆，作为下一步思考的依据
        self.transcript.add_tool_result(call.id, result).await?;

        Ok(())
    }

    /// 辅助方法：发送错误给客户端
    async fn send_error(&self, error_msg: String) -> Result<()> {
        self.outbound_tx
            .send(OutboundMessage::Error(error_msg))
            .await?;
        Ok(())
    }
}
