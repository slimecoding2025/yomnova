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
    TCP,
    UDP,
    // Compound
    Array(Box<Type>),
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
            Type::TCP => write!(f, "TCP"),
            Type::UDP => write!(f, "UDP"),
            Type::Array(t) => write!(f, "Array<{t}>"),
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
