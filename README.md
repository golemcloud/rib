## Rib

A programming language to interact with WebAssembly components deployed in runtimes like Wasmtime, Golem, etc.

### Golem Integration

`rib-repl-golem` provides a standalone REPL for components deployed on a Golem server — no `golem-cli` required.

Golem's own REPL (via `golem-cli`) manages the full lifecycle: project scaffolding, builds, deployments.
Use `rib-repl-golem` when you already have a WASM component deployed already into golem

```sh
cargo run -p rib-repl-golem -- --app-name agent-http-routes-rust --env-name local --agent-id "HttpAgent(test)"
```

