// pub fn add(left: u64, right: u64) -> u64 {
//     left + right
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
// atlas-core/src/lib.rs

// 导出核心模块，以便 atlas-cli 可以使用
pub mod errors;
pub mod execution;
pub mod guard;
pub mod memory;
pub mod model;
// 将 atlas-protocol crate 重导出为 protocol 模块，
// 使得 crate 内部可以通过 crate::protocol::... 引用协议类型
pub use atlas_protocol as protocol;

// 重新导出常用组件，方便引用
pub use execution::context_builder::ContextBuilder;
pub use execution::orchestrator::Orchestrator;
pub use execution::tool_registry::ToolRegistry;
pub use execution::tool_router::{NoOpSandbox, ToolRouter};
