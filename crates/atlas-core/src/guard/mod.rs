// atlas-core/src/guard/mod.rs
//
// 安全过滤层模块

pub mod firewall;

// 重导出，使外部可通过 crate::guard::SecurityGuard 直接引用
pub use firewall::SecurityGuard;
