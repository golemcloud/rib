//! Map a **call label** (what the Rib runtime passes to the host) to Wasm export path segments using
//! [`WitExport`] metadata only.
//!
//! REPL / Wasmtime flows use short names like `lookup-sku` or `inventory/lookup-sku`, not Golem-style
//! fully qualified [`ParsedFunctionName`] strings. This module does not parse that grammar.

use rib::wit_type::WitExport;

/// Component-type walks often include a leading `namespace:package` segment that is **not** a real
/// world export name; the world still exports interface instances at the root (`inventory`, …).
/// Embedders walk `get_export_index` with that world-local path, so we drop one leading segment when
/// it looks like a WIT package id (`:`).
fn strip_leading_packaged_namespace(mut path: Vec<String>) -> Vec<String> {
    if path.len() >= 2 && path[0].contains(':') {
        path.remove(0);
    }
    path
}

fn push_normalized_path(out: &mut Vec<Vec<String>>, path: Vec<String>) {
    let path = strip_leading_packaged_namespace(path);
    if !path.is_empty() && !out.iter().any(|e| e == &path) {
        out.push(path);
    }
}

/// All export paths derived from [`WitExport`] (nested instance names + function), in the same shape
/// embedders use with `get_export_index` — **after** normalizing away a leading packaged namespace
/// segment when present.
pub fn wasm_export_paths_from_wit(exports: &[WitExport]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for e in exports {
        match e {
            WitExport::Function(f) => push_normalized_path(&mut out, vec![f.name.clone()]),
            WitExport::Interface(iface) => {
                let prefix: Vec<String> = iface
                    .name
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                for f in &iface.functions {
                    let mut p = prefix.clone();
                    p.push(f.name.clone());
                    push_normalized_path(&mut out, p);
                }
            }
        }
    }
    out
}

struct ExportQuery {
    leaf: String,
    /// When `None`, only the function leaf is used (must be unique across exports).
    interface_prefix: Option<Vec<String>>,
}

/// Turn a runtime call label into a leaf name and optional interface path.
///
/// Accepted shapes (no `ParsedFunctionName` / Golem FQN parser):
/// - `lookup-sku` — function leaf only
/// - `inventory/lookup-sku` — interface path + function (segments separated by `/`)
/// - `inventory.{lookup-sku}` — optional brace form: text before `.{` gives the interface (last path
///   segment after `/` if present, else the whole prefix)
fn parse_call_label(label: &str) -> ExportQuery {
    let label = label.trim();

    if let Some(idx) = label.rfind(".{") {
        if let Some(rest) = label.get(idx + 2..) {
            if let Some(end) = rest.find('}') {
                let leaf = rest[..end].to_string();
                let before = label[..idx].trim_end_matches('.');
                let interface_prefix = if before.is_empty() {
                    None
                } else if let Some((_, last)) = before.rsplit_once('/') {
                    Some(vec![last.to_string()])
                } else {
                    Some(vec![before.to_string()])
                };
                return ExportQuery {
                    leaf,
                    interface_prefix,
                };
            }
        }
    }

    if label.contains('/') {
        let parts: Vec<&str> = label.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            let leaf = parts[parts.len() - 1].to_string();
            let prefix = parts[..parts.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect();
            return ExportQuery {
                leaf,
                interface_prefix: Some(prefix),
            };
        }
    }

    ExportQuery {
        leaf: label.to_string(),
        interface_prefix: None,
    }
}

fn names_equivalent_wit_abi(a: &str, b: &str) -> bool {
    a == b || a.replace('-', "_") == b.replace('-', "_")
}

/// Rib call sites use `resource.new` inside braces; WIT export metadata uses `[constructor]resource`
/// (see [`WitFunction::is_constructor`] in `rib-lang`).
fn call_leaf_matches_wit_export_name(call_leaf: &str, export_name: &str) -> bool {
    if names_equivalent_wit_abi(call_leaf, export_name) {
        return true;
    }
    if let Some(resource) = call_leaf.strip_suffix(".new") {
        if export_name.starts_with("[constructor]") {
            let res = &export_name["[constructor]".len()..];
            return names_equivalent_wit_abi(resource, res);
        }
    }
    false
}

fn path_matches_query(path: &[String], query: &ExportQuery) -> bool {
    let Some(last) = path.last() else {
        return false;
    };
    if !call_leaf_matches_wit_export_name(&query.leaf, last) {
        return false;
    }
    let iface = &path[..path.len().saturating_sub(1)];
    match &query.interface_prefix {
        None => true,
        Some(want) if want.is_empty() => true,
        Some(want) => iface == want.as_slice() || iface.ends_with(want.as_slice()),
    }
}

/// Resolve a call label to the unique Wasm export path for this component's [`WitExport`] list.
pub fn resolve_wasm_export_path(exports: &[WitExport], function_name: &str) -> Result<Vec<String>, String> {
    let paths = wasm_export_paths_from_wit(exports);
    let query = parse_call_label(function_name);
    let matches: Vec<&Vec<String>> = paths
        .iter()
        .filter(|p| path_matches_query(p, &query))
        .collect();
    match matches.len() {
        0 => {
            let sample: Vec<String> = paths
                .iter()
                .take(24)
                .map(|p| p.join("/"))
                .collect();
            Err(format!(
                "no export matches `{function_name}`. Example paths: [{}]",
                sample.join(", ")
            ))
        }
        1 => Ok(matches[0].clone()),
        _ => Err(format!(
            "ambiguous `{function_name}`: {} matching exports (use a path like `interface/function-name`)",
            matches.len()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rib::wit_type::{WitFunction, WitInterface};

    fn sample_inventory_export() -> Vec<WitExport> {
        vec![WitExport::Interface(WitInterface {
            name: "inventory".to_string(),
            functions: vec![WitFunction {
                name: "lookup-sku".to_string(),
                parameters: vec![],
                result: None,
            }],
        })]
    }

    #[test]
    fn bare_function_name_when_unique() {
        let exports = sample_inventory_export();
        let p = resolve_wasm_export_path(&exports, "lookup-sku").expect("resolve");
        assert_eq!(p, vec!["inventory", "lookup-sku"]);
    }

    #[test]
    fn interface_slash_function() {
        let exports = sample_inventory_export();
        let p = resolve_wasm_export_path(&exports, "inventory/lookup-sku").expect("resolve");
        assert_eq!(p, vec!["inventory", "lookup-sku"]);
    }

    #[test]
    fn brace_form_resolves_interface_leaf() {
        let exports = sample_inventory_export();
        let p = resolve_wasm_export_path(&exports, "inventory.{lookup-sku}").expect("resolve");
        assert_eq!(p, vec!["inventory", "lookup-sku"]);
    }

    /// Mirrors Wasmtime `component_exports`: interface key is `path[..-1].join("/")`, which can start
    /// with `component:pkg/...`. The real world export path must not keep the package segment.
    #[test]
    fn strips_leading_namespace_package_from_wit_metadata_paths() {
        let exports = vec![WitExport::Interface(WitInterface {
            name: "component:rib-smoke/inventory".to_string(),
            functions: vec![WitFunction {
                name: "lookup-sku".to_string(),
                parameters: vec![],
                result: None,
            }],
        })];
        let paths = wasm_export_paths_from_wit(&exports);
        assert_eq!(paths, vec![vec!["inventory", "lookup-sku"]]);

        let p = resolve_wasm_export_path(
            &exports,
            "component:rib-smoke/inventory.{lookup-sku}",
        )
        .expect("resolve");
        assert_eq!(p, vec!["inventory", "lookup-sku"]);
    }

    #[test]
    fn resource_constructor_cart_new_matches_braced_call() {
        let exports = vec![WitExport::Interface(WitInterface {
            name: "component:rib-smoke/shopping".to_string(),
            functions: vec![WitFunction {
                name: "[constructor]cart".to_string(),
                parameters: vec![],
                result: None,
            }],
        })];
        let p = resolve_wasm_export_path(
            &exports,
            "component:rib-smoke/shopping.{cart.new}",
        )
        .expect("resolve");
        assert_eq!(p, vec!["shopping", "[constructor]cart"]);
    }
}
