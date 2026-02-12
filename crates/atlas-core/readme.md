atlas-core/
├── Cargo.toml
└── src/
    ├── lib.rs              # 导出主要的 Agent 结构体和 Trait
    ├── execution/          # 执行逻辑 (Think-Act Loop)
    │   ├── orchestrator.rs
    │   ├── scheduler.rs        # [新增] 并发控制，会话队列
    │   ├── tool_router.rs      # [新增] 工具分发（本地 vs 沙盒）
    │   └── tool_registry.rs    # [新增] 工具定义与 Schema 生成
    │   └── context_builder.rs  # [新增] Prompt 拼装与 Token 管理
    ├── memory/             # 记忆管理 (RAG & History)
    │   ├── transcript.rs
    │   ├── compact.rs
    │   └── vector.rs
    ├── model/              # 模型接入层
    │   ├── adapter.rs
    │   └── selectors.rs
    ├── guard/              # 安全过滤层
    │   └── firewall.rs
    └── errors.rs           # 统一错误定义
