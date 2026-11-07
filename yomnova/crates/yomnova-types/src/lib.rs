//! Type system representations and relationships for YOMNOVA.

use std::fmt;
use yomnova_ast::TypeAnnotation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    Text,
    Unit,
    // Protocol-aware types
    IPv4,
    IPv6,
    Port,
    Endpoint,
    Packet,
    NetworkBuffer,
    TCP,
    UDP,
    // Compound
    Array(Box<Type>),
    /// A reference parameter type (`&T` when `false`, `&mut T` when `true`).
    Ref(Box<Type>, bool),
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Enum {
        name: String,
        variants: Vec<(String, Option<Type>)>,
    },
    Custom(String),
    Unknown,
}

impl Type {
    /// The type a reference parameter refers to (the type itself otherwise).
    pub fn referent(&self) -> &Type {
        match self {
            Type::Ref(inner, _) => inner,
            other => other,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    pub fn is_network_type(&self) -> bool {
        matches!(
            self,
            Type::IPv4
                | Type::IPv6
                | Type::Port
                | Type::Endpoint
                | Type::Packet
                | Type::NetworkBuffer
                | Type::TCP
                | Type::UDP
        )
    }

    pub fn from_annotation(anno: &TypeAnnotation) -> Self {
        match anno {
            TypeAnnotation::Int => Type::Int,
            TypeAnnotation::Float => Type::Float,
            TypeAnnotation::Bool => Type::Bool,
            TypeAnnotation::Text => Type::Text,
            TypeAnnotation::Unit => Type::Unit,
            TypeAnnotation::IPv4 => Type::IPv4,
            TypeAnnotation::IPv6 => Type::IPv6,
            TypeAnnotation::Port => Type::Port,
            TypeAnnotation::Endpoint => Type::Endpoint,
            TypeAnnotation::Packet => Type::Packet,
            TypeAnnotation::TCP => Type::TCP,
            TypeAnnotation::UDP => Type::UDP,
            TypeAnnotation::Array(inner) => Type::Array(Box::new(Self::from_annotation(inner))),
            TypeAnnotation::Ref(inner, m) => Type::Ref(Box::new(Self::from_annotation(inner)), *m),
            TypeAnnotation::Custom(name) if name == "NetworkBuffer" => Type::NetworkBuffer,
            TypeAnnotation::Custom(name) => Type::Custom(name.clone()),
        }
    }

    pub fn is_assignable_to(&self, target: &Type) -> bool {
        if self == target {
            return true;
        }
        if *target == Type::Unknown || *self == Type::Unknown {
            return true;
        }
        if let (Type::Array(a), Type::Array(b)) = (self, target) {
            return a.is_assignable_to(b);
        }
        false
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Text => write!(f, "Text"),
            Type::Unit => write!(f, "Unit"),
            Type::IPv4 => write!(f, "IPv4"),
            Type::IPv6 => write!(f, "IPv6"),
            Type::Port => write!(f, "Port"),
            Type::Endpoint => write!(f, "Endpoint"),
            Type::Packet => write!(f, "Packet"),
            Type::NetworkBuffer => write!(f, "NetworkBuffer"),
            Type::TCP => write!(f, "TCP"),
            Type::UDP => write!(f, "UDP"),
            Type::Array(t) => write!(f, "Array<{t}>"),
            Type::Ref(t, false) => write!(f, "&{t}"),
            Type::Ref(t, true) => write!(f, "&mut {t}"),
            Type::Function {
                params,
                return_type,
            } => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {return_type}")
            }
            Type::Struct { name, .. } => write!(f, "struct {name}"),
            Type::Enum { name, .. } => write!(f, "enum {name}"),
            Type::Custom(name) => write!(f, "{name}"),
            Type::Unknown => write!(f, "<unknown>"),
        }
    }
}

/// Native functions for `NetworkBuffer` and `Packet`: (name, parameter types, return type).
/// Shared by the type checker and the ownership checker.
pub fn net_builtins() -> Vec<(&'static str, Vec<Type>, Type)> {
    use Type::{Bool, Int, NetworkBuffer as Buf, Packet, Text};
    vec![
        ("NetworkBuffer", vec![Int], Buf),
        ("bytes_of", vec![Text], Buf),
        ("buf_get", vec![Buf, Int], Int),
        ("buf_get16", vec![Buf, Int], Int),
        ("buf_get32", vec![Buf, Int], Int),
        ("buf_set", vec![Buf, Int, Int], Buf),
        ("buf_set16", vec![Buf, Int, Int], Buf),
        ("buf_set32", vec![Buf, Int, Int], Buf),
        ("buf_slice", vec![Buf, Int, Int], Buf),
        ("buf_concat", vec![Buf, Buf], Buf),
        ("Packet", vec![Buf, Buf], Packet),
        ("packet_header", vec![Packet], Buf),
        ("packet_payload", vec![Packet], Buf),
        ("str_upper", vec![Text], Text),
        ("str_lower", vec![Text], Text),
        ("str_trim", vec![Text], Text),
        ("str_contains", vec![Text, Text], Bool),
        ("str_starts_with", vec![Text, Text], Bool),
        ("str_ends_with", vec![Text, Text], Bool),
        ("str_find", vec![Text, Text], Int),
        ("str_slice", vec![Text, Int, Int], Text),
        ("str_repeat", vec![Text, Int], Text),
        ("str_replace", vec![Text, Text, Text], Text),
        ("str_split", vec![Text, Text], Type::Array(Box::new(Text))),
        ("str_from_int", vec![Int], Text),
        ("str_parse_int", vec![Text], Int),
    ]
}

/// Net natives that only read their arguments (the checker treats the arguments as borrowed, not moved).
pub const NET_READ_ONLY: &[&str] = &[
    "buf_get",
    "buf_get16",
    "buf_get32",
    "buf_slice",
    "buf_concat",
    "packet_header",
    "packet_payload",
    "str_upper",
    "str_lower",
    "str_trim",
    "str_contains",
    "str_starts_with",
    "str_ends_with",
    "str_find",
    "str_slice",
    "str_repeat",
    "str_replace",
    "str_split",
    "str_parse_int",
];
