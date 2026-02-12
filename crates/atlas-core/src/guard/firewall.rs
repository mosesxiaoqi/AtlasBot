// atlas-core/src/guard/firewall.rs
//
// 安全过滤层（Security Guard）
// 负责输入防御（Prompt Injection 检测）和工具调用审计。
// 当前为占位实现，所有检查默认放行。

use crate::errors::Result;
use crate::protocol::ToolCall;

/// 安全卫士
///
/// 负责在 Agent 执行流程中的关键节点进行安全审计：
/// - 用户输入阶段：检测 Prompt Injection 等恶意指令
/// - 工具调用阶段：校验参数是否访问敏感资源（如 /etc/passwd）
#[derive(Debug, Clone)]
pub struct SecurityGuard {
    // TODO: 后续可添加策略配置、黑白名单等字段
}

impl Default for SecurityGuard {
    fn default() -> Self {
        Self {}
    }
}

impl SecurityGuard {
    /// 检查用户输入是否安全
    ///
    /// 泛型参数允许接收任意消息类型（如 InboundMessage）。
    /// TODO: 实现 Prompt Injection 检测逻辑
    pub fn check_input<T>(&self, _message: &T) -> std::result::Result<(), String> {
        // 占位：默认放行
        Ok(())
    }

    /// 审计工具调用请求
    ///
    /// 在工具执行前检查调用是否符合安全策略。
    /// TODO: 实现敏感路径拦截、沙盒策略校验等
    pub fn audit_tool_call(&self, _call: &ToolCall) -> Result<()> {
        // 占位：默认放行
        Ok(())
    }
}
