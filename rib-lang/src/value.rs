#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    S8(i8),
    S16(i16),
    S32(i32),
    S64(i64),
    F32(f32),
    F64(f64),
    Char(char),
    String(String),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Record(Vec<Value>),
    Variant {
        case_idx: u32,
        case_value: Option<Box<Value>>,
    },
    Enum(u32),
    Flags(Vec<bool>),
    Option(Option<Box<Value>>),
    Result(Result<Option<Box<Value>>, Option<Box<Value>>>),
    /// Guest resource handle at the interpreter boundary.
    ///
    /// - `resource_id` — embedder-defined id (for example a table slot in Wasmtime).
    /// - `instance_name` — component **instance** key (`instance()` / `instance("x")`) used to route
    ///   host calls for resource methods. When empty, [`handle_instance_name`](Value::handle_instance_name)
    ///   falls back to the last `/` segment of `uri` for older embedders.
    Handle {
        uri: String,
        resource_id: u64,
        instance_name: String,
    },
}

impl Value {
    /// Instance key for routing resource method invocations to the correct host-side component
    /// instance. Prefers [`Handle::instance_name`]; if empty, uses the last `/`-separated segment of
    /// [`Handle::uri`].
    pub fn handle_instance_name(&self) -> Option<String> {
        match self {
            Value::Handle {
                uri,
                instance_name,
                resource_id: _,
            } => {
                if !instance_name.is_empty() {
                    Some(instance_name.clone())
                } else {
                    Some(
                        uri.rsplit_once('/')
                            .map(|(_, last)| last.to_string())
                            .unwrap_or_else(|| uri.clone()),
                    )
                }
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{v}"),
            Value::U8(v) => write!(f, "{v}"),
            Value::U16(v) => write!(f, "{v}"),
            Value::U32(v) => write!(f, "{v}"),
            Value::U64(v) => write!(f, "{v}"),
            Value::S8(v) => write!(f, "{v}"),
            Value::S16(v) => write!(f, "{v}"),
            Value::S32(v) => write!(f, "{v}"),
            Value::S64(v) => write!(f, "{v}"),
            Value::F32(v) => write!(f, "{v}"),
            Value::F64(v) => write!(f, "{v}"),
            Value::Char(v) => write!(f, "'{v}'"),
            Value::String(v) => write!(f, "\"{v}\""),
            Value::List(values) => {
                write!(f, "[")?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Tuple(values) => {
                write!(f, "(")?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Value::Record(values) => {
                write!(f, "{{")?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "}}")
            }
            Value::Variant {
                case_idx,
                case_value,
            } => {
                write!(f, "variant#{case_idx}")?;
                if let Some(v) = case_value {
                    write!(f, "({v})")?;
                }
                Ok(())
            }
            Value::Enum(idx) => write!(f, "enum#{idx}"),
            Value::Flags(flags) => {
                write!(f, "{{")?;
                let mut first = true;
                for (i, set) in flags.iter().enumerate() {
                    if *set {
                        if !first {
                            write!(f, ", ")?;
                        }
                        write!(f, "flag#{i}")?;
                        first = false;
                    }
                }
                write!(f, "}}")
            }
            Value::Option(None) => write!(f, "none"),
            Value::Option(Some(v)) => write!(f, "some({v})"),
            Value::Result(Ok(Some(v))) => write!(f, "ok({v})"),
            Value::Result(Ok(None)) => write!(f, "ok"),
            Value::Result(Err(Some(v))) => write!(f, "err({v})"),
            Value::Result(Err(None)) => write!(f, "err"),
            Value::Handle {
                uri,
                resource_id,
                instance_name,
            } => write!(f, "handle({uri}#{resource_id} @ {instance_name})",),
        }
    }
}
