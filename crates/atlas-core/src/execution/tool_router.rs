use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

use crate::execution::tool_registry::ToolRegistry;
use crate::protocol::{ToolCall, ToolResult};

/// 沙盒客户端接口 (Sandbox Client Interface)
/// 定义了 Core 与 Sandbox (Docker/WASM) 通信的标准
/// 未来由 `crates/atlas-sandbox` 实现
#[async_trait]
pub trait SandboxClient: Send + Sync {
    /// 列出沙盒环境支持的所有工具定义 (JSON Schema)
    async fn list_tools(&self) -> Result<Vec<Value>>;

    /// 在沙盒中执行指定工具
    async fn execute_tool(&self, name: &str, args: Value) -> Result<String>;
}

/// 默认的空沙盒实现 (用于开发阶段或纯本地模式)
pub struct NoOpSandbox;
#[async_trait]
impl SandboxClient for NoOpSandbox {
    async fn list_tools(&self) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn execute_tool(&self, name: &str, _args: Value) -> Result<String> {
        Err(anyhow!("Sandbox not configured. Cannot execute '{}'", name))
    }
}

/// 工具路由器
/// 负责聚合本地工具和沙盒工具，并进行分发
pub struct ToolRouter {
    /// 本地工具注册表 (内存、RAG、API封装等)
    local_registry: ToolRegistry,

    /// 沙盒客户端 (Bash、Python、文件操作等)
    sandbox: Arc<dyn SandboxClient>,
}

impl ToolRouter {
    /// 创建新的路由器
    pub fn new(local_registry: ToolRegistry, sandbox: Arc<dyn SandboxClient>) -> Self {
        Self {
            local_registry,
            sandbox,
        }
    }

    /// 获取合并后的所有工具定义 (用于生成 System Prompt)
    pub async fn get_all_tool_definitions(&self) -> Result<Vec<Value>> {
        // 1. 获取本地工具定义
        let mut definitions = self.local_registry.to_llm_config();

        // 2. 获取沙盒工具定义 (通常是动态的，取决于沙盒环境)
        match self.sandbox.list_tools().await {
            Ok(sandbox_defs) => {
                definitions.extend(sandbox_defs);
            }
            Err(e) => {
                warn!("Failed to fetch sandbox tools: {}", e);
                // 这里的策略是：如果沙盒挂了，我们只返回本地工具，不让整个 Agent 崩溃
            }
        }

        Ok(definitions)
    }

    /// 核心分发逻辑
    #[instrument(skip(self, call), fields(tool_name = %call.name, call_id = %call.id))]
    pub async fn dispatch(&self, call: ToolCall) -> Result<ToolResult> {
        debug!("Dispatching tool call");

        // 1. 优先尝试本地匹配
        // 本地工具通常响应更快，且包含核心系统功能
        if let Some(tool) = self.local_registry.get(&call.name) {
            info!("Routing to LOCAL handler");
            match tool.execute(call.args).await {
                Ok(output) => {
                    return Ok(ToolResult::success(call.id, output));
                }
                Err(e) => {
                    // 工具内部执行错误 (业务逻辑错误)
                    warn!("Local tool execution failed: {}", e);
                    return Ok(ToolResult::error(call.id, format!("Error: {}", e)));
                }
            }
        }

        // 2. 本地没有，尝试发往沙盒
        // 这通常是处理 "bash", "python", "read_file" 等
        info!("Routing to SANDBOX handler");
        match self.sandbox.execute_tool(&call.name, call.args).await {
            Ok(output) => Ok(ToolResult::success(call.id, output)),
            Err(e) => {
                // 沙盒通信错误或沙盒内工具未找到
                warn!("Sandbox execution failed: {}", e);
                // 注意：这里我们需要区分是 "工具不存在" 还是 "执行出错"
                // 简化起见，统统返回 Error
                Ok(ToolResult::error(call.id, format!("Sandbox Error: {}", e)))
            }
        }
    }
}
