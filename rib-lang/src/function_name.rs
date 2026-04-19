use combine::stream::position::Stream;
use combine::{eof, EasyParser, Parser};

use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(PartialEq, Hash, Eq, Clone, Ord, PartialOrd)]
pub struct SemVer(pub semver::Version);

impl SemVer {
    pub fn parse(version: &str) -> Result<Self, String> {
        semver::Version::parse(version)
            .map(SemVer)
            .map_err(|e| format!("Invalid semver string: {e}"))
    }
}

impl std::fmt::Debug for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub enum ParsedFunctionSite {
    Global,
    Interface {
        name: String,
    },
    PackagedInterface {
        namespace: String,
        package: String,
        interface: String,
        version: Option<SemVer>,
    },
}

impl ParsedFunctionSite {
    pub fn parse(name: impl AsRef<str>) -> Result<Self, String> {
        ParsedFunctionName::parse(format!("{}.{{x}}", name.as_ref()))
            .map(|ParsedFunctionName { site, .. }| site)
    }

    pub fn interface_name(&self) -> Option<String> {
        match self {
            Self::Global => None,
            Self::Interface { name } => Some(name.clone()),
            Self::PackagedInterface {
                namespace,
                package,
                interface,
                version: None,
            } => Some(format!("{namespace}:{package}/{interface}")),
            Self::PackagedInterface {
                namespace,
                package,
                interface,
                version: Some(version),
            } => Some(format!("{namespace}:{package}/{interface}@{}", version.0)),
        }
    }

    pub fn unversioned(&self) -> ParsedFunctionSite {
        match self {
            ParsedFunctionSite::Global => ParsedFunctionSite::Global,
            ParsedFunctionSite::Interface { name } => {
                ParsedFunctionSite::Interface { name: name.clone() }
            }
            ParsedFunctionSite::PackagedInterface {
                namespace,
                package,
                interface,
                version: _,
            } => ParsedFunctionSite::PackagedInterface {
                namespace: namespace.clone(),
                package: package.clone(),
                interface: interface.clone(),
                version: None,
            },
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub enum DynamicParsedFunctionReference {
    Function { function: String },
    RawResourceConstructor { resource: String },
    RawResourceDrop { resource: String },
    RawResourceMethod { resource: String, method: String },
    RawResourceStaticMethod { resource: String, method: String },
}

impl DynamicParsedFunctionReference {
    pub fn name_pretty(&self) -> String {
        match self {
            DynamicParsedFunctionReference::Function { function, .. } => function.clone(),
            DynamicParsedFunctionReference::RawResourceConstructor { resource, .. } => {
                resource.to_string()
            }
            DynamicParsedFunctionReference::RawResourceDrop { .. } => "drop".to_string(),
            DynamicParsedFunctionReference::RawResourceMethod { method, .. } => method.to_string(),
            DynamicParsedFunctionReference::RawResourceStaticMethod { method, .. } => {
                method.to_string()
            }
        }
    }

    fn to_static(&self) -> ParsedFunctionReference {
        match self {
            Self::Function { function } => ParsedFunctionReference::Function {
                function: function.clone(),
            },
            Self::RawResourceConstructor { resource } => {
                ParsedFunctionReference::RawResourceConstructor {
                    resource: resource.clone(),
                }
            }
            Self::RawResourceDrop { resource } => ParsedFunctionReference::RawResourceDrop {
                resource: resource.clone(),
            },
            Self::RawResourceMethod { resource, method } => {
                ParsedFunctionReference::RawResourceMethod {
                    resource: resource.clone(),
                    method: method.clone(),
                }
            }
            Self::RawResourceStaticMethod { resource, method } => {
                ParsedFunctionReference::RawResourceStaticMethod {
                    resource: resource.clone(),
                    method: method.clone(),
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ParsedFunctionReference {
    Function { function: String },
    RawResourceConstructor { resource: String },
    RawResourceDrop { resource: String },
    RawResourceMethod { resource: String, method: String },
    RawResourceStaticMethod { resource: String, method: String },
}

impl Display for ParsedFunctionReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let function_name = match self {
            Self::Function { function } => function.clone(),
            Self::RawResourceConstructor { resource } => format!("{resource}.new"),
            Self::RawResourceMethod { resource, method } => format!("{resource}.{method}"),
            Self::RawResourceStaticMethod { resource, method } => {
                format!("[static]{resource}.{method}")
            }
            Self::RawResourceDrop { resource } => format!("{resource}.drop"),
        };

        write!(f, "{function_name}")
    }
}

impl ParsedFunctionReference {
    pub fn function_name(&self) -> String {
        match self {
            Self::Function { function, .. } => function.clone(),
            Self::RawResourceConstructor { resource, .. } => format!("[constructor]{resource}"),
            Self::RawResourceDrop { resource, .. } => format!("[drop]{resource}"),
            Self::RawResourceMethod {
                resource, method, ..
            } => format!("[method]{resource}.{method}"),
            Self::RawResourceStaticMethod {
                resource, method, ..
            } => format!("[static]{resource}.{method}"),
        }
    }

    pub fn resource_method_name(&self) -> Option<String> {
        match self {
            Self::RawResourceMethod { method, .. }
            | Self::RawResourceStaticMethod { method, .. } => Some(method.clone()),
            _ => None,
        }
    }

    pub fn method_as_static(&self) -> Option<ParsedFunctionReference> {
        match self {
            Self::RawResourceMethod { resource, method } => Some(Self::RawResourceStaticMethod {
                resource: resource.clone(),
                method: method.clone(),
            }),

            _ => None,
        }
    }

    pub fn resource_name(&self) -> Option<&String> {
        match self {
            Self::RawResourceConstructor { resource }
            | Self::RawResourceDrop { resource }
            | Self::RawResourceMethod { resource, .. }
            | Self::RawResourceStaticMethod { resource, .. } => Some(resource),
            _ => None,
        }
    }
}

// DynamicParsedFunctionName is different from ParsedFunctionName.
// In `DynamicParsedFunctionName` the resource parameters are `Expr` (Rib) while they are `String`
// in `ParsedFunctionName`.
// `Expr` implies the real values are yet to be computed, while `String`
// in ParsedFunctionName is a textual representation of the evaluated values.
// `Examples`:
// `DynamicParsedFunctionName` : ns:name/interface.{resource1(identifier1, { field-a: some(identifier2) }).new}
// `ParsedFunctionName` : ns:name/interface.{resource1("foo", { field-a: some("bar") }).new}
#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub struct DynamicParsedFunctionName {
    pub site: ParsedFunctionSite,
    pub function: DynamicParsedFunctionReference,
}

impl DynamicParsedFunctionName {
    pub fn parse(name: impl AsRef<str>) -> Result<Self, String> {
        let name = name.as_ref();

        let mut parser = crate::parser::call::function_name();

        let result = parser.easy_parse(Stream::new(name));

        match result {
            Ok((parsed, _)) => Ok(parsed),
            Err(error) => {
                let error_message = error.map_position(|p| p.to_string()).to_string();
                Err(error_message)
            }
        }
    }

    pub fn function_name_with_prefix_identifiers(&self) -> String {
        self.to_parsed_function_name().function.function_name()
    }

    // Usually resource name in the real metadata consist of prefixes such as [constructor]
    // However, the one obtained through the dynamic-parsed-function-name is simple without these prefix
    pub fn resource_name_simplified(&self) -> Option<String> {
        self.to_parsed_function_name()
            .function
            .resource_name()
            .cloned()
    }

    // Usually resource method in the real metadata consist of prefixes such as [method]
    pub fn resource_method_name_simplified(&self) -> Option<String> {
        self.to_parsed_function_name()
            .function
            .resource_method_name()
    }

    //
    pub fn to_parsed_function_name(&self) -> ParsedFunctionName {
        ParsedFunctionName {
            site: self.site.clone(),
            function: self.function.to_static(),
        }
    }
}

impl Display for DynamicParsedFunctionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let function_name = self.to_parsed_function_name().to_string();
        write!(f, "{function_name}")
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ParsedFunctionName {
    pub site: ParsedFunctionSite,
    pub function: ParsedFunctionReference,
}

impl Serialize for ParsedFunctionName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let function_name = self.to_string();
        serializer.serialize_str(&function_name)
    }
}

impl<'de> Deserialize<'de> for ParsedFunctionName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let function_name = <String as Deserialize>::deserialize(deserializer)?;
        ParsedFunctionName::parse(function_name).map_err(serde::de::Error::custom)
    }
}

impl Display for ParsedFunctionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let function_name = self
            .site
            .interface_name()
            .map_or(self.function.function_name(), |interface| {
                format!("{}.{{{}}}", interface, self.function)
            });
        write!(f, "{function_name}")
    }
}

impl ParsedFunctionName {
    pub fn new(site: ParsedFunctionSite, function: ParsedFunctionReference) -> Self {
        Self { site, function }
    }

    pub fn global(name: String) -> Self {
        Self {
            site: ParsedFunctionSite::Global,
            function: ParsedFunctionReference::Function { function: name },
        }
    }

    pub fn on_interface(interface: String, function: String) -> Self {
        Self {
            site: ParsedFunctionSite::Interface { name: interface },
            function: ParsedFunctionReference::Function { function },
        }
    }

    pub fn parse(name: impl AsRef<str>) -> Result<Self, String> {
        let name = name.as_ref();

        let mut parser = crate::parser::call::function_name().skip(eof());

        let result = parser.easy_parse(Stream::new(name));

        match result {
            Ok((parsed, _)) => Ok(parsed.to_parsed_function_name()),
            Err(error) => {
                let error_message = error.map_position(|p| p.to_string()).to_string();
                Err(error_message)
            }
        }
    }

    pub fn site(&self) -> &ParsedFunctionSite {
        &self.site
    }

    pub fn function(&self) -> &ParsedFunctionReference {
        &self.function
    }

    pub fn method_as_static(&self) -> Option<Self> {
        self.function.method_as_static().map(|function| Self {
            site: self.site.clone(),
            function,
        })
    }

    pub fn is_constructor(&self) -> Option<&str> {
        match &self.function {
            ParsedFunctionReference::RawResourceConstructor { resource, .. } => Some(resource),
            _ => None,
        }
    }

    pub fn is_method(&self) -> Option<&str> {
        match &self.function {
            ParsedFunctionReference::RawResourceMethod { resource, .. }
            | ParsedFunctionReference::RawResourceStaticMethod { resource, .. } => Some(resource),
            _ => None,
        }
    }

    pub fn is_static_method(&self) -> Option<&str> {
        match &self.function {
            ParsedFunctionReference::RawResourceStaticMethod { resource, .. } => Some(resource),
            _ => None,
        }
    }

    pub fn with_site(&self, site: ParsedFunctionSite) -> Self {
        Self {
            site,
            function: self.function.clone(),
        }
    }

    /// Segments for resolving a WebAssembly component export via nested instance names, then the
    /// function export name.
    ///
    /// Packaged WIT paths like `component:pkg/iface.{fn}` identify the interface in metadata, but
    /// component worlds usually export the interface **instance at the root** as `iface`, not as a
    /// nested export named `component:pkg`. Runtimes should walk `Component::get_export_index` with
    /// these segments (for example `["inventory", "lookup-sku"]`), not a path derived from splitting
    /// the full [`ParsedFunctionSite::interface_name`] string.
    pub fn wasm_component_export_path(&self) -> Vec<String> {
        let mut segments: Vec<String> = match &self.site {
            ParsedFunctionSite::Global => Vec::new(),
            ParsedFunctionSite::Interface { name } => name
                .split('/')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            ParsedFunctionSite::PackagedInterface { interface, .. } => vec![interface.clone()],
        };
        let leaf = match &self.function {
            ParsedFunctionReference::Function { function } => function.clone(),
            _ => self.function.function_name(),
        };
        segments.push(leaf);
        segments
    }

    /// Like [`wasm_component_export_path`](Self::wasm_component_export_path), but includes common
    /// alternate spellings for the **last** segment (WIT kebab-case vs snake_case in lowered names).
    pub fn wasm_component_export_path_candidates(&self) -> Vec<Vec<String>> {
        let primary = self.wasm_component_export_path();
        let mut out: Vec<Vec<String>> = Vec::new();
        let mut push = |p: Vec<String>| {
            if !out.iter().any(|e| e == &p) {
                out.push(p);
            }
        };
        push(primary.clone());
        if let Some(last) = primary.last() {
            let snake = last.replace('-', "_");
            if snake != *last {
                let mut alt = primary.clone();
                alt.pop();
                alt.push(snake);
                push(alt);
            }
        }
        out
    }
}

#[cfg(test)]
mod function_name_tests {
    use super::{ParsedFunctionName, ParsedFunctionReference, ParsedFunctionSite, SemVer};
    use test_r::test;

    #[test]
    fn parse_function_name_does_not_accept_partial_matches() {
        let result = ParsedFunctionName::parse("x:y/z");
        assert!(result.is_err());
    }

    #[test]
    fn parse_function_name_global() {
        let parsed = ParsedFunctionName::parse("run-example").expect("Parsing failed");
        assert_eq!(parsed.site().interface_name(), None);
        assert_eq!(parsed.function().function_name(), "run-example");
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::Global,
                function: ParsedFunctionReference::Function {
                    function: "run-example".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_in_exported_interface_no_package() {
        let parsed = ParsedFunctionName::parse("interface.{fn1}").expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("interface".to_string())
        );
        assert_eq!(parsed.function().function_name(), "fn1".to_string());
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::Interface {
                    name: "interface".to_string()
                },
                function: ParsedFunctionReference::Function {
                    function: "fn1".to_string()
                },
            }
        );
    }

    #[test]
    fn wasm_component_export_path_packaged_matches_world_root() {
        let parsed =
            ParsedFunctionName::parse("component:rib-smoke/inventory.{lookup-sku}").expect("parse");
        assert_eq!(
            parsed.wasm_component_export_path(),
            vec!["inventory", "lookup-sku"]
        );
        let cands = parsed.wasm_component_export_path_candidates();
        assert!(cands.iter().any(|p| p == &vec!["inventory", "lookup-sku"]));
        assert!(cands.iter().any(|p| p == &vec!["inventory", "lookup_sku"]));
    }

    #[test]
    fn parse_function_name_in_exported_interface() {
        let parsed = ParsedFunctionName::parse("ns:name/interface.{fn1}").expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(parsed.function().function_name(), "fn1".to_string());
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::Function {
                    function: "fn1".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_in_versioned_exported_interface() {
        let parsed = ParsedFunctionName::parse("wasi:cli/run@0.2.0.{run}").expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("wasi:cli/run@0.2.0".to_string())
        );
        assert_eq!(parsed.function().function_name(), "run".to_string());
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "wasi".to_string(),
                    package: "cli".to_string(),
                    interface: "run".to_string(),
                    version: Some(SemVer(semver::Version::new(0, 2, 0))),
                },
                function: ParsedFunctionReference::Function {
                    function: "run".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_constructor_syntax_sugar() {
        let parsed =
            ParsedFunctionName::parse("ns:name/interface.{resource1.new}").expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[constructor]resource1".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceConstructor {
                    resource: "resource1".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_constructor() {
        let parsed = ParsedFunctionName::parse("ns:name/interface.{[constructor]resource1}")
            .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[constructor]resource1".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceConstructor {
                    resource: "resource1".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_method_syntax_sugar() {
        let parsed = ParsedFunctionName::parse("ns:name/interface.{resource1.do-something}")
            .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[method]resource1.do-something".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceMethod {
                    resource: "resource1".to_string(),
                    method: "do-something".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_function_name_method() {
        let parsed =
            ParsedFunctionName::parse("ns:name/interface.{[method]resource1.do-something}")
                .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[method]resource1.do-something".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceMethod {
                    resource: "resource1".to_string(),
                    method: "do-something".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_function_name_static_method_syntax_sugar() {
        // Note: the syntax sugared version cannot distinguish between method and static - so we need to check the actual existence of
        // the function and fallback.
        let parsed = ParsedFunctionName::parse("ns:name/interface.{resource1.do-something-static}")
            .expect("Parsing failed")
            .method_as_static()
            .unwrap();
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[static]resource1.do-something-static".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceStaticMethod {
                    resource: "resource1".to_string(),
                    method: "do-something-static".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_function_name_static() {
        let parsed =
            ParsedFunctionName::parse("ns:name/interface.{[static]resource1.do-something-static}")
                .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[static]resource1.do-something-static".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceStaticMethod {
                    resource: "resource1".to_string(),
                    method: "do-something-static".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_function_name_drop_syntax_sugar() {
        let parsed = ParsedFunctionName::parse("ns:name/interface.{resource1.drop}")
            .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[drop]resource1".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceDrop {
                    resource: "resource1".to_string()
                },
            }
        );
    }

    #[test]
    fn parse_function_name_drop() {
        let parsed = ParsedFunctionName::parse("ns:name/interface.{[drop]resource1}")
            .expect("Parsing failed");
        assert_eq!(
            parsed.site().interface_name(),
            Some("ns:name/interface".to_string())
        );
        assert_eq!(
            parsed.function().function_name(),
            "[drop]resource1".to_string()
        );
        assert_eq!(
            parsed,
            ParsedFunctionName {
                site: ParsedFunctionSite::PackagedInterface {
                    namespace: "ns".to_string(),
                    package: "name".to_string(),
                    interface: "interface".to_string(),
                    version: None,
                },
                function: ParsedFunctionReference::RawResourceDrop {
                    resource: "resource1".to_string()
                },
            }
        );
    }

    fn round_trip_function_name_parse(input: &str) {
        let parsed = ParsedFunctionName::parse(input)
            .unwrap_or_else(|_| panic!("Input Parsing failed for {input}"));
        let parsed_written =
            ParsedFunctionName::parse(parsed.to_string()).expect("Round-trip parsing failed");
        assert_eq!(parsed, parsed_written);
    }

    #[test]
    fn test_parsed_function_name_display() {
        round_trip_function_name_parse("run-example");
        round_trip_function_name_parse("interface.{fn1}");
        round_trip_function_name_parse("wasi:cli/run@0.2.0.{run}");
        round_trip_function_name_parse("ns:name/interface.{resource1.new}");
        round_trip_function_name_parse("ns:name/interface.{[constructor]resource1}");
        round_trip_function_name_parse("ns:name/interface.{resource1.do-something}");
        round_trip_function_name_parse("ns:name/interface.{[static]resource1.do-something-static}");
        round_trip_function_name_parse("ns:name/interface.{resource1.drop}");
        round_trip_function_name_parse("ns:name/interface.{[drop]resource1}");
    }
}
