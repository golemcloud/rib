# Rib language guide

Rib is a small **expression language** for **WebAssembly components**: types follow **WIT**, and many **literals** use **[Wasm Wave](https://github.com/bytecodealliance/wasm-wave)** text. The **WebAssembly** ecosystem sits mostly next to **Rust**, and we want people who already live there to feel **at home quickly**. Rib’s syntax **feels familiar**—**`let`**, blocks, **`if` / `then` / `else`**, **`match`**, and **dot syntax** for calling things on the value you got from **`instance()`**—the same mental model as **method calls on an instance** in Rust, Java, C#, Kotlin, and friends—so your existing habits carry over. The **grammar itself is intentionally minimal**; treat the opening sections of this guide as a **~5 minute** orientation, not a second language to memorise.

**Repository:** [github.com/golemcloud/rib](https://github.com/golemcloud/rib) · **Grammar (EBNF):** [rib-lang/README.md](../rib-lang/README.md)

**Why “Rib”?** — Think **potter’s rib**: the tiny paddle that saves your pot from thumbprints. Here, Rib is the paddle for **components**—same idea, less clay in your keyboard. **[Full story →](../README.md#why-the-name-rib)**

### Rib at the REPL (you do not need this whole guide first)

**The REPL is the main place Rib is meant to shine.** When the host has already **loaded your component’s WIT**, the prompt can **auto-complete** exports and **shape** arguments for you—almost everything you type is guided from that contract. The experience we aim for: **once you are in a wired-up REPL, you should not keep jumping back to WIT source files** for ordinary work; the session reflects the types you shipped, and you stay in flow.

Day-to-day use stays small: **`instance()`**, a **`let`** binding (e.g. **`let my-instance = instance();`**), then **exports** with dot syntax—**§1** is enough to be productive. You are still speaking the **component model’s** types and **Wasm Wave** literals, not inventing a parallel schema in your head.

### Rib in scripts and beside APIs (where the language earns its keep)

Rib also shines when you **run a program** against values that already crossed the boundary: **post-process** worker or component results, **reshape** nested **`record` / `list` / `option` / `result`** trees with **`match`** and expressions, and **test structure and transformations** instead of only asserting “something came back.” That is how you avoid hand-written ad-hoc checks that drift from your real WIT. Historically, runtimes such as **Golem** embedded Rib in **API-definition YAML** so a small script could **reshape the HTTP-facing output** of a component-backed worker—same pattern anywhere you want a **typed, short** script next to config rather than untyped glue.

If your world is **WIT**, **Wasmtime**, and **components as contracts**: Rib is a compact, **statically checked** expression layer that stays on those types end-to-end—useful whenever you want programmable glue **without** leaving the WebAssembly component ecosystem.

### About this guide (long on purpose, light in practice)

We document a lot so nothing feels hidden—but **Rib is not heavyweight**. For **REPL** work especially, **§0** (the teaching WIT at a glance), **§1** (`instance()` and calling exports), and **§2** (programs, blocks, and `;`) are usually **all you need** to feel fluent at the prompt, especially with completion wired to your component. Treat the rest of this file as a **reference** you reach for when you move into **scripts**, **`match`** on richer values, **resources**, comprehensions, or other **advanced** Rib—not a front-to-back assignment.

If you live around **Wasmtime** and the component stack: Rib is meant to be a **thin layer** on the types you already ship—**not** another big platform to adopt wholesale.

---

This guide lists **language features** you can use in scripts and in the **REPL**. It does **not** focus on **embedder-specific globals** (e.g. HTTP-shaped `request` inputs): those depend on how `rib-lang` is configured and may change or disappear between releases.

**Companion WIT** — **[`example.wit`](example.wit)** exports **`inventory`** (records, enums, plain funcs) and **`shopping`** (a **`cart`** **resource**) from world **`guide-demo`**. For a **fast path**, read **§0** then **§1** (and skim **§2** if you chain expressions); **§9** (`match`) and beyond are there when you need them.

---

## Table of contents

0. [Example WIT (`example.wit`)](#0-example-wit-examplewit)  
1. [`instance()` and calling exports](#1-instance-and-calling-exports)  
2. [Programs, blocks, and semicolons](#2-programs-blocks-and-semicolons)  
3. [Comments](#3-comments)  
4. [Literals and Wave-shaped values](#4-literals-and-wave-shaped-values)  
5. [Types and annotations](#5-types-and-annotations)  
6. [`let` bindings](#6-let-bindings)  
7. [Operators](#7-operators)  
8. [`if` / `then` / `else`](#8-if--then--else)  
9. [`match` and patterns](#9-match-and-patterns)  
10. [More call shapes](#10-more-call-shapes)  
11. [List comprehensions (`for` … `yield`)](#11-list-comprehensions-for--yield)  
12. [List aggregation (`reduce` … `yield`)](#12-list-aggregation-reduce--yield)  
13. [Records, tuples, lists, flags](#13-records-tuples-lists-flags)  
14. [`option` and `result`](#14-option-and-result)  
15. [String interpolation](#15-string-interpolation)  
16. [WIT resources](#16-wit-resources)  
17. [Qualified WIT export paths](#17-qualified-wit-export-paths)  

---

## 0. Example WIT (`example.wit`)

The file **[`example.wit`](example.wit)** is **documentation-only** (not wired into the compiler by default), but world **`guide-demo`** exports two interfaces so the guide can stay in one place:

- **`inventory`** — records, enums, variants, flags, and plain **`func`** exports (most examples below).
- **`shopping`** — a **`resource cart`** (constructor + methods) used in **§1** and **§16**; its shape is aligned with the **shopping-cart style** metadata in Rib’s own compiler tests (see **§16** for the path).

| WIT shape | Where | Meaning |
|-----------|--------|--------|
| `record` | `inventory` | `point`, `line-item`, … |
| `enum` | `inventory` | `order-stage`: `draft` \| `placed` \| `shipped` |
| `variant` | `inventory` | `payment-info`, … |
| `flags` | `inventory` | `file-access`: `read`, `write`, `execute` |
| `func` | `inventory` | `length`, `validate-qty`, `lookup-sku`, `ratio`, … |
| `resource` | `shopping` / `cart` | State per cart; **`constructor`**, **`line-count`**, **`add-line`** (uses **`inventory`.`line-item`**) |

How you **call** exports from Rib is in **§1**: **`let my-instance = instance();`** then **`my-instance.lookup-sku(...)`**, etc. WIT export names use **kebab-case** after the dot.

---

## 1. `instance()` and calling exports

**What `instance()` is (plain version):** Your host (REPL, test harness, etc.) has already loaded the Wasm **component** your **WIT** describes. **`instance()`** is the one call that says: “hand me the live object I should send export calls to.” Give that object a **`let`** name that reads nicely to you—this guide uses **`my-instance`** because it sits right next to **`instance()`** and stays easy to spot in snippets. Then call **exports** with **dot** syntax, same rhythm as methods on a value in Rust.

```rust
let my-instance = instance();
my-instance.lookup-sku(7)
```

That’s the whole pattern: one binding from **`instance()`**, then **`that-name.export-name(…)`**. Export names come from WIT and are usually **kebab-case** (`lookup-sku`, `format-stage`, …). Hyphens in **`let`** names are fine too (`store-main`, `lane-a`, …) if you prefer that style.

**More calls** against **[`example.wit`](example.wit)** → **`inventory`**:

```rust
let my-instance = instance();

let d = my-instance.length({ x: 3, y: -4 });
let blurb = my-instance.format-stage(draft);
let label = my-instance.lookup-sku(42); // `option<string>` — often `match` (§9.1)
let half = my-instance.ratio(9, 2);
let row = my-instance.make-item("pencil", 5u32);
let caps = my-instance.describe-access({ read, write });
```

**Resources** (e.g. **`shopping`**’s **`cart`** in the same world) use the same pattern: **`let shopping-cart = my-instance.cart("checkout-1");`** then **`shopping-cart.add-line(…)`**. Details in **§16**.

Exact lowering still depends on your embedder; **`example.wit`** is the reference. A richer **cart** API in compiler tests is linked from **§16**.

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

**Type ascription** on an expression: **`expr : type`**.

```rust
let n: s32 = 40;
let m: s32 = n + 2;
m
```

Common **scalar** type names: `bool`, `s8`, `u8`, `s16`, `u16`, `s32`, `u32`, `s64`, `u64`, `f32`, `f64`, `char`, `string`.

**Compound** types: `list<string>`, `tuple<s32, string>`, `option<u64>`, `result` / `result<string, string>`, etc.

---

## 6. `let` bindings

```rust
let answer = 42;
let labeled: u64 = 7u64;
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
if score > 10u64 then "win" else "lose"
```

All three parts are expressions and must type-check together.

---

## 9. `match` and patterns

`match` chooses an arm by **pattern** on a value. Arms are **`pattern => expr`**, separated by commas, inside `{ }`.  
Below, types come from **[`example.wit`](example.wit)** → **`inventory`**. Rib uses **Wave-shaped** literals and **WIT-derived** constructor / case names (often **kebab-case** where the WIT used hyphens).

### 9.1 `option` — e.g. return type of `lookup-sku`

`lookup-sku` returns `option<string>`. *(**`my-instance`** here is just an example name—use whatever reads best for you; see **§1**.)*

```rust
let my-instance = instance();
let label = my-instance.lookup-sku(42);

match label {
  some(name) => name,
  none => "unknown"
}
```

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

To turn a stage into a single string with a **function** instead of `match`, call **`format-stage`** (see **§1**).

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

To produce a **single string** from a `payment-info` value via an export, use **`summarize-payment`** on the same **`instance()`** binding you used elsewhere (**§1**).

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

- **`_`** — any value not covered by earlier arms (required if patterns are not exhaustive).  
- **`name @ pattern`** — bind the **whole** matched value to **`name`** while also matching **`pattern`**.

```rust
match home {
  p @ { x: xa, y: ya } => xa + ya
}
```

---

## 10. More call shapes

**§1** already covers **`let my-instance = instance();`** and **dot-syntax export calls** (same idea as method calls) such as **`my-instance.lookup-sku(42)`** and **`my-instance.format-stage(draft)`** against **[`example.wit`](example.wit)**.

Other shapes you may see:

- **Plain function call:** **`name(arg1, arg2)`** for functions registered with the compiler that are not spelled as **`receiver.`**… exports.
- **Qualified WIT paths** (package / interface / version / export) when a short name is ambiguous: **§17**.
- **Resources** and **`borrow`**: construction + methods as in **§16**.

Exact export **spelling** still follows **§0** and your embedder’s lowering from WIT.

---

## 11. List comprehensions (`for` … `yield`)

```rust
for word in ["hello", "world"] {
  yield word;
}
```

Optional **statements** may appear **before** `yield` in the `{ }` block (same idea as a small inner block).

---

## 12. List aggregation (`reduce` … `yield`)

```rust
reduce acc, x in [1, 2, 3] from 0 {
  yield acc + x;
}
```

Here **`acc`** is the accumulator, **`x`** is each element from the list after **`in`**, and **`from`** supplies the initial accumulator value.

---

## 13. Records, tuples, lists, flags

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

## 14. `option` and `result`

**Construction** (Wave-shaped; same shapes as **`lookup-sku`** / **`ratio`** results):

```rust
let ok-x = ok(42);
let err-x = err("by-zero");
let some-x = some("found");
let none-x = none;
```

**Destruction** — see **§9.1** (`option`) and **§9.2** (`result`) using the real **`inventory`** return types.

---

## 15. String interpolation

Inside `"…"`, **`${` … `}`** embeds a Rib **block** (sequence of expressions; the block’s value is inserted).

```rust
let name = "Rib";
"The language is ${name}"
```

---

## 16. WIT resources

**[`example.wit`](example.wit)** → interface **`shopping`** defines **`resource cart`**: **`constructor`**, **`line-count`**, **`add-line`**, using **`inventory`.`line-item`**. That is the **small teaching surface** for this repo’s docs.

Rib’s own **shopping-cart style** tests use a larger, programmatic WIT snapshot (interface path **`golem:it/api`**, exports such as **`[constructor]cart`**, **`[method]cart.add-item`**, **`[method]cart.checkout`**, **`[drop]cart`**, …). See **`rib-lang/src/compiler/mod.rs`**, function **`get_metadata`** in the nested **`test_utils`** module (roughly the **`test_invalid_resource_*`** tests and the **`resource_export`** / **`WitExport::Interface`** block that builds the cart API).

A WIT **`resource`** is a **typed object** you obtain from a **constructor** export on whatever **`instance()`** returned—same dot syntax as everything else (e.g. **`let my-instance = instance();`** then **`let shopping-cart = my-instance.cart("checkout-1");`**), then **methods** on **`shopping-cart`** (`add-line`, `line-count`, …, still **kebab-case** from WIT). Under the hood Rib lines this up with WIT’s **`borrow`** rules: the **first** parameter is the resource; Rib fills it in from the left-hand side of the **`.`**.

**Example aligned with `example.wit` / `shopping`:**

```rust
let my-instance = instance();
let shopping-cart = my-instance.cart("checkout-1");
shopping-cart.add-line({ sku: "notebook", qty: 2u32 });
shopping-cart.line-count()
```

**Important**

- **Resource values are not arbitrary Wave text.** Printing or serialising them like ordinary JSON/Wave data is **not** supported the same way as records and numbers; treat them as **opaque** at the boundary unless your embedder defines extra behaviour.
- Some **patterns of nested construction** (e.g. certain inline combinations) may be rejected by the compiler with a specific error—when in doubt, use **`let`** to name intermediate values.
- The **`golem:it/api`** test metadata is **not** the same file as **`example.wit`**; use it when you need a **richer** cart contract while debugging Rib itself.

---

## 17. Qualified WIT export paths

When you need to disambiguate **package / interface / version** and **function**, Rib supports **interface paths** in the grammar, roughly:

`package-namespace:package-name / interface-name @ version . { export }`

Inner **export** forms include plain functions, **`[constructor]`**, **`[method]`**, **`[static]`**, **`[drop]`**, etc., as generated from your WIT. Prefer **`let my-instance = instance();`** then **`my-instance.some-export()`** (kebab-case export names) in REPLs unless you must spell the full path.

---

## Quick reference card

| Topic | Syntax / reminder |
|-------|-------------------|
| Sequence | `expr1; expr2; expr3` |
| Block | `{ … }` |
| Let | `let x = e` or `let x: T = e` |
| If | `if c then a else b` |
| Match | `match e { pat => x, _ => y }` |
| Call | `f(a, b)` or `recv.method(a)` |
| Instance | `let my-instance = instance();` then `my-instance.lookup-sku(7)` (name is yours) |
| For | `for x in xs { yield y; }` |
| Reduce | `reduce acc, x in xs from init { yield e; }` |
| Option | `none`, `some(x)` |
| Result | `ok(x)`, `err(x)` |
| Interpolate | `"Hello ${ name }"` |
| Resource | constructor + methods on the value; not generic Wave “print me” |

Names in this guide default to **[`example.wit`](example.wit)**. For any other component, substitute the **WIT** you actually loaded — that is what **`rib-lang`** type-checks against.
