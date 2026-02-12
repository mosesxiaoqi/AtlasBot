use crate::execution::tool_registry::ToolRegistry;
use crate::protocol::{Message, Role};
use anyhow::{anyhow, Result};
use chrono::Local;
use tiktoken_rs::{get_bpe_from_model, CoreBPE};
use tracing::{debug, warn};

/// 上下文构建请求参数
pub struct ContextRequest<'a> {
    pub system_prompt_template: String,
    pub user_query: Option<String>,      // 当前用户正在输入的内容
    pub history: &'a [Message],          // 原始对话历史
    pub rag_content: Option<String>,     // 从向量库搜到的知识片段
    pub tools: Option<&'a ToolRegistry>, // 可用工具
}

/// 构建完成的上下文结果
pub struct BuiltContext {
    pub messages: Vec<Message>,
    pub token_usage: usize,
    pub max_tokens: usize,
}

/// 上下文构建器配置
#[derive(Clone)]
pub struct ContextBuilderConfig {
    pub model_name: String,
    pub max_context_tokens: usize, // e.g. 128000 for GPT-4o
    pub reserve_for_reply: usize,  // 预留给模型回复的 Token 数，例如 4096
}

pub struct ContextBuilder {
    config: ContextBuilderConfig,
    tokenizer: CoreBPE,
}

impl ContextBuilder {
    pub fn new(config: ContextBuilderConfig) -> Result<Self> {
        // 获取对应模型的 Tokenizer，如果未知模型则回退到 cl100k_base
        let tokenizer = get_bpe_from_model(&config.model_name)
            .or_else(|_| tiktoken_rs::cl100k_base())
            .map_err(|e| anyhow!("Failed to initialize tokenizer: {}", e))?;

        Ok(Self { config, tokenizer })
    }

    /// 核心方法：构建最终发送给 LLM 的消息列表
    pub fn build(&self, req: ContextRequest) -> Result<BuiltContext> {
        let mut final_messages: Vec<Message> = Vec::new();
        let mut used_tokens = 0;

        // 计算实际可用的窗口大小 (总限制 - 预留回复空间)
        let effective_limit = self
            .config
            .max_context_tokens
            .saturating_sub(self.config.reserve_for_reply);

        // ====================================================
        // 1. 构建 System Message (优先级最高)
        // ====================================================
        let system_msg = self.construct_system_message(&req)?;
        let system_tokens = self.count_message_tokens(&system_msg);

        if system_tokens > effective_limit {
            return Err(anyhow!(
                "System prompt is too long ({}) for model limit ({})",
                system_tokens,
                effective_limit
            ));
        }

        used_tokens += system_tokens;
        // 先暂存 System Msg，稍后放到列表头部

        // ====================================================
        // 2. 构建当前用户输入 (优先级次高)
        // ====================================================
        let mut pending_user_msg = None;
        if let Some(query) = req.user_query {
            let msg = Message::new(Role::User, query);
            let tokens = self.count_message_tokens(&msg);
            if used_tokens + tokens < effective_limit {
                used_tokens += tokens;
                pending_user_msg = Some(msg);
            } else {
                warn!("User query too long, truncating...");
                // 这里可以做截断处理，简化起见直接报错或截断
            }
        }

        // ====================================================
        // 3. 填充历史记录 (倒序填充，越新越优先)
        // ====================================================
        let mut history_buffer: Vec<Message> = Vec::new();

        // 这是一个简单的滑动窗口算法
        for msg in req.history.iter().rev() {
            let tokens = self.count_message_tokens(msg);

            if used_tokens + tokens >= effective_limit {
                debug!(
                    "Context limit reached at history depth. Usage: {}/{}",
                    used_tokens, effective_limit
                );
                break;
            }

            // TODO: 在这里处理 ToolCall/ToolResult 配对的完整性
            // 如果截断导致只保留了 Result 没保留 Call，LLM 会报错。
            // 简单的做法是：如果当前 msg 是 ToolResult，强制尝试把前一个 ToolCall 也读进来。

            used_tokens += tokens;
            history_buffer.push(msg.clone());
        }

        // ====================================================
        // 4. 组装最终列表
        // ====================================================

        // A. 放入 System Prompt
        final_messages.push(system_msg);

        // B. 放入历史 (需要反转回时间顺序)
        history_buffer.reverse();
        final_messages.extend(history_buffer);

        // C. 放入当前用户输入 (如果有)
        if let Some(msg) = pending_user_msg {
            final_messages.push(msg);
        }

        Ok(BuiltContext {
            messages: final_messages,
            token_usage: used_tokens,
            max_tokens: self.config.max_context_tokens,
        })
    }

    /// 辅助方法：组装 System Prompt
    fn construct_system_message(&self, req: &ContextRequest) -> Result<Message> {
        let mut content = req.system_prompt_template.clone();

        // 注入动态变量：当前时间
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        content.push_str(&format!("\n\n[Current Time]: {}", now));

        // 注入 RAG 内容
        if let Some(rag) = &req.rag_content {
            content.push_str("\n\n### Retrieved Context (RAG):\n");
            content.push_str(rag);
            content.push_str("\n### End Context\n");
        }

        // 注入工具定义 (OpenAI 模式下通常作为 separate param，但在某些 Prompt 模式下需要注入文本)
        // 这里我们假设使用 Native Tools 协议，不需要手动注入到 System Text 中，
        // 但如果用的是 Claude/ReAct 模式，可能需要在这里把 tools 渲染成文本。
        // 此处暂略，假设由 Model Adapter 处理 json schema。

        Ok(Message::new(Role::System, content))
    }

    /// 估算单条消息的 Token 数
    /// 参考: https://github.com/openai/openai-cookbook/blob/main/examples/How_to_count_tokens_with_tiktoken.ipynb
    pub fn count_message_tokens(&self, msg: &Message) -> usize {
        let mut tokens = 0;

        // Per-message overhead (OpenAI chat format)
        // <|start|>role\ncontent<|end|>\n
        tokens += 3;

        // Role tokens
        tokens += self
            .tokenizer
            .encode_with_special_tokens(&msg.role.to_string())
            .len();

        // Content tokens
        tokens += self
            .tokenizer
            .encode_with_special_tokens(&msg.content)
            .len();

        // Tool calls logic
        if let Some(calls) = &msg.tool_calls {
            for call in calls {
                tokens += self.tokenizer.encode_with_special_tokens(&call.name).len();
                tokens += self
                    .tokenizer
                    .encode_with_special_tokens(&call.arguments)
                    .len();
                tokens += 10; // Extra overhead estimate for tool structures
            }
        }

        if let Some(tool_result_id) = &msg.tool_call_id {
            tokens += self
                .tokenizer
                .encode_with_special_tokens(tool_result_id)
                .len();
            tokens += 5; // overhead
        }

        tokens
    }
}
