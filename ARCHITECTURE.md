# Atlats Architecture

Atlats is a local-first autonomous agent runtime.

## Layers

- infra: security, IO, crypto, logging
- protocol: message formats and RPC contracts
- core: agent brain, scheduler, memory
- sandbox: isolated execution (Docker / WASM)
- apps: daemon + CLI
- extensions: official plugins
