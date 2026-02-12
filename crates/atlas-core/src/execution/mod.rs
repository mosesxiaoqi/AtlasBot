// 1. 声明子模块（这会告诉编译器去 execution/ 文件夹下找对应的文件）
pub mod context_builder;
pub mod orchestrator;
pub mod scheduler;
pub mod tool_registry;
pub mod tool_router;

// 2. (可选) 内部重导出
// 这样在 lib.rs 里可以直接用 execution::Orchestrator 而不是 execution::orchestrator::Orchestrator
pub use context_builder::ContextBuilder;
pub use orchestrator::Orchestrator;
pub use tool_registry::ToolRegistry;
pub use tool_router::ToolRouter;
