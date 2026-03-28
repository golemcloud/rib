## Rib

A programming language to interact with WebAssembly components deployed in runtimes like Wasmtime, Golem, etc.

### Wasmtime integration

`wasmtime` can make use of the `rib-repl` crate to interact with WASM components. The `rib-repl` crate provides a REPL interface 
to call functions and manipulate data in WASM components running in a Wasmtime environment.
This work is on it's way.

### Golem Integration

While `rib-repl` crate is primarily designed for use with runtimes such as wasmtime,
it is also compatible with other runtimes, including golem. This is proved through `rib-repl-golem`, a binary that can interact with Golem agents.

However, the recommended approach for interacting with Golem agents (implemented as WASM components) is via golem-cli. 
This tool is purpose-built for Golem and provides a more native and streamlined experience aligned with Golem’s core concepts. 
It also enables interaction with agents using languages such as Rust and TypeScript.

Rib, by contrast, is a WebAssembly-oriented language whose grammar adheres to the WASM-WAVE specification. 
It is particularly suitable for users who are familiar with the broader WebAssembly ecosystem. 
Rib can be considered a general-purpose interface for interacting with WASM components across different environments.

To interact with a Golem agent using Rib, execute the rib-repl-golem binary with the
appropriate configuration parameters required to connect to the target agent.

Refer to the Golem documentation for instructions on retrieving the necessary identifiers, such as agent-id and env-name.

```sh
cargo run -p rib-repl-golem -- --app-name agent-http-routes-rust --env-name local --agent-id "HttpAgent(test)"
```

