# rib-lang

Core library for the Rib language: parser, type inference, compiler, and interpreter.

## Grammar

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