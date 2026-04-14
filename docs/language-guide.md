# Rib language guide

Rib is a small expression language for WebAssembly components: WIT-shaped types, [Wasm Wave](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave) literals where they apply. Syntax is Rust-like (`let`, blocks, `if` / `then` / `else`, `match`, `instance()` plus dotted export calls)—common in the Wasm toolchain. Grammar stays small: skim the first sections (~5 min), keep the rest for reference.

You do not need this whole guide first—[Rib at the REPL](#rib-at-the-repl) is enough to get oriented at the prompt.

Repository: [github.com/golemcloud/rib](https://github.com/golemcloud/rib) · Grammar (EBNF): [rib-lang/README.md](../rib-lang/README.md)

### Rib at the REPL

The REPL is the main place Rib is meant to shine. When the host has already loaded your component, the prompt can auto-complete exports and shape arguments for you—almost everything you type is guided from that contract. The experience we aim for: once you are in a wired-up REPL, you should not keep jumping back to WIT source files for ordinary work; the session reflects the types you shipped, and you stay in flow.

**Typical launch** — the exact command is embedder-specific, but the pattern is: point your host’s Rib-enabled CLI at a component, then work at a `>>>` prompt—for example `my-runtime-cli rib path/to/component.wasm`.

```console
>>> let a = instance()

>>> let b = a.increment-and-get()

>>> let c = a.increment-and-get()

>>> b + c
3
```

**Tab completion** covers two different things:

- **Function names** — after the dot on whatever `instance()` returned (or on a resource handle), **Tab** completes **kebab-case** names from your WIT. Keep pressing **Tab** **before** you type **`(`** to cycle through **every** callable on that value—including **resource constructors**—in one list.

- **Arguments** — **after** you pick a name, type **`(`**, then **Tab** to insert **Wave-shaped** placeholders that match the WIT parameter types (record fields, `option` / `result` shells, numeric literals where it helps, …). Edit the stub in place instead of copying from WIT by hand.

```console
>>> let a = instance()

>>> a.add-to-cart({ product-id: 1, product: "t-shirt" })

()
```

**Resource in the same session** — when your world exposes a **`cart`** resource (as in [`example.wit`](example.wit) `shopping`), the constructor is just another dotted name on `instance()`; the handle then gets its own **function names** after the dot—same rhythm as above.

```console
>>> let w = instance()

>>> let cart = w.cart("checkout-1")

>>> cart.add-line({ sku: "mug", qty: 2u32 })

>>> cart.line-count()
1
```

Day-to-day use stays small: `instance()`, a `let` binding (e.g. `let my-instance = instance();`), then exports with dot syntax—[§1 *`instance()` and calling exports*](#1-instance-and-calling-exports) is enough to be productive. You are still speaking the component model’s types and Wasm Wave literals, not inventing a parallel schema in your head.

### Advanced and Direct use of Rib (not through REPL)

Rib also shines when you run a program against values that already crossed the boundary: post-process worker or component results, reshape nested `record` / `list` / `option` / `result` trees with `match` and expressions, and test structure and transformations instead of only asserting “something came back.” That is how you avoid hand-written ad-hoc checks that drift from your real WIT. Historically, runtimes such as Golem embedded Rib in API-definition YAML so a small script could reshape the HTTP-facing output of a component-backed worker—same pattern anywhere you want a typed, short script next to config rather than untyped glue.

If your world is WIT, Wasmtime, and components as contracts: Rib is a compact, statically checked expression layer that stays on those types end-to-end—useful whenever you want programmable glue without leaving the WebAssembly component ecosystem.

### About this guide (long on purpose, light in practice)

We document a lot so nothing feels hidden—but Rib is not heavyweight. For REPL work especially, [§1 *`instance()` and calling exports*](#1-instance-and-calling-exports) is usually enough to feel fluent at the prompt, especially with completion wired to your component. Treat the rest of this file as a reference you reach for when you move into scripts, `match` on richer values, resources, comprehensions, or other advanced Rib—not a front-to-back assignment.

---

Companion WIT — [`example.wit`](example.wit) exports `inventory` (records, enums, plain funcs) and `shopping` (a `cart` resource) from world `guide-demo`. Start with [§1](#1-instance-and-calling-exports); [§0](#0-example-wit-examplewit) spells out the WIT if you want it on the page, and [§9 *`match`*](#9-match-and-patterns) onward when you need more than export calls.

---

## Table of contents

0. [Example WIT (`example.wit`)](#0-example-wit-examplewit)  
1. [`instance()` and calling exports](#1-instance-and-calling-exports)  
2. [Programs, blocks, and semicolons](#2-programs-blocks-and-semicolons)  
3. [Comments](#3-comments)  
4. [Literals and Wave-shaped values](#4-literals-and-wave-shaped-values)  
5. [Types and annotations](#5-types-and-annotations) ([inference sketch](#type-inference-sketch))  
6. [`let` bindings](#6-let-bindings)  
7. [Operators](#7-operators)  
8. [`if` / `then` / `else`](#8-if--then--else)  
9. [`match` and patterns](#9-match-and-patterns)  
10. [List comprehensions (`for` … `yield`)](#10-list-comprehensions-for--yield)  
11. [List aggregation (`reduce` … `yield`)](#11-list-aggregation-reduce--yield)  
12. [Records, tuples, lists, flags](#12-records-tuples-lists-flags)  
13. [`option` and `result`](#13-option-and-result)  
14. [String interpolation](#14-string-interpolation)  
15. [Invoking resource methods](#15-invoking-resource-methods)  

---

## 0. Example WIT (`example.wit`)

The file [`example.wit`](example.wit) is documentation-only (not wired into the compiler by default), but world `guide-demo` exports two interfaces so the guide can stay in one place:

- `inventory` — records, enums, variants, flags, and plain `func` exports (most examples below).
- `shopping` — a `resource cart` (constructor + methods); see [§1](#1-instance-and-calling-exports) and [§15](#15-invoking-resource-methods).

| WIT shape | Where | Meaning |
|-----------|--------|--------|
| `record` | `inventory` | `point`, `line-item`, … |
| `enum` | `inventory` | `order-stage`: `draft` \| `placed` \| `shipped` |
| `variant` | `inventory` | `payment-info`, … |
| `flags` | `inventory` | `file-access`: `read`, `write`, `execute` |
| `func` | `inventory` | `length`, `validate-qty`, `lookup-sku`, `ratio`, … |
| `resource` | `shopping` / `cart` | State per cart; `constructor`, `line-count`, `add-line` (uses `inventory`.`line-item`) |

How you call exports from Rib is in [§1](#1-instance-and-calling-exports): `let my-instance = instance();` then `my-instance.lookup-sku(...)`, etc. WIT export names use kebab-case after the dot.

---

## 1. `instance()` and calling exports

What `instance()` is (plain version): Your host (REPL, test harness, etc.) has already loaded the Wasm component your WIT describes. `instance()` is the one call that says: “hand me the live object I should send export calls to.” Give that object a `let` name that reads nicely to you—this guide uses `my-instance` because it sits right next to `instance()` and stays easy to spot in snippets. Then call exports with dot syntax, same rhythm as methods on a value in Rust.

```rust
let my-instance = instance();

my-instance.lookup-sku(7)
```

That’s the whole pattern: one binding from `instance()`, then `that-name.export-name(…)`. Export names come from WIT and are usually kebab-case (`lookup-sku`, `format-stage`, …). Hyphens in `let` names are fine too (`store-main`, `lane-a`, …) if you prefer that style.

More calls against [`example.wit`](example.wit) → `inventory`:

```rust
let my-instance = instance();

let d = my-instance.length({ x: 3, y: -4 });

let blurb = my-instance.format-stage(draft);

let label = my-instance.lookup-sku(42); // option<string>

let half = my-instance.ratio(9, 2);

let row = my-instance.make-item("pencil", 5u32);

let caps = my-instance.describe-access({ read, write });
```

*For `lookup-sku`, you often follow with `match` on `option`—see [§9.1](#sec-9-1).*

Resources (e.g. `shopping`’s `cart` in the same world) use the same pattern: `let shopping-cart = my-instance.cart("checkout-1");` then `shopping-cart.add-line(…)`. Details in [§15 *Invoking resource methods*](#15-invoking-resource-methods).

Exact lowering still depends on your embedder; `example.wit` is the reference. A richer `cart` API in compiler tests is linked from [§15](#15-invoking-resource-methods).

---

## 2. Programs, blocks, and semicolons

A Rib **program** is a sequence of expressions separated by **`;`**. The value of the whole program is the **last** expression (REPLs usually print that).

```rust
let x = 1;

let y = 2;

x + y
```

A **block** is `{` … `}` containing its own `;`-separated Rib program:

```rust
let z = {
  let a = 10;

  a + 1
};

z
```

---

## 3. Comments

- **Line:** `//` and `///`  
- **Block:** `/* … */` and doc-style `/** … */`

```rust
// one line

/* block
   comment */
```

---

## 4. Literals and Wave-shaped values

Scalars and structured values use **Wasm Wave** text rules. The right column ties each shape to **[`example.wit`](example.wit)** (`inventory`).

| Kind | Examples | From `inventory` (when relevant) |
|------|-----------|-----------------------------------|
| Boolean | `true`, `false` | Conditions, e.g. with **`validate-qty`** (`u32` → `bool`) |
| Integer | `0`, `-42`, `42u32` | `qty` is `u32`; point fields are `s32` |
| String | `"hello"` | `sku` values |
| List | `[1, 2, 3]`, `["a"]` | Any `list<…>` you compose at the prompt |
| Record | `{ x: 1, y: -2 }` | A **`point`** literal |
| Record | `{ sku: "pen", qty: 3u32 }` | A **`line-item`** literal |
| Tuple | `("x", 1u32)` | General tuple syntax |
| Flags | `{ read, write }` | A **`file-access`** value (subset of `read`, `write`, `execute`) |
| `option` | `none`, `some("ink")` | Same shape as **`lookup-sku`** result |
| `result` | `ok(7)`, `err("div0")` | Same shape as **`ratio`** result |

---

## 5. Types and annotations

Rib is **statically typed** against the **WIT** you register with the compiler.

Most of the time you never need to specifically annotate the type. When you do need to, Rib allows **explicit** types—**`let x: T = …`** and **type ascription** **`expr : type`** (below).

**Type ascription** on an expression: **`expr : type`**.

```rust
let n: s32 = 40;

let m: s32 = n + 2;

m
```

Common **scalar** type names: `bool`, `s8`, `u8`, `s16`, `u16`, `s32`, `u32`, `s64`, `u64`, `f32`, `f64`, `char`, `string`.

**Compound** types: `list<string>`, `tuple<s32, string>`, `option<u64>`, `result` / `result<string, string>`, etc.

<a id="type-inference-sketch"></a>

### Type inference (sketch)

Most types are **fixed by WIT**: `instance()`, export signatures, and the Wave-shaped literals you build. Inference does not stop at a single left-to-right pass: the checker **propagates constraints in both directions** (calls, `let`, literals, and return positions) and **re-runs to a fixed point**—it keeps tightening until nothing more can be learned or it finds a contradiction. That **best-effort** cycle is what lets you **omit explicit `: type` annotations** most of the time: Rib keeps reconciling partial information until the line agrees with your WIT.

In a **REPL** with **tab completion** wired to the loaded component, stubs and signatures already sit on the right WIT types, so short expressions **usually “just work”** without you spelling widths by hand.

When you omit an annotation on **`let`**, **integer literals** (`1`, `42`, …) still need a concrete width (`u8`, `u16`, `u32`, …). Those literals are **pulled** toward whatever **`func`** arguments and surrounding expressions require.

Against [`example.wit`](example.wit), `inventory` includes **`validate-qty: func(qty: u32) -> bool`**. So in:

```rust
let my-instance = instance();

let x = 1;

my-instance.validate-qty(x)
```

**`x`** is inferred as **`u32`**, because that is what **`validate-qty`** expects. The same idea applies to other calls: **`instance()`** pins the component API, and argument positions **pull** the types they need from the registered **`func`** signatures (including the right **integer width** among `u8`, `u16`, `u32`, … when a plain literal is passed).

**Longer Rib programs** (many bindings, nested control flow, or heavy overloading-style ambiguity) give the solver **less local** evidence per line; you may need an occasional **`:`** annotation or a slightly more explicit literal. That is rarer at the prompt than in a big script.

The **full** inference algorithm (unification, error messages, edge cases) is **out of scope** for this guide—if Rib reports a type error, read it as “this line cannot be made consistent with your WIT,” then add a **`:`** annotation or adjust literals / calls.

---

## 6. `let` bindings

```rust
let answer = 42;

let labeled: u32 = 7u32;
```

Bindings from earlier lines **stay in scope** in a REPL session until cleared.

---

## 7. Operators

Binary operators (with usual precedence grouping in Rib): **`+` `-` `*` `/`**, comparisons **`==` `!=` `<` `>` `<=` `>=`**, and **`&&` `||`**. Unary **`!`**.

**Chaining:** Rib also has suffix forms for indexing, field-like segments, ranges, and further binary ops on the right—see the full grammar in `rib-lang/README.md` for `rib_suffix` / `segment_suffix` / `range_suffix`.

```rust
let xs = [1, 2, 3];
xs[0] == 1
```

---

## 8. `if` / `then` / `else`

```rust
if score > 10 then "win" else "lose"
```

All three parts are expressions and must type-check together. **`score`** should match your comparison (e.g. **`u32`** from WIT); a plain literal **`10`** is fine once types line up.

---

## 9. `match` and patterns

`match` chooses an arm by **pattern** on a value. Arms are **`pattern => expr`**, separated by commas, inside `{ }`.  
Below, types come from **[`example.wit`](example.wit)** → **`inventory`**. Rib uses **Wave-shaped** literals and **WIT-derived** constructor / case names (often **kebab-case** where the WIT used hyphens).

<a id="sec-9-1"></a>

### 9.1 `option` — e.g. return type of `lookup-sku`

`lookup-sku` returns `option<string>`. *(`my-instance` here is just an example name—use whatever reads best for you; see [§1](#1-instance-and-calling-exports).)*

```rust
let my-instance = instance();

let label = my-instance.lookup-sku(42);

match label {
  some(name) => name,
  none => "unknown"
}
```

<a id="sec-9-2"></a>

### 9.2 `result` — e.g. return type of `ratio`

`ratio` returns `result<s32, string>`.

```rust
let my-instance = instance();

let q = my-instance.ratio(10, 2);

match q {
  ok(n) => n,
  err(msg) => 0
}
```

### 9.3 `enum` — `order-stage` (`draft` \| `placed` \| `shipped`)

Enum patterns are the **case names** from WIT. Suppose **`s`** has type **`order-stage`** (however you obtained it—another function’s return value, etc.):

```rust
match s {
  draft => "still editing",
  placed => "waiting to ship",
  shipped => "on the truck"
}
```

To turn a stage into a single string with a function instead of `match`, call `format-stage` (see [§1](#1-instance-and-calling-exports)).

### 9.4 `variant` — `payment-info`

From **`example.wit`**:

```text
variant payment-info { card(string), wallet, failed(string) }
```

Rib patterns use the **case name**; payloads go in parentheses when the case carries data:

```rust
// Suppose `p` has type `payment-info` (e.g. passed in from another call).

match p {
  card(last4) => "card",
  wallet => "wallet",
  failed(reason) => reason
}
```

To produce a single string from a `payment-info` value via an export, use `summarize-payment` on the same `instance()` binding you used elsewhere ([§1](#1-instance-and-calling-exports)).

### 9.5 `record` — `point`, `line-item`

Field names match WIT (`x` / `y`, `sku` / `qty`):

```rust
let home = { x: 0, y: 0 };

let item = { sku: "notebook", qty: 2u32 };

match home {
  { x: x, y: y } => x + y
}
```

### 9.6 `list` patterns

```rust
match ids {
  [only] => only,
  _ => 0
}
```

### 9.7 Catch-all and aliases

- **`_`** — any value not covered by earlier arms (required when patterns would otherwise be non-exhaustive; see §9.8).  
- **`name @ pattern`** — bind the **whole** matched value to **`name`** while also matching **`pattern`**.

```rust
match home {
  p @ { x: xa, y: ya } => xa + ya
}
```

### 9.8 Compile-time errors (`match`, enums, calls)

Rib reports many mistakes **while compiling** Rib source (REPL line or script)—**before** your embedder invokes Wasm. A few common cases:

**Exhaustive `match` on variants** — In [`example.wit`](example.wit), `variant payment-info` has exactly **three** cases: **`card`**, **`wallet`**, and **`failed`** (§9.4). A `match` on a `payment-info` value must cover **all** of them, unless you add a **`_`** arm. If you only write arms for **two** of the three and omit **`_`**, Rib rejects the program at **compile time** with a non-exhaustive `match` error—you do not wait until runtime to discover the gap.

**Enums** — `order-stage` is only **`draft`**, **`placed`**, and **`shipped`**. A typo in a pattern (e.g. a name that is not a WIT case) or a `match` that omits a case without **`_`** is likewise a **compile-time** error.

**Calls** — Arguments are checked against the **`func`** signature. Example: **`validate-qty`** expects **`u32`**; passing a **string** or the wrong **record** shape to **`length`** is rejected **at compile time**, not as a failed Wasm call later.

---

## 10. List comprehensions (`for` … `yield`)

```rust
for word in ["hello", "world"] {
  yield word;
}
```

Optional **statements** may appear **before** `yield` in the `{ }` block (same idea as a small inner block).

---

## 11. List aggregation (`reduce` … `yield`)

```rust
reduce acc, x in [1, 2, 3] from 0 {
  yield acc + x;
}
```

Here **`acc`** is the accumulator, **`x`** is each element from the list after **`in`**, and **`from`** supplies the initial accumulator value.

---

## 12. Records, tuples, lists, flags

**Record** — field names and types must match WIT. From **`inventory`**:

```rust
let origin: point = { x: 0, y: 0 };
let row: line-item = { sku: "eraser", qty: 1u32 };
```

**Tuple:** `(a, b, c)` — general syntax; your WIT may or may not expose tuples.

**List:** `[e1, e2, e3]`.

**Flags** — independent booleans from WIT; subset of the declared names. **`file-access`** allows **`read`**, **`write`**, **`execute`**:

```rust
let my-instance = instance();

let f = { read, execute };

my-instance.describe-access(f)
```

---

## 13. `option` and `result`

**Construction** (Wave-shaped; same shapes as **`lookup-sku`** / **`ratio`** results):

```rust
let ok-x = ok(42);

let err-x = err("by-zero");

let some-x = some("found");

let none-x = none;
```

Destruction — see [§9.1](#sec-9-1) (`option`) and [§9.2](#sec-9-2) (`result`) using the real `inventory` return types.

---

## 14. String interpolation

Inside `"…"`, **`${` … `}`** embeds a Rib **block** (sequence of expressions; the block’s value is inserted).

```rust
let name = "Rib";

"The language is ${name}"
```

---

## 15. Invoking resource methods

[`example.wit`](example.wit) defines **`cart`** as a **resource** (under `shopping`); **`add-line`** and **`line-count`** are **methods** on the handle. The way this works in Rib is very intuitive—see below. Same mental model as **`instance()`**, so resource calls stay easy.

```rust
let my-instance = instance();

let shopping-cart = my-instance.cart("checkout-1");

shopping-cart.add-line({ sku: "notebook", qty: 2u32 });

shopping-cart.line-count()
```

**Important**

- **Resource values are not arbitrary Wave text.** Printing or serialising them like ordinary JSON/Wave data is **not** supported the same way as records and numbers; treat them as **opaque** at the boundary unless your embedder defines extra behaviour.
- Some **patterns of nested construction** (e.g. certain inline combinations) may be rejected by the compiler with a specific error—when in doubt, use **`let`** to name intermediate values.

---

## Quick reference card

| Topic | Syntax / reminder |
|-------|-------------------|
| Sequence | `expr1; expr2; expr3` |
| Block | `{ … }` |
| Let | `let x = e` or `let x: T = e` |
| Inference | fixed-point over WIT; REPL tab completion (§5) |
| If | `if c then a else b` |
| Match | `match e { pat => x, _ => y }` |
| Compile-time | Exhaustive `match`, enum case names, call arity/types vs WIT (§9.8) |
| Call | `f(a, b)` or `recv.method(a)` |
| Instance | `let my-instance = instance();` then `my-instance.lookup-sku(7)` (name is yours) |
| For | `for x in xs { yield y; }` |
| Reduce | `reduce acc, x in xs from init { yield e; }` |
| Option | `none`, `some(x)` |
| Result | `ok(x)`, `err(x)` |
| Interpolate | `"Hello ${ name }"` |
| Resource | constructor + methods on the value; not generic Wave “print me” |

Names in this guide default to **[`example.wit`](example.wit)**. For any other component, substitute the **WIT** you actually loaded — that is what **`rib-lang`** type-checks against.
