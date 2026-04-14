# Rib

[Rib](rib-lang/README.md) is a small **expression language** with a [REPL](rib-repl/README.md) for working with **WebAssembly components** using types aligned with **WIT**. It supports **interactive export probing**, **lightweight validation scripts**, and **embedding** a line-oriented shell in hosts such as **Wasmtime**.

**Rib** lets you interact with deployed Wasm components using **short Rib expressions**.

Rib is **statically typed**: if Rib text does not match the WIT types (wrong fields, arity, etc.), you get a **Rib compile/type error** *before* your embedding runs the actual Wasm call. Many mistakes show up there rather than only as a **failed or trapping invocation** after the fact.

Runtimes can depend on **`rib-repl`** to add a REPL to their CLI without touching **`rib-lang`** directly, with tab completion and argument stubs generated from WIT signatures in **Wasm Wave** syntax—fewer syntax and type mistakes while experimenting.

Most usage is through the **REPL**; **runtimes** can also embed **`rib-lang`** in tests or offer Rib as a small scripting surface for users.

```rust
// define a variable `counter` and assign it to the instance of the component that's loaded by the runtime
let counter = instance();

// calls the export `increment_and_get` and assigns the result to `num`
let a = counter.increment-and-get(); 

let b = counter.increment-and-get();

// adds the two numbers together and returns the result
a + b
```

## Documentation

| | |
|:---|:---|
| [Rib language guide](https://golemcloud.github.io/rib/guide.html) | Features, REPL workflow, `instance()` and exports, `match`, resources; [§1](https://golemcloud.github.io/rib/guide.html#1-instance-and-calling-exports) is enough for many REPL sessions, the rest is reference. |
| [Grammar (EBNF)](rib-lang/README.md) | Formal syntax for implementers and tooling. |
| [REPL](rib-repl/README.md) | Session behaviour, commands, and embedding notes. |

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

**Wasm Wave** — Uses [`wasm-wave`](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave) for parsing and printing many component values in the shared textual format. Resource **handles** are not generally serialisable as arbitrary Wave text; APIs reflect that limitation.

---

## Language surface (summary)

Rib is expression-oriented: literals, records, lists, `let`, conditionals, `match`, calls (including WIT-style qualified export paths), `option` / `result`, comprehensions, and related forms. The **syntax is close to Rust**; **value text** follows **Wasm Wave**, so Rust and Wasm-component authors have little extra surface to learn. The **`rib-repl`** front end adds **tab completion**, including **call completions** that can insert **Wave-shaped placeholder arguments** typed from each export’s **WIT** signature. Rib is intended for **orchestration and exploration**, not as a replacement for full application languages.

See **[Documentation](#documentation)** above for the language guide, grammar, and REPL docs.

---

## History

Rib was first developed for **[Golem Cloud](https://www.golem.cloud/)** against WIT-described components. **`rib-lang`** and **`rib-repl`** are published independently so any Component Model host or tool chain can reuse the same implementation.

### Why the name “Rib”?

No, not the bone—and not an acronym someone reverse-engineered at 2 a.m.

In **pottery**, a **rib** is the modest little paddle (wood, metal, rubber—pick your vibe) that **extends your hand** on the wheel: you smooth the wobbles, firm up the walls, and stop pretending five thumbs are a finishing strategy. Clay spins; the rib keeps you honest.

**Rib** the language stole that job description. Components and **WIT** are the spinning part; Rib is the **small tool at the boundary** that helps you **shape** exports, nudge values into place, and file off the rough bits. Same energy: craft, not chaos.

---

## License

**Apache License, Version 2.0** — see [`LICENSE`](LICENSE). The file includes the **LLVM Exception** appendix (the same dual-appendix pattern used by Rust and many Wasmtime-related projects). Each published crate also ships a copy as `rib-lang/LICENSE` and `rib-repl/LICENSE`.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
