# `rib-lang` 

`rib-lang` implements **Rib**: a compact expression language aligned with the [WebAssembly Component Model](https://component-model.bytecodealliance.org/) and **WIT**-shaped types, with value text compatible with **[Wasm Wave](https://github.com/bytecodealliance/wasm-wave)** where applicable. The crate provides the full pipeline—**parse**, **type inference**, **checking against embedder-supplied export metadata**, **compile**, and **interpret**—so component hosts can offer typed scripting without maintaining a parallel type system.

**Familiarity** — Rib’s **syntax is deliberately Rust-like** (`let`, `match`, blocks, calls, records, string syntax). Authors comfortable with Rust typically write well-formed Rib quickly. **Runtime literals** (records, lists, scalars, `option`, `result`, etc.) follow **Wasm Wave** text rules, so experience with the Wasm **component / WIT / Wave** stack carries over directly.

---

## Audience

- **Wasm-time and other runtime maintainers** integrating a typed shell, diagnostics command, or test harness on top of `wasmtime::component::…` (or equivalent): analysed types and `Val` / resource tables already exist in the embedding; Rib centralises turning **user-authored text** into those calls with **static checking** first.

- **Tooling authors** standardising on **Wave-shaped literals** for records, variants, lists, and `option` / `result` instead of per-product JSON or ad-hoc parsers.

- **Rust embeddings that already load components** via `wasmtime::component` (or similar): Rib occupies the space between hand-written marshalling for every scenario and embedding a general-purpose scripting runtime.
 
---

## Design constraints

1. **Types mirror WIT** — Scalars (`s32`, `string`, …), `list<T>`, `option<T>`, `result`, records, variant `match`, and qualified **export paths** in the grammar.

2. **Wave integration** — `WitType` implements `wasm_wave::wasm::WasmType`; `ValueAndType` implements `WasmValue`. Parsing and printing use **`wasm-wave`**, consistent with other Bytecode Alliance tooling. Resource **handles** are not treated as arbitrary Wave-printable values; the API documents that boundary.

3. **Pluggable invocation** — On `call`, the interpreter defers to **`RibComponentFunctionInvoke`** (see `interpreter`). The crate does not assume Wasmtime; it requires analysed exports and an embedder capable of performing the call.

---

## Subsystem overview

| Subsystem | Role |
|-----------|------|
| **Parser** | Rib source → AST |
| **Type inference & checker** | Programs checked against the embedder’s registry / `WitExport` view |
| **Compiler** | Lowers to bytecode consumed by the interpreter |
| **Interpreter** | Evaluation and invocation dispatch |
| **`wit_type`** | Structured representation of WIT-level types and exports |
| **`wave` / `wasm_wave_text`** | Wave bridge for `ValueAndType` |
| **`registry`** | Export and dependency metadata supplied to the compiler |

The semver-sensitive public API is defined by `rib-lang/src/lib.rs` (re-exports and modules).

---

## Embedding workflow

1. Obtain **analysed interface** data from the host stack (`wasmtime::component::…`, `wit-component`, etc.) and map it into Rib’s **`WitExport`** / **`WitType`** representation.

2. Construct a **registry** (and any instance or worker metadata required by the deployment model).

3. Run **parse → infer → check → compile**, then execute with an implementation of **`RibComponentFunctionInvoke`** that performs the actual cross-boundary call.

The **`rib-repl`** crate in this repository consumes the same pipeline for interactive input; CLIs and tests can call `rib-lang` directly without the REPL.

---

## Illustrative scenario

An embedding already holds a **`wasmtime::component::Instance`** (or equivalent). A line of text—entered at a REPL or read from a test fixture—such as a `checkout({ … })`-shaped expression is passed to `rib-lang` together with the export table. If the expression is ill-typed relative to WIT, the failure is reported **before** any Wasm entrypoint runs. If it type-checks, **`RibComponentFunctionInvoke`** maps the interpreted call to the host’s normal **`call`**, lifting/lowering, and resource rules.

---

## Further reading

- [WebAssembly Component Model — introduction](https://component-model.bytecodealliance.org/)  
- [WIT](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)  
- [Wasm Wave](https://github.com/bytecodealliance/wasm-wave)  
- Repository overview: [README.md](../README.md)  
- REPL built on this crate: [rib-repl/README.md](../rib-repl/README.md)  

---

## Formal grammar

The language syntax in EBNF-style form:

```
letter        ::= ? Unicode letter ? ;
digit         ::= "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
ident_start   ::= letter ;
ident_cont    ::= letter | digit | "_" | "-" ;
IDENT         ::= ident_start ident_cont* ;

(* trivia: whitespace; // /// line comments; /* */ /** */ block comments *)

program       ::= rib_block ;

rib_block     ::= rib_expr ( ";" rib_expr )* ;

rib_expr      ::= simple_expr ( ":" type_name )? rib_suffix* ;

rib_suffix    ::= index_suffix ( "." segment_suffix )* ( range_suffix )? ( bin_suffix )* ;

index_suffix  ::= ( "[" rib_expr "]" )* ;

segment_suffix ::= ( simple_expr index_suffix ( ":" type_name )? )
                | fraction_suffix ;

fraction_suffix ::= digit+ ( ( "e" | "E" ) ( "+" | "-" )? digit+ )? ;

range_suffix  ::= ( ".." | "..=" ) simple_expr? index_suffix ( ":" type_name )? ;

bin_suffix    ::= binary_op rib_expr index_suffix ;

binary_op     ::= ">=" | "<=" | "==" | "<" | ">" | "&&" | "||" | "+" | "-" | "*" | "/" ;

simple_expr   ::= list_comprehension
                | list_aggregation
                | pattern_match
                | let_binding
                | conditional
                | multi_line_block
                | flag_expr
                | record_expr
                | tuple_expr
                | boolean_literal
                | string_literal
                | not_expr
                | option_expr
                | result_expr
                | call_expr
                | sequence_expr
                | identifier_expr
                | integer_literal ;

let_binding   ::= "let" IDENT ( ":" type_name )? "=" rib_expr ;

conditional   ::= "if" rib_expr "then" rib_expr "else" rib_expr ;

pattern_match ::= "match" rib_expr "{" match_arm ( "," match_arm )* "}" ;

match_arm     ::= arm_pattern "=>" rib_expr ;

arm_pattern   ::= ctor_pattern
                | tuple_pat
                | list_pat
                | record_pat
                | "_"
                | pattern_alias
                | arm_literal ;

pattern_alias ::= IDENT "@" arm_pattern ;

ctor_pattern  ::= "none"
                | ctor_name "(" ( arm_pattern ( "," arm_pattern )* )? ")" ;

ctor_name     ::= ( letter | digit | "_" | "-" )+ ;

tuple_pat     ::= "(" ( arm_pattern ( "," arm_pattern )* )? ")" ;

list_pat      ::= "[" ( arm_pattern ( "," arm_pattern )* )? "]" ;

record_pat    ::= "{" key_pat ( "," key_pat )+ "}" ;

key_pat       ::= record_key ":" arm_pattern ;

record_key    ::= letter ( letter | "_" | "-" )* ;

arm_literal   ::= rib_expr ;

call_expr     ::= function_name "(" ( rib_expr ( "," rib_expr )* )? ")" ;

function_name ::= IDENT
                | interface_path ;

interface_path ::= ( ns_pkg "/" )? interface_name ( "@" semver )? "." "{" inner_function "}" ;

ns_pkg        ::= ident_segment ":" ident_segment ;

ident_segment ::= ident_piece+ ;

ident_piece   ::= ( letter | digit | "-" )+ ;

interface_name ::= ident_piece+ ;

semver        ::= ? text until ".{" ? ;

inner_function ::= raw_ctor | raw_drop | raw_method | raw_static | plain_fn ;

raw_ctor      ::= IDENT "." "new" | "[constructor]" IDENT ;
raw_drop      ::= IDENT "." "drop" | "[drop]" IDENT ;
raw_method    ::= IDENT "." IDENT | "[method]" IDENT "." IDENT ;
raw_static    ::= "[static]" IDENT "." IDENT ;
plain_fn      ::= IDENT ;

not_expr      ::= "!" rib_expr ;

option_expr   ::= "some" "(" rib_expr ")" | "none" ;

result_expr   ::= "ok" "(" rib_expr ")" | "err" "(" rib_expr ")" ;

tuple_expr    ::= "(" ( rib_expr ( "," rib_expr )* )? ")" ;

sequence_expr ::= "[" ( rib_expr ( "," rib_expr )* )? "]" ;

record_expr   ::= "{" field ( "," field )+ "}" ;

field         ::= field_key ":" rib_expr ;

field_key     ::= letter ( letter | digit | "_" | "-" )* ;

flag_expr     ::= "{" ( flag_name ( "," flag_name )* )? "}" ;

flag_name     ::= ( letter | "_" | digit | "-" )+ ;

boolean_literal ::= "true" | "false" ;

string_literal ::= "\"" string_char* "\"" ;

string_char   ::= ? any except "\" \"$" ? | escape | interpolation ;

escape        ::= "\\" ( "n" | "t" | "r" | "\\" | "\"" | "$" | "{" | "}" | ?any? ) ;

interpolation ::= "${" rib_block "}" ;

integer_literal ::= "-"? digit+ ;

multi_line_block ::= "{" rib_block "}" ;

list_comprehension ::= "for" IDENT "in" rib_expr "{"
                       block_body?
                       "yield" rib_expr ";"
                       "}" ;

list_aggregation ::= "reduce" IDENT "," IDENT "in" rib_expr "from" rib_expr "{"
                       block_body?
                       "yield" rib_expr ";"
                       "}" ;

block_body    ::= ( rib_expr ";" )* ;

identifier_expr ::= IDENT ;

type_name     ::= basic_type | list_type | tuple_type | option_type | result_type ;

basic_type    ::= "bool" | "s8" | "u8" | "s16" | "u16" | "s32" | "u32"
                | "s64" | "u64" | "f32" | "f64" | "char" | "string" ;

list_type     ::= "list" "<" type_name ">" ;
tuple_type    ::= "tuple" "<" type_name ( "," type_name )* ">" ;
option_type   ::= "option" "<" type_name ">" ;
result_type   ::= "result"
                | "result" "<" ( "_" | type_name ) ( "," type_name )? ">" ;

```
