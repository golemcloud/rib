# Rib

[Rib](rib-lang/README.md) is a small **expression language** with a [REPL](rib-repl/README.md) for working with **WebAssembly components** using types aligned with **WIT** . It supports **interactive export probing**, **lightweight validation scripts**, and **embedding** a line-oriented shell in hosts such as **Wasmtime**.
**Rib** lets you write interaction with deployed WASM components using  **short Rib expression**. 

It is statically typed that if Rib text does not match the WIT types (wrong fields, arity, etc.), you get a **Rib compile/type error** *before* your embedding runs the actual WASM call. Many mistakes show up there rather than only as a **failed or trapping invocation** after the fact.

The runtimes can depend on `rib-repl` to quickly add a REPL to their CLI without worry about `rib-lang` at all, with full auto complete features including function argument stubs generated from WIT signatures in WASM-WAVE syntax. This will allow users to experiment with runtime with least number of mistakes (syntax, type mismatch etc)

For the most part, the usage pattern of `rib` is through REPL, unless ruintimes choose to use `rib-lang` directly in their codebase, for example in tests or as a scripting language for users. 

```rust
// define a variable `counter` and assign it to the instance of the component that's loaded by the runtime
let counter = instance();

// calls the export `increment_and_get` and assigns the result to `num`
let a = counter.increment_and_get(); 

let b = counter.increment_and_get();

// adds the two numbers together and returns the result
a + b
```

Find more details: [language](rib-lang/README.md) and [REPL](rib-repl/README.md)
---

## Component model, WIT, and execution context

**[WebAssembly Component Model](https://component-model.bytecodealliance.org/)** — Standard packaging and typing for Wasm components so hosts and guests agree on exports and value representations.

**[WIT](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)** — Interface definitions for that model (functions, records, lists, `option`, `result`, enums, variants, flags, resources).

**Execution context** — Rib source is **not** compiled into the guest component. In a REPL integration, Rib text is entered at an interactive prompt; it is parsed and interpreted **in the host process** (the binary that embeds the runtime, e.g. the Wasmtime CLI). That host then performs the actual **component calls** according to the embedder’s `invoke` implementation.

---

## Repository layout

| crates.io | Library | Responsibility |
|-----------|---------|----------------|
| **`rib-lang`** | `rib` | Parse Rib, infer and check types, compile, interpret; **`wit_type`**; **Wasm Wave** integration for typed value text where applicable. |
| **`rib-repl`** | — | REPL session (line editor, state, commands); delegates compilation to **`rib-lang`**; requires an embedder-supplied **`ComponentFunctionInvoke`** implementation for real calls. |

**`rib-lang` without the REPL** — Supply analysed exports and types, register them, run the parse/check/compile/interpret pipeline, and implement the interpreter’s invocation hook. **`rib-repl`** is the reference embedding for an interactive session on top of the same stack.

**Wasm Wave** — Uses [`wasm-wave`](https://github.com/bytecodealliance/wasm-wave) for parsing and printing many component values in the shared textual format. Resource **handles** are not generally serialisable as arbitrary Wave text; APIs reflect that limitation.

---

## Language surface (summary)

Rib is expression-oriented: literals, records, lists, `let`, conditionals, `match`, calls (including WIT-style qualified export paths), `option` / `result`, comprehensions, and related forms. The **syntax is close to Rust**; **value text** follows **Wasm Wave**, so Rust and Wasm-component authors have little extra surface to learn. The **`rib-repl`** front end adds **tab completion**, including **call completions** that can insert **Wave-shaped placeholder arguments** typed from each export’s **WIT** signature. Rib is intended for **orchestration and exploration**, not as a replacement for full application languages.

Formal grammar: [rib-lang/README.md](rib-lang/README.md). REPL behaviour: [rib-repl/README.md](rib-repl/README.md).

---

## History

Rib was first developed for **[Golem Cloud](https://www.golem.cloud/)** against WIT-described components. **`rib-lang`** and **`rib-repl`** are published independently so any Component Model host or tool chain can reuse the same implementation.

---

## License

**Apache License, Version 2.0** — see [`LICENSE`](LICENSE). The file includes the **LLVM Exception** appendix (the same dual-appendix pattern used by Rust and many Wasmtime-related projects). Each published crate also ships a copy as `rib-lang/LICENSE` and `rib-repl/LICENSE`.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
