// REOX Standard Library - Core Module
// Provides fundamental utility functions

use crate::interpreter::Value;

/// Get the length of a collection (array, string, map)
pub fn len(val: &Value) -> i64 {
    match val {
        Value::Array(arr) => arr.len() as i64,
        Value::String(s) => s.len() as i64,
        Value::Map(m) => m.len() as i64,
        _ => 0,
    }
}

/// Get the type name of a value
pub fn type_of(val: &Value) -> String {
    match val {
        Value::Nil => "nil".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Int(_) => "int".to_string(),
        Value::Float(_) => "float".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Map(_) => "map".to_string(),
        Value::Color { .. } => "color".to_string(),
        Value::Struct { name, .. } => format!("struct:{}", name),
        Value::NativeAction { .. } => "action".to_string(),
    }
}

/// Create a range of integers [start, end)
pub fn range(start: i64, end: i64) -> Vec<Value> {
    if start >= end {
        return Vec::new();
    }
    (start..end).map(Value::Int).collect()
}

/// Create a range with step
pub fn range_step(start: i64, end: i64, step: i64) -> Vec<Value> {
    if step == 0 || (step > 0 && start >= end) || (step < 0 && start <= end) {
        return Vec::new();
    }
    
    let mut result = Vec::new();
    let mut i = start;
    
    if step > 0 {
        while i < end {
            result.push(Value::Int(i));
            i += step;
        }
    } else {
        while i > end {
            result.push(Value::Int(i));
            i += step;
        }
    }
    result
}

/// Clamp a value between min and max
pub fn clamp(val: f64, min: f64, max: f64) -> f64 {
    if val < min { min } else if val > max { max } else { val }
}

/// Get the minimum of two values
pub fn min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// Get the maximum of two values
pub fn max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// Absolute value
pub fn abs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// Sign of a number (-1, 0, or 1)
pub fn sign(x: f64) -> i64 {
    if x > 0.0 { 1 } else if x < 0.0 { -1 } else { 0 }
}

/// Square root
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Power function
pub fn pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// Floor to integer
pub fn floor(x: f64) -> i64 {
    x.floor() as i64
}

/// Ceiling to integer
pub fn ceil(x: f64) -> i64 {
    x.ceil() as i64
}

/// Round to nearest integer
pub fn round(x: f64) -> i64 {
    x.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_len() {
        let arr = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(len(&arr), 3);
        
        let s = Value::String("hello".to_string());
        assert_eq!(len(&s), 5);
    }
    
    #[test]
    fn test_type_of() {
        assert_eq!(type_of(&Value::Int(42)), "int");
        assert_eq!(type_of(&Value::Float(3.14)), "float");
        assert_eq!(type_of(&Value::String("hi".to_string())), "string");
        assert_eq!(type_of(&Value::Bool(true)), "bool");
    }
    
    #[test]
    fn test_range() {
        let r = range(0, 5);
        assert_eq!(r.len(), 5);
        if let Value::Int(v) = &r[0] { assert_eq!(*v, 0); }
        if let Value::Int(v) = &r[4] { assert_eq!(*v, 4); }
    }
    
    #[test]
    fn test_clamp() {
        assert!((clamp(5.0, 0.0, 10.0) - 5.0).abs() < 0.001);
        assert!((clamp(-5.0, 0.0, 10.0) - 0.0).abs() < 0.001);
        assert!((clamp(15.0, 0.0, 10.0) - 10.0).abs() < 0.001);
    }
    
    #[test]
    fn test_math() {
        assert!((abs(-5.0) - 5.0).abs() < 0.001);
        assert_eq!(sign(-5.0), -1);
        assert_eq!(sign(5.0), 1);
        assert_eq!(sign(0.0), 0);
    }
}
