//! Standard library functions and native runtime helpers for YOMNOVA.

use yomnova_bytecode::Value;

pub fn std_math_abs(val: &Value) -> Option<Value> {
    match val {
        Value::Int(n) => Some(Value::Int(n.abs())),
        Value::Float(f) => Some(Value::Float(f.abs())),
        _ => None,
    }
}

pub fn std_text_len(val: &Value) -> Option<Value> {
    match val {
        Value::Text(s) => Some(Value::Int(s.len() as i64)),
        _ => None,
    }
}

pub fn std_array_len(val: &Value) -> Option<Value> {
    match val {
        Value::Array(a) => Some(Value::Int(a.len() as i64)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdlib_functions() {
        assert_eq!(std_math_abs(&Value::Int(-42)), Some(Value::Int(42)));
        assert_eq!(
            std_text_len(&Value::Text("hello".to_string())),
            Some(Value::Int(5))
        );
        assert_eq!(
            std_array_len(&Value::Array(vec![Value::Int(1), Value::Int(2)])),
            Some(Value::Int(2))
        );
    }
}
