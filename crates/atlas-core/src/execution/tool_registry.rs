use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// 定义工具的参数 Schema 生成器
/// 任何想要成为工具参数的 Struct 都需要实现这个 Trait (通过 #[derive(JsonSchema)])
pub trait ToolArgs: JsonSchema + Send + Sync {}
impl<T> ToolArgs for T where T: JsonSchema + Send + Sync {}

/// 核心工具 Trait
/// 所有的本地工具（Native Tools）都必须实现这个接口
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称 (例如 "weather_lookup")
    fn name(&self) -> &str;

    /// 工具描述 (给 LLM 看的说明书)
    fn description(&self) -> &str;

    /// 返回参数的 JSON Schema
    /// (通常由 schemars 自动生成)
    fn parameters_schema(&self) -> Value;

    /// 执行工具逻辑
    /// args: 从 LLM 传来的 JSON 参数
    async fn execute(&self, args: Value) -> Result<String>;
}

/// 工具注册表
/// 负责管理所有可用工具，并生成给 LLM 的配置
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册一个新工具
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            warn!("工具 '{}' 被重复注册，旧定义将被覆盖", name);
        }
        info!("注册工具: {}", name);
        self.tools.insert(name, Arc::new(tool));
    }

    /// 注册动态工具 (Arc 指针形式)
    pub fn register_arc(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// 根据名称查找工具
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 导出为 OpenAI/兼容格式的工具定义列表
    /// 这就是直接喂给 `client.chat.completions.create({ tools: ... })` 的数据
    pub fn to_llm_config(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema()
                    }
                })
            })
            .collect()
    }

    /// 列出所有已注册的工具名称
    pub fn list_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

// ==========================================
// 辅助宏与通用实现 (Helper Implementation)
// ==========================================

/// 这是一个通用的结构体，用于快速把 Rust 函数包装成 Tool
/// 它可以极大地减少样板代码
pub struct FunctionalTool<F, A> {
    pub name: String,
    pub description: String,
    pub func: F,
    pub _phantom: std::marker::PhantomData<A>,
}

impl<F, A, Fut> FunctionalTool<F, A>
where
    A: ToolArgs + serde::de::DeserializeOwned + 'static,
    F: Fn(A) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<String>> + Send + 'static,
{
    pub fn new(name: &str, description: &str, func: F) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<F, A, Fut> Tool for FunctionalTool<F, A>
where
    A: ToolArgs + serde::de::DeserializeOwned + 'static,
    F: Fn(A) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<String>> + Send + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        // 魔法发生在这里：schemars 自动从泛型 A 生成 JSON Schema
        let schema = schemars::schema_for!(A);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, args: Value) -> Result<String> {
        // 自动将 JSON 解析为强类型的 Rust Struct
        let parsed_args: A = serde_json::from_value(args)?;
        // 调用具体的业务逻辑函数
        (self.func)(parsed_args).await
    }
}
