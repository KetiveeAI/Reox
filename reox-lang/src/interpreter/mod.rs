// REOX Interpreter - Evaluates REOX AST for REPL mode

#![allow(dead_code)]

use crate::lexer::Span;
use crate::parser::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Nil, Bool(bool), Int(i64), Float(f64), String(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Color { r: u8, g: u8, b: u8, a: u8 },
    Struct { name: String, fields: HashMap<String, Value> },
    NativeAction(NativeFn),
    Closure { params: Vec<Param>, body: Block, env: Vec<HashMap<String, Value>> },
    Variant { type_name: String, case_name: String, payload: Vec<Value> },
}

/// Structural equality. Closures never compare equal; natives compare by
/// function pointer.
impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Color { r, g, b, a }, Value::Color { r: r2, g: g2, b: b2, a: a2 }) => {
                (r, g, b, a) == (r2, g2, b2, a2)
            }
            (Value::Struct { name, fields }, Value::Struct { name: n2, fields: f2 }) => name == n2 && fields == f2,
            (Value::NativeAction(a), Value::NativeAction(b)) => std::ptr::fn_addr_eq(*a, *b),
            (
                Value::Variant { type_name, case_name, payload },
                Value::Variant { type_name: t2, case_name: c2, payload: p2 },
            ) => type_name == t2 && case_name == c2 && payload == p2,
            _ => false,
        }
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self { Value::Nil => false, Value::Bool(b) => *b, Value::Int(i) => *i != 0, _ => true }
    }
    pub fn type_name(&self) -> &'static str {
        match self { Value::Nil => "nil", Value::Bool(_) => "bool", Value::Int(_) => "int",
                     Value::Float(_) => "float", Value::String(_) => "string", Value::Array(_) => "array",
                     Value::Map(_) => "map", Value::Color {..} => "color",
                     Value::Struct {..} => "struct", Value::NativeAction(_) => "native",
                     Value::Closure {..} => "closure", Value::Variant {..} => "variant" }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"), Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i), Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(a) => { write!(f, "[")?; for (i,v) in a.iter().enumerate() { if i>0 {write!(f,",")?;} write!(f,"{}",v)?; } write!(f, "]") },
            Value::Map(m) => { write!(f, "{{")?; for (i,(k,v)) in m.iter().enumerate() { if i>0 {write!(f,",")?;} write!(f,"{}:{}",k,v)?; } write!(f, "}}") },
            Value::Color{r,g,b,a} => write!(f, "rgba({},{},{},{})", r, g, b, a),
            Value::Struct{name,..} => write!(f, "<{}>", name),
            Value::NativeAction(_) => write!(f, "<native>"),
            Value::Closure {..} => write!(f, "<closure>"),
            Value::Variant { type_name, case_name, payload } => {
                write!(f, "{}.{}", type_name, case_name)?;
                if !payload.is_empty() {
                    write!(f, "(")?;
                    for (i, v) in payload.iter().enumerate() { if i > 0 { write!(f, ", ")?; } write!(f, "{}", v)?; }
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

/// Signature of a built-in function callable from REOX code.
pub type NativeFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

/// Largest array a range expression or `range()` call may materialise.
pub const MAX_RANGE_LEN: i64 = 10_000_000;

// === Argument helpers for native functions ===

fn arg<'a>(a: &'a [Value], i: usize, fname: &str) -> Result<&'a Value, RuntimeError> {
    a.get(i).ok_or_else(|| {
        RuntimeError::new(format!(
            "{}() expects at least {} argument{}, found {}",
            fname, i + 1, if i == 0 { "" } else { "s" }, a.len()
        ))
    })
}

fn type_mismatch(fname: &str, i: usize, expected: &str, found: &Value) -> RuntimeError {
    RuntimeError::new(format!(
        "{}() expects {} as argument {}, found {}",
        fname, expected, i + 1, found.type_name()
    ))
}

fn arg_int(a: &[Value], i: usize, fname: &str) -> Result<i64, RuntimeError> {
    match arg(a, i, fname)? {
        Value::Int(v) => Ok(*v),
        other => Err(type_mismatch(fname, i, "an int", other)),
    }
}

fn arg_float(a: &[Value], i: usize, fname: &str) -> Result<f64, RuntimeError> {
    match arg(a, i, fname)? {
        Value::Float(v) => Ok(*v),
        Value::Int(v) => Ok(*v as f64),
        other => Err(type_mismatch(fname, i, "a float", other)),
    }
}

fn arg_str<'a>(a: &'a [Value], i: usize, fname: &str) -> Result<&'a str, RuntimeError> {
    match arg(a, i, fname)? {
        Value::String(v) => Ok(v.as_str()),
        other => Err(type_mismatch(fname, i, "a string", other)),
    }
}

fn arg_array<'a>(a: &'a [Value], i: usize, fname: &str) -> Result<&'a Vec<Value>, RuntimeError> {
    match arg(a, i, fname)? {
        Value::Array(v) => Ok(v),
        other => Err(type_mismatch(fname, i, "an array", other)),
    }
}

fn arg_map<'a>(a: &'a [Value], i: usize, fname: &str) -> Result<&'a HashMap<String, Value>, RuntimeError> {
    match arg(a, i, fname)? {
        Value::Map(v) => Ok(v),
        other => Err(type_mismatch(fname, i, "a map", other)),
    }
}

/// Convert a REOX index to a Vec index, rejecting negative values.
fn index_of(v: i64, what: &str) -> Result<usize, RuntimeError> {
    usize::try_from(v).map_err(|_| RuntimeError::new(format!("{} must not be negative, got {}", what, v)))
}

fn channel(v: i64, name: &str) -> Result<u8, RuntimeError> {
    u8::try_from(v).map_err(|_| RuntimeError::new(format!("{} channel must be 0..=255, got {}", name, v)))
}

fn unit_interval(a: &[Value], fname: &str) -> Result<f64, RuntimeError> {
    Ok(arg_float(a, 0, fname)?.clamp(0.0, 1.0))
}

fn io_error(what: &str, path: &str, e: std::io::Error) -> RuntimeError {
    RuntimeError::new(format!("{} '{}': {}", what, path, e))
}

fn epoch_duration() -> Result<std::time::Duration, RuntimeError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| RuntimeError::new(format!("system clock is before the Unix epoch: {}", e)))
}

fn range_values(start: i64, end_inclusive: i64) -> Result<Vec<Value>, RuntimeError> {
    if start > end_inclusive {
        return Ok(Vec::new());
    }
    let len = end_inclusive
        .checked_sub(start)
        .and_then(|d| d.checked_add(1))
        .ok_or_else(|| RuntimeError::new("range is too large to represent"))?;
    if len > MAX_RANGE_LEN {
        return Err(RuntimeError::new(format!(
            "range {}..{} has {} elements, more than the {} element limit",
            start, end_inclusive, len, MAX_RANGE_LEN
        )));
    }
    Ok((start..=end_inclusive).map(Value::Int).collect())
}

fn flush_stdout() -> Result<(), RuntimeError> {
    std::io::Write::flush(&mut std::io::stdout())
        .map_err(|e| RuntimeError::new(format!("failed to flush stdout: {}", e)))
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => (x - y).abs() < f64::EPSILON,
        (Value::String(x), Value::String(y)) => x == y,
        _ => false,
    }
}

/// Names of every built-in function the interpreter provides. The type
/// checker uses this list so that calls to runtime natives are not reported
/// as undefined variables.
pub fn native_names() -> impl Iterator<Item = &'static str> {
    NATIVES.iter().map(|(name, _)| *name)
}

#[derive(Debug, Clone)]
pub struct Environment { scopes: Vec<HashMap<String, Value>> }

impl Environment {
    pub fn new() -> Self {
        let mut e = Self { scopes: vec![HashMap::new()] };
        for (name, f) in NATIVES {
            e.define(name, Value::NativeAction(*f));
        }
        e
    }

    pub fn push(&mut self) { self.scopes.push(HashMap::new()); }
    pub fn pop(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }
    pub fn define(&mut self, n: &str, v: Value) {
        if self.scopes.is_empty() {
            self.scopes.push(HashMap::new());
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(n.into(), v);
        }
    }
    pub fn get(&self, n: &str) -> Option<Value> { self.scopes.iter().rev().find_map(|s| s.get(n).cloned()) }
    pub fn set(&mut self, n: &str, v: Value) -> bool {
        for s in self.scopes.iter_mut().rev() { if s.contains_key(n) { s.insert(n.into(), v); return true; } }
        false
    }
}

impl Default for Environment { fn default() -> Self { Self::new() } }

/// Built-in functions available to every REOX program in the interpreter.
/// Every native validates its arguments and reports a RuntimeError instead
/// of indexing blindly or returning a silent default.
static NATIVES: &[(&str, NativeFn)] = &[
    // I/O
    ("print", |a| {
        for x in &a { print!("{}", x); }
        flush_stdout()?;
        Ok(Value::Nil)
    }),
    ("println", |a| {
        for x in &a { println!("{}", x); }
        Ok(Value::Nil)
    }),
    ("input", |a| {
        if let Some(prompt) = a.first() {
            print!("{}", prompt);
            flush_stdout()?;
        }
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line)
            .map_err(|e| RuntimeError::new(format!("failed to read stdin: {}", e)))?;
        Ok(Value::String(line.trim_end().to_string()))
    }),

    // Collections
    ("len", |a| match arg(&a, 0, "len")? {
        Value::Array(v) => Ok(Value::Int(v.len() as i64)),
        Value::String(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::Map(m) => Ok(Value::Int(m.len() as i64)),
        other => Err(type_mismatch("len", 0, "an array, string or map", other)),
    }),
    ("push", |a| {
        let mut arr = arg_array(&a, 0, "push")?.clone();
        arr.push(arg(&a, 1, "push")?.clone());
        Ok(Value::Array(arr))
    }),
    ("pop", |a| {
        let mut arr = arg_array(&a, 0, "pop")?.clone();
        Ok(arr.pop().unwrap_or(Value::Nil))
    }),
    ("array_get", |a| {
        let arr = arg_array(&a, 0, "array_get")?;
        let idx = arg_int(&a, 1, "array_get")?;
        Ok(usize::try_from(idx).ok().and_then(|i| arr.get(i)).cloned().unwrap_or(Value::Nil))
    }),
    ("array_set", |a| {
        let mut arr = arg_array(&a, 0, "array_set")?.clone();
        let idx = index_of(arg_int(&a, 1, "array_set")?, "array index")?;
        let value = arg(&a, 2, "array_set")?.clone();
        match arr.get_mut(idx) {
            Some(slot) => { *slot = value; Ok(Value::Array(arr)) }
            None => Err(RuntimeError::new(format!("array index {} out of bounds (length {})", idx, arr.len()))),
        }
    }),
    ("array_contains", |a| {
        let arr = arg_array(&a, 0, "array_contains")?;
        let target = arg(&a, 1, "array_contains")?;
        Ok(Value::Bool(arr.iter().any(|item| values_equal(item, target))))
    }),
    ("array_slice", |a| {
        let arr = arg_array(&a, 0, "array_slice")?;
        let start = index_of(arg_int(&a, 1, "array_slice")?, "slice start")?.min(arr.len());
        let end = index_of(arg_int(&a, 2, "array_slice")?, "slice end")?.min(arr.len());
        if start > end {
            return Err(RuntimeError::new(format!("slice start {} is after slice end {}", start, end)));
        }
        Ok(Value::Array(arr[start..end].to_vec()))
    }),
    ("range", |a| {
        let start = arg_int(&a, 0, "range")?;
        let end = arg_int(&a, 1, "range")?;
        Ok(Value::Array(range_values(start, end.saturating_sub(1))?))
    }),

    // Maps
    ("map_new", |_| Ok(Value::Map(HashMap::new()))),
    ("map_set", |a| {
        let mut m = arg_map(&a, 0, "map_set")?.clone();
        let k = arg_str(&a, 1, "map_set")?.to_string();
        m.insert(k, arg(&a, 2, "map_set")?.clone());
        Ok(Value::Map(m))
    }),
    ("map_get", |a| {
        let m = arg_map(&a, 0, "map_get")?;
        let k = arg_str(&a, 1, "map_get")?;
        Ok(m.get(k).cloned().unwrap_or(Value::Nil))
    }),
    ("map_has", |a| {
        let m = arg_map(&a, 0, "map_has")?;
        Ok(Value::Bool(m.contains_key(arg_str(&a, 1, "map_has")?)))
    }),
    ("map_remove", |a| {
        let mut m = arg_map(&a, 0, "map_remove")?.clone();
        m.remove(arg_str(&a, 1, "map_remove")?);
        Ok(Value::Map(m))
    }),
    ("map_keys", |a| {
        let m = arg_map(&a, 0, "map_keys")?;
        let mut keys: Vec<&String> = m.keys().collect();
        keys.sort();
        Ok(Value::Array(keys.into_iter().map(|k| Value::String(k.clone())).collect()))
    }),

    // Colors
    ("rgba", |a| Ok(Value::Color {
        r: channel(arg_int(&a, 0, "rgba")?, "red")?,
        g: channel(arg_int(&a, 1, "rgba")?, "green")?,
        b: channel(arg_int(&a, 2, "rgba")?, "blue")?,
        a: channel(arg_int(&a, 3, "rgba")?, "alpha")?,
    })),
    ("rgb", |a| Ok(Value::Color {
        r: channel(arg_int(&a, 0, "rgb")?, "red")?,
        g: channel(arg_int(&a, 1, "rgb")?, "green")?,
        b: channel(arg_int(&a, 2, "rgb")?, "blue")?,
        a: 255,
    })),
    ("hex", |a| {
        let s = arg_str(&a, 0, "hex")?;
        let digits = s.trim_start_matches('#');
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(RuntimeError::new(format!("hex() expects 6 hex digits like \"#RRGGBB\", got \"{}\"", s)));
        }
        let parse = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&digits[range], 16)
                .map_err(|e| RuntimeError::new(format!("invalid hex color \"{}\": {}", s, e)))
        };
        Ok(Value::Color { r: parse(0..2)?, g: parse(2..4)?, b: parse(4..6)?, a: 255 })
    }),
    ("hsl", |a| {
        let h = arg_float(&a, 0, "hsl")?;
        let s = arg_float(&a, 1, "hsl")? / 100.0;
        let l = arg_float(&a, 2, "hsl")? / 100.0;
        if !h.is_finite() || !s.is_finite() || !l.is_finite() {
            return Err(RuntimeError::new("hsl() arguments must be finite numbers"));
        }
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let (r1, g1, b1) = match h.rem_euclid(360.0) as i32 {
            0..=59 => (c, x, 0.0),
            60..=119 => (x, c, 0.0),
            120..=179 => (0.0, c, x),
            180..=239 => (0.0, x, c),
            240..=299 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to_u8 = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Ok(Value::Color { r: to_u8(r1), g: to_u8(g1), b: to_u8(b1), a: 255 })
    }),
    ("color_primary", |_| Ok(Value::Color { r: 0, g: 122, b: 255, a: 255 })),
    ("color_secondary", |_| Ok(Value::Color { r: 88, g: 86, b: 214, a: 255 })),
    ("color_success", |_| Ok(Value::Color { r: 52, g: 199, b: 89, a: 255 })),
    ("color_warning", |_| Ok(Value::Color { r: 255, g: 149, b: 0, a: 255 })),
    ("color_danger", |_| Ok(Value::Color { r: 255, g: 59, b: 48, a: 255 })),
    ("color_background", |_| Ok(Value::Color { r: 28, g: 28, b: 30, a: 255 })),
    ("color_surface", |_| Ok(Value::Color { r: 44, g: 44, b: 46, a: 255 })),
    ("color_text", |_| Ok(Value::Color { r: 255, g: 255, b: 255, a: 255 })),
    ("color_text_dim", |_| Ok(Value::Color { r: 142, g: 142, b: 147, a: 255 })),

    // Strings
    ("str_split", |a| {
        let s = arg_str(&a, 0, "str_split")?;
        let delim = arg_str(&a, 1, "str_split")?;
        if delim.is_empty() {
            return Ok(Value::Array(s.chars().map(|c| Value::String(c.to_string())).collect()));
        }
        Ok(Value::Array(s.split(delim).map(|p| Value::String(p.to_string())).collect()))
    }),
    ("str_join", |a| {
        let arr = arg_array(&a, 0, "str_join")?;
        let sep = arg_str(&a, 1, "str_join")?;
        let parts: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
        Ok(Value::String(parts.join(sep)))
    }),
    ("str_trim", |a| Ok(Value::String(arg_str(&a, 0, "str_trim")?.trim().to_string()))),
    ("str_upper", |a| Ok(Value::String(arg_str(&a, 0, "str_upper")?.to_uppercase()))),
    ("str_lower", |a| Ok(Value::String(arg_str(&a, 0, "str_lower")?.to_lowercase()))),
    ("str_contains", |a| {
        let s = arg_str(&a, 0, "str_contains")?;
        Ok(Value::Bool(s.contains(arg_str(&a, 1, "str_contains")?)))
    }),
    ("str_replace", |a| {
        let s = arg_str(&a, 0, "str_replace")?;
        let from = arg_str(&a, 1, "str_replace")?;
        let to = arg_str(&a, 2, "str_replace")?;
        Ok(Value::String(s.replace(from, to)))
    }),
    ("string_concat", |a| {
        let s1 = a.first().map(|v| v.to_string()).unwrap_or_default();
        let s2 = a.get(1).map(|v| v.to_string()).unwrap_or_default();
        Ok(Value::String(format!("{}{}", s1, s2)))
    }),
    ("int_to_string", |a| Ok(Value::String(arg(&a, 0, "int_to_string")?.to_string()))),
    ("float_to_string", |a| Ok(Value::String(arg(&a, 0, "float_to_string")?.to_string()))),

    // Math
    ("abs", |a| match arg(&a, 0, "abs")? {
        Value::Int(i) => i.checked_abs().map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in abs()")),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        other => Err(type_mismatch("abs", 0, "a number", other)),
    }),
    ("min", |a| match (arg(&a, 0, "min")?, arg(&a, 1, "min")?) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.min(y))),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x.min(*y))),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float((*x as f64).min(*y))),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x.min(*y as f64))),
        (_, other) => Err(type_mismatch("min", 1, "two numbers", other)),
    }),
    ("max", |a| match (arg(&a, 0, "max")?, arg(&a, 1, "max")?) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.max(y))),
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x.max(*y))),
        (Value::Int(x), Value::Float(y)) => Ok(Value::Float((*x as f64).max(*y))),
        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x.max(*y as f64))),
        (_, other) => Err(type_mismatch("max", 1, "two numbers", other)),
    }),
    ("sqrt", |a| Ok(Value::Float(arg_float(&a, 0, "sqrt")?.sqrt()))),
    ("pow", |a| match (arg(&a, 0, "pow")?, arg(&a, 1, "pow")?) {
        (Value::Int(base), Value::Int(exp)) => {
            let exp = u32::try_from(*exp)
                .map_err(|_| RuntimeError::new(format!("pow() integer exponent must be 0..=4294967295, got {}", exp)))?;
            base.checked_pow(exp).map(Value::Int)
                .ok_or_else(|| RuntimeError::new(format!("integer overflow in pow({}, {})", base, exp)))
        }
        (Value::Float(base), Value::Float(exp)) => Ok(Value::Float(base.powf(*exp))),
        (Value::Int(base), Value::Float(exp)) => Ok(Value::Float((*base as f64).powf(*exp))),
        (Value::Float(base), Value::Int(exp)) => Ok(Value::Float(base.powf(*exp as f64))),
        (_, other) => Err(type_mismatch("pow", 1, "two numbers", other)),
    }),
    ("floor", |a| Ok(Value::Int(arg_float(&a, 0, "floor")?.floor() as i64))),
    ("ceil", |a| Ok(Value::Int(arg_float(&a, 0, "ceil")?.ceil() as i64))),
    ("round", |a| Ok(Value::Int(arg_float(&a, 0, "round")?.round() as i64))),
    ("clamp", |a| match (arg(&a, 0, "clamp")?, arg(&a, 1, "clamp")?, arg(&a, 2, "clamp")?) {
        (Value::Int(v), Value::Int(lo), Value::Int(hi)) if lo <= hi => Ok(Value::Int((*v).max(*lo).min(*hi))),
        (Value::Float(v), Value::Float(lo), Value::Float(hi)) if lo <= hi => Ok(Value::Float(v.max(*lo).min(*hi))),
        (Value::Int(_), Value::Int(lo), Value::Int(hi)) => Err(RuntimeError::new(format!("clamp() lower bound {} is above upper bound {}", lo, hi))),
        (Value::Float(_), Value::Float(lo), Value::Float(hi)) => Err(RuntimeError::new(format!("clamp() lower bound {} is above upper bound {}", lo, hi))),
        (other, _, _) => Err(type_mismatch("clamp", 0, "three ints or three floats", other)),
    }),
    ("sign", |a| match arg(&a, 0, "sign")? {
        Value::Int(i) => Ok(Value::Int(i.signum())),
        Value::Float(f) => Ok(Value::Int(if *f > 0.0 { 1 } else if *f < 0.0 { -1 } else { 0 })),
        other => Err(type_mismatch("sign", 0, "a number", other)),
    }),
    ("random_int", |a| {
        let min = arg_int(&a, 0, "random_int")?;
        let max = arg_int(&a, 1, "random_int")?;
        if min > max {
            return Err(RuntimeError::new(format!("random_int() lower bound {} is above upper bound {}", min, max)));
        }
        let span = (max as i128 - min as i128 + 1) as u128;
        let nanos = epoch_duration()?.as_nanos();
        Ok(Value::Int((min as i128 + (nanos % span) as i128) as i64))
    }),

    // Conversions
    ("int", |a| match arg(&a, 0, "int")? {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::String(s) => Ok(s.trim().parse::<i64>().map(Value::Int).unwrap_or(Value::Nil)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        other => Err(type_mismatch("int", 0, "an int, float, string or bool", other)),
    }),
    ("float", |a| match arg(&a, 0, "float")? {
        Value::Int(i) => Ok(Value::Float(*i as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::String(s) => Ok(s.trim().parse::<f64>().map(Value::Float).unwrap_or(Value::Nil)),
        other => Err(type_mismatch("float", 0, "an int, float or string", other)),
    }),
    ("str", |a| Ok(Value::String(a.first().map(|v| v.to_string()).unwrap_or_default()))),
    ("bool", |a| Ok(Value::Bool(a.first().map(|v| v.is_truthy()).unwrap_or(false)))),
    ("type_of", |a| Ok(Value::String(arg(&a, 0, "type_of")?.type_name().to_string()))),

    // AI
    ("ai_generate", |a| Ok(crate::stdlib::ai::generate(a))),
    ("ai_complete", |a| Ok(Value::String(crate::stdlib::ai::ai_complete(arg_str(&a, 0, "ai_complete")?)))),
    ("ai_explain", |a| Ok(Value::String(crate::stdlib::ai::ai_explain(arg_str(&a, 0, "ai_explain")?)))),
    ("ai_fix", |a| {
        let err = arg_str(&a, 0, "ai_fix")?;
        let ctx = a.get(1).map(|v| v.to_string()).unwrap_or_default();
        Ok(Value::String(crate::stdlib::ai::ai_fix(err, &ctx)))
    }),
    ("ai_ui", |a| Ok(Value::String(crate::stdlib::ai::ai_ui(arg_str(&a, 0, "ai_ui")?)))),
    ("ai_review", |a| Ok(Value::String(crate::stdlib::ai::ai_review(arg_str(&a, 0, "ai_review")?)))),

    // Animation easing
    ("ease_linear", |a| Ok(Value::Float(unit_interval(&a, "ease_linear")?))),
    ("ease_in", |a| { let t = unit_interval(&a, "ease_in")?; Ok(Value::Float(t * t)) }),
    ("ease_out", |a| { let t = unit_interval(&a, "ease_out")?; Ok(Value::Float(1.0 - (1.0 - t) * (1.0 - t))) }),
    ("ease_in_out", |a| {
        let t = unit_interval(&a, "ease_in_out")?;
        Ok(Value::Float(if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }))
    }),
    ("lerp", |a| {
        let from = arg_float(&a, 0, "lerp")?;
        let to = arg_float(&a, 1, "lerp")?;
        let t = arg_float(&a, 2, "lerp")?.clamp(0.0, 1.0);
        Ok(Value::Float(from + (to - from) * t))
    }),

    // Files
    ("file_read", |a| {
        let path = arg_str(&a, 0, "file_read")?;
        std::fs::read_to_string(path).map(Value::String).map_err(|e| io_error("failed to read", path, e))
    }),
    ("file_write", |a| {
        let path = arg_str(&a, 0, "file_write")?;
        let content = arg_str(&a, 1, "file_write")?;
        std::fs::write(path, content).map(|_| Value::Bool(true)).map_err(|e| io_error("failed to write", path, e))
    }),
    ("file_append", |a| {
        let path = arg_str(&a, 0, "file_append")?;
        let content = arg_str(&a, 1, "file_append")?;
        let mut f = std::fs::OpenOptions::new().append(true).create(true).open(path)
            .map_err(|e| io_error("failed to open", path, e))?;
        std::io::Write::write_all(&mut f, content.as_bytes())
            .map(|_| Value::Bool(true))
            .map_err(|e| io_error("failed to append to", path, e))
    }),
    ("file_exists", |a| Ok(Value::Bool(std::path::Path::new(arg_str(&a, 0, "file_exists")?).exists()))),
    ("is_file", |a| Ok(Value::Bool(std::path::Path::new(arg_str(&a, 0, "is_file")?).is_file()))),
    ("is_dir", |a| Ok(Value::Bool(std::path::Path::new(arg_str(&a, 0, "is_dir")?).is_dir()))),
    ("file_delete", |a| {
        let path = arg_str(&a, 0, "file_delete")?;
        std::fs::remove_file(path).map(|_| Value::Bool(true)).map_err(|e| io_error("failed to delete", path, e))
    }),
    ("file_size", |a| {
        let path = arg_str(&a, 0, "file_size")?;
        std::fs::metadata(path)
            .map(|m| Value::Int(i64::try_from(m.len()).unwrap_or(i64::MAX)))
            .map_err(|e| io_error("failed to stat", path, e))
    }),
    ("create_dir", |a| {
        let path = arg_str(&a, 0, "create_dir")?;
        std::fs::create_dir_all(path).map(|_| Value::Bool(true)).map_err(|e| io_error("failed to create directory", path, e))
    }),
    ("dir_list", |a| {
        let path = arg_str(&a, 0, "dir_list")?;
        let entries = std::fs::read_dir(path).map_err(|e| io_error("failed to list", path, e))?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_error("failed to list", path, e))?;
            names.push(Value::String(entry.file_name().to_string_lossy().into_owned()));
        }
        Ok(Value::Array(names))
    }),

    // Time
    ("time_now", |_| Ok(Value::Int(i64::try_from(epoch_duration()?.as_secs()).unwrap_or(i64::MAX)))),
    ("time_millis", |_| Ok(Value::Int(i64::try_from(epoch_duration()?.as_millis()).unwrap_or(i64::MAX)))),
    ("time_sleep", |a| {
        let ms = arg_int(&a, 0, "time_sleep")?;
        let ms = u64::try_from(ms).map_err(|_| RuntimeError::new(format!("time_sleep() duration must not be negative, got {}", ms)))?;
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(Value::Nil)
    }),

    // Environment and process
    ("env_get", |a| {
        let name = arg_str(&a, 0, "env_get")?;
        Ok(std::env::var(name).map(Value::String).unwrap_or(Value::Nil))
    }),
    ("env_args", |_| Ok(Value::Array(std::env::args().map(Value::String).collect()))),
    ("process_exec", |a| {
        let cmd = arg_str(&a, 0, "process_exec")?;
        let out = std::process::Command::new("sh").arg("-c").arg(cmd).output()
            .map_err(|e| RuntimeError::new(format!("failed to run '{}': {}", cmd, e)))?;
        Ok(Value::String(String::from_utf8_lossy(&out.stdout).into_owned()))
    }),

    // Network
    ("http_get", |a| {
        let url = arg_str(&a, 0, "http_get")?;
        let resp = reqwest::blocking::get(url).map_err(|e| RuntimeError::new(format!("http_get('{}') failed: {}", url, e)))?;
        resp.text().map(Value::String).map_err(|e| RuntimeError::new(format!("http_get('{}') failed to read body: {}", url, e)))
    }),
];

/// Runtime error, with the span of the expression or statement that failed
/// when the interpreter knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<Span>,
}

impl RuntimeError {
    pub fn new(m: impl Into<String>) -> Self { Self { message: m.into(), span: None } }

    pub fn at(m: impl Into<String>, span: Span) -> Self { Self { message: m.into(), span: Some(span) } }

    /// Attach a span if the error does not have one yet.
    pub fn with_span(mut self, span: Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span);
        }
        self
    }

    pub fn display(&self) -> String {
        match self.span {
            Some(span) => format!("runtime error[{}:{}]: {}", span.line, span.column, self.message),
            None => format!("runtime error: {}", self.message),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for RuntimeError {}

/// Non-error control flow carried through the Result channel.
const BREAK_SIGNAL: &str = "__break__";
const CONTINUE_SIGNAL: &str = "__continue__";

/// Deepest chain of REOX function calls the interpreter will execute.
pub const MAX_CALL_DEPTH: usize = 512;

/// Deepest expression tree the interpreter evaluates recursively.
pub const MAX_EXPR_DEPTH: usize = 2048;

/// Stack reserved for evaluation. Each REOX call uses several interpreter
/// frames, which are large in debug builds, so evaluation runs on its own thread.
const EVAL_STACK_BYTES: usize = 256 * 1024 * 1024;

pub struct Interpreter {
    env: Environment,
    structs: HashMap<String, StructDecl>,
    functions: HashMap<String, FnDecl>,
    call_depth: usize,
    expr_depth: usize,
    /// Set by `return`; unwinds enclosing blocks and loops until the call takes it.
    returning: Option<Value>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            structs: HashMap::new(),
            functions: HashMap::new(),
            call_depth: 0,
            expr_depth: 0,
            returning: None,
        }
    }

    /// Evaluate a program: register declarations, then run `main` if present.
    /// Evaluation runs on a dedicated large-stack thread so deep (but bounded)
    /// recursion in user code cannot overflow the caller's stack.
    pub fn eval(&mut self, ast: &Ast) -> Result<Value, RuntimeError> {
        let outcome = std::thread::scope(|scope| {
            let worker = std::thread::Builder::new()
                .name("reox-eval".to_string())
                .stack_size(EVAL_STACK_BYTES)
                .spawn_scoped(scope, || self.eval_on_current_thread(ast));
            match worker {
                Ok(handle) => handle.join().unwrap_or_else(|_| {
                    Err(RuntimeError::new("interpreter thread terminated unexpectedly"))
                }),
                Err(e) => Err(RuntimeError::new(format!("failed to start interpreter thread: {}", e))),
            }
        });
        outcome
    }

    /// Evaluate on the calling thread. Prefer `eval`; this exists for callers
    /// that manage their own stack.
    pub fn eval_on_current_thread(&mut self, ast: &Ast) -> Result<Value, RuntimeError> {
        for d in &ast.declarations {
            match d {
                Decl::Struct(s) => { self.structs.insert(s.name.clone(), s.clone()); },
                Decl::Function(f) => { self.functions.insert(f.name.clone(), f.clone()); },
                Decl::Variant(v) => {
                    for case in &v.cases {
                        let type_name = v.name.clone();
                        let case_name = case.name.clone();
                        // Register all variant cases as constant values
                        // Payload variants are constructed via pattern syntax: VariantName.CaseName(args)
                        self.env.define(&format!("{}_{}", v.name, case.name),
                            Value::Variant { type_name, case_name, payload: vec![] });
                    }
                },
                Decl::Const(c) => {
                    let val = self.expr(&c.value).map_err(|e| e.with_span(c.span))?;
                    self.env.define(&c.name, val);
                },
                Decl::Layer(l) => {
                    for method in &l.methods {
                        let mangled = format!("{}_{}", l.name, method.name);
                        self.functions.insert(mangled, method.clone());
                    }
                },
                Decl::Panel(p) => {
                    for method in &p.methods {
                        let mangled = format!("{}_{}", p.name, method.name);
                        self.functions.insert(mangled, method.clone());
                    }
                },
                Decl::Extension(ext) => {
                    for method in &ext.methods {
                        let mangled = format!("{}_{}", ext.target, method.name);
                        self.functions.insert(mangled, method.clone());
                    }
                },
                Decl::GlobalVar(g) => {
                    let val = g.init.as_ref().map(|e| self.expr(e)).transpose()
                        .map_err(|e| e.with_span(g.span))?
                        .unwrap_or(Value::Nil);
                    self.env.define(&g.name, val);
                },
                Decl::Import(_) | Decl::Extern(_) | Decl::Protocol(_) | Decl::Typealias(_) => {}
            }
        }
        match self.functions.get("main").cloned() {
            Some(f) => self.call(&f, vec![]).map_err(Self::leaked_control_flow),
            None => Ok(Value::Nil),
        }
    }

    /// A break/continue that escaped every loop is a real error.
    fn leaked_control_flow(e: RuntimeError) -> RuntimeError {
        match e.message.as_str() {
            BREAK_SIGNAL => RuntimeError { message: "'break' outside of a loop".to_string(), span: e.span },
            CONTINUE_SIGNAL => RuntimeError { message: "'continue' outside of a loop".to_string(), span: e.span },
            _ => e,
        }
    }

    fn enter_call(&mut self, span: Option<Span>) -> Result<(), RuntimeError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            let e = RuntimeError::new(format!("call stack exceeded {} nested calls", MAX_CALL_DEPTH));
            return Err(match span { Some(s) => e.with_span(s), None => e });
        }
        self.call_depth += 1;
        Ok(())
    }

    fn exit_call(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
    }

    fn call(&mut self, f: &FnDecl, a: Vec<Value>) -> Result<Value, RuntimeError> {
        self.enter_call(Some(f.span))?;
        self.env.push();
        for (i, p) in f.params.iter().enumerate() { self.env.define(&p.name, a.get(i).cloned().unwrap_or(Value::Nil)); }
        let r = self.block(&f.body);
        self.env.pop();
        self.exit_call();
        self.take_return(r)
    }

    /// Resolve a function body's result: an explicit `return` value wins over
    /// the value of the last statement.
    fn take_return(&mut self, body_result: Result<Value, RuntimeError>) -> Result<Value, RuntimeError> {
        let returned = self.returning.take();
        match (body_result, returned) {
            (Ok(_), Some(v)) => Ok(v),
            (other, _) => other,
        }
    }

    fn call_closure(&mut self, params: &[Param], body: &Block, env: Vec<HashMap<String, Value>>, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        self.enter_call(Some(span))?;
        let saved = std::mem::replace(&mut self.env.scopes, env);
        self.env.push();
        for (i, p) in params.iter().enumerate() {
            self.env.define(&p.name, args.get(i).cloned().unwrap_or(Value::Nil));
        }
        let result = self.block(body);
        self.env.scopes = saved;
        self.exit_call();
        self.take_return(result)
    }

    fn block(&mut self, b: &Block) -> Result<Value, RuntimeError> {
        let mut r = Value::Nil;
        for s in &b.statements {
            r = self.stmt(s)?;
            if self.returning.is_some() {
                return Ok(r);
            }
        }
        Ok(r)
    }

    fn stmt(&mut self, s: &Stmt) -> Result<Value, RuntimeError> {
        self.stmt_inner(s).map_err(|e| e.with_span(s.span()))
    }

    fn stmt_inner(&mut self, s: &Stmt) -> Result<Value, RuntimeError> {
        match s {
            Stmt::Let(l) => {
                let v = l.init.as_ref().map(|e| self.expr(e)).transpose()?.unwrap_or(Value::Nil);
                self.env.define(&l.name, v);
                Ok(Value::Nil)
            },
            Stmt::Expr(e) => self.expr(e),
            Stmt::Return(r) => {
                let v = r.value.as_ref().map(|e| self.expr(e)).transpose()?.unwrap_or(Value::Nil);
                self.returning = Some(v.clone());
                Ok(v)
            },
            Stmt::If(i) => {
                if self.expr(&i.condition)?.is_truthy() {
                    self.block(&i.then_block)
                } else {
                    i.else_block.as_ref().map(|b| self.block(b)).transpose().map(|v| v.unwrap_or(Value::Nil))
                }
            },
            Stmt::While(w) => {
                while self.expr(&w.condition)?.is_truthy() {
                    match self.block(&w.body) {
                        Err(e) if e.message == BREAK_SIGNAL => break,
                        Err(e) if e.message == CONTINUE_SIGNAL => continue,
                        Err(e) => return Err(e),
                        Ok(_) if self.returning.is_some() => break,
                        Ok(_) => {}
                    }
                }
                Ok(Value::Nil)
            },
            Stmt::For(f) => {
                let items = match self.expr(&f.iterable)? {
                    Value::Array(a) => a,
                    other => return Err(RuntimeError::at(
                        format!("for loop requires an array, found {}", other.type_name()),
                        f.iterable.span(),
                    )),
                };
                for i in items {
                    self.env.push();
                    self.env.define(&f.var, i);
                    let outcome = self.block(&f.body);
                    self.env.pop();
                    match outcome {
                        Err(e) if e.message == BREAK_SIGNAL => break,
                        Err(e) if e.message == CONTINUE_SIGNAL => continue,
                        Err(e) => return Err(e),
                        Ok(_) if self.returning.is_some() => break,
                        Ok(_) => {}
                    }
                }
                Ok(Value::Nil)
            },
            Stmt::Block(b) => self.block(b),
            Stmt::Break(_) => Err(RuntimeError::new(BREAK_SIGNAL)),
            Stmt::Continue(_) => Err(RuntimeError::new(CONTINUE_SIGNAL)),
            // Swift-style guard statement
            Stmt::Guard(g) => {
                if !self.expr(&g.condition)?.is_truthy() {
                    self.block(&g.else_block)?;
                }
                Ok(Value::Nil)
            },
            // Defer - executed immediately in the interpreter (full scope-exit
            // semantics belong to the C backend).
            Stmt::Defer(d) => {
                self.block(&d.body)?;
                Ok(Value::Nil)
            },
            // Try-catch exception handling. Control-flow signals are not catchable.
            Stmt::TryCatch(tc) => {
                match self.block(&tc.try_block) {
                    Ok(v) => Ok(v),
                    Err(e) if e.message == BREAK_SIGNAL || e.message == CONTINUE_SIGNAL => Err(e),
                    Err(e) => {
                        self.env.push();
                        if let Some(var) = &tc.catch_var {
                            self.env.define(var, Value::String(e.message));
                        }
                        let result = self.block(&tc.catch_block);
                        self.env.pop();
                        result
                    }
                }
            },
            // Throw creates a runtime error
            Stmt::Throw(t) => {
                let msg = self.expr(&t.value)?;
                Err(RuntimeError::at(msg.to_string(), t.span))
            },
        }
    }

    fn expr(&mut self, e: &Expr) -> Result<Value, RuntimeError> {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            return Err(RuntimeError::at(
                format!("expression nested deeper than {} levels", MAX_EXPR_DEPTH),
                e.span(),
            ));
        }
        self.expr_depth += 1;
        let result = self.expr_inner(e).map_err(|err| err.with_span(e.span()));
        self.expr_depth -= 1;
        result
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, RuntimeError> {
        args.iter().map(|x| self.expr(x)).collect()
    }

    fn expr_inner(&mut self, e: &Expr) -> Result<Value, RuntimeError> {
        match e {
            Expr::Literal(l) => Ok(match l {
                Literal::Int(i,_) => Value::Int(*i),
                Literal::Float(f,_) => Value::Float(*f),
                Literal::String(s,_) => Value::String(s.clone()),
                Literal::Bool(b,_) => Value::Bool(*b)
            }),
            Expr::Identifier(n, _) => self.env.get(n).ok_or_else(|| RuntimeError::new(format!("undefined variable '{}'", n))),
            Expr::Binary(l, o, r, span) => {
                let lv = self.expr(l)?;
                let rv = self.expr(r)?;
                self.binop(lv, o, rv).map_err(|e| e.with_span(*span))
            },
            Expr::Unary(o, x, _) => {
                let v = self.expr(x)?;
                match o {
                    UnaryOp::Neg => match v {
                        Value::Int(i) => i.checked_neg().map(Value::Int)
                            .ok_or_else(|| RuntimeError::new("integer overflow in negation")),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => Err(RuntimeError::new(format!("cannot negate {}", other.type_name())))
                    },
                    UnaryOp::Not => Ok(Value::Bool(!v.is_truthy())),
                    UnaryOp::BitwiseNot => match v {
                        Value::Int(i) => Ok(Value::Int(!i)),
                        other => Err(RuntimeError::new(format!("bitwise not requires int, found {}", other.type_name())))
                    }
                }
            },
            Expr::Call(c, a, span) => {
                if let Expr::Identifier(n, _) = c.as_ref() {
                    match self.env.get(n) {
                        Some(Value::NativeAction(f)) => {
                            let vs = self.eval_args(a)?;
                            return f(vs);
                        }
                        Some(Value::Closure { params, body, env }) => {
                            let vs = self.eval_args(a)?;
                            return self.call_closure(&params, &body, env, vs, *span);
                        }
                        _ => {}
                    }
                    if let Some(f) = self.functions.get(n).cloned() {
                        let vs = self.eval_args(a)?;
                        return self.call(&f, vs);
                    }
                }
                // Calling the result of an arbitrary expression
                let callee = self.expr(c)?;
                match callee {
                    Value::NativeAction(f) => {
                        let vs = self.eval_args(a)?;
                        f(vs)
                    }
                    Value::Closure { params, body, env } => {
                        let vs = self.eval_args(a)?;
                        self.call_closure(&params, &body, env, vs, *span)
                    }
                    other => Err(RuntimeError::new(format!("{} is not callable", other.type_name())))
                }
            },
            Expr::Member(o, f, _) => {
                let ov = self.expr(o)?;
                match ov {
                    Value::Struct { fields, .. } => fields.get(f).cloned()
                        .ok_or_else(|| RuntimeError::new(format!("undefined field '{}'", f))),
                    other => Err(RuntimeError::new(format!("member access '.{}' on {}", f, other.type_name()))),
                }
            },
            Expr::Index(a, i, _) => {
                let av = self.expr(a)?;
                let iv = self.expr(i)?;
                match (&av, &iv) {
                    (Value::Array(arr), Value::Int(idx)) => {
                        usize::try_from(*idx).ok().and_then(|i| arr.get(i)).cloned()
                            .ok_or_else(|| RuntimeError::new(format!("index {} out of bounds (length {})", idx, arr.len())))
                    },
                    (Value::Map(m), Value::String(k)) => {
                        Ok(m.get(k).cloned().unwrap_or(Value::Nil))
                    },
                    (base, index) => Err(RuntimeError::new(format!(
                        "cannot index {} with {}", base.type_name(), index.type_name()
                    )))
                }
            },
            Expr::Assign(t, v, _) => {
                let val = self.expr(v)?;
                match t.as_ref() {
                    Expr::Identifier(n, _) => {
                        if self.env.set(n, val.clone()) {
                            Ok(val)
                        } else {
                            Err(RuntimeError::new(format!("assignment to undefined variable '{}'", n)))
                        }
                    }
                    _ => Err(RuntimeError::new("invalid assignment target")),
                }
            },
            Expr::ArrayLit(es, _) => Ok(Value::Array(self.eval_args(es)?)),
            Expr::StructLit(n, fs, _) => {
                let mut m = HashMap::new();
                for (k,v) in fs { m.insert(k.clone(), self.expr(v)?); }
                Ok(Value::Struct{name:n.clone(),fields:m})
            },
            Expr::Match(x, arms, _) => {
                let v = self.expr(x)?;
                for arm in arms {
                    if self.pat(&arm.pattern, &v) {
                        return self.expr(&arm.body);
                    }
                }
                Ok(Value::Nil)
            },
            // Compound assignments: +=, -=, *=, /=, %=
            Expr::CompoundAssign(target, op, value, _) => {
                let n = match target.as_ref() {
                    Expr::Identifier(n, _) => n,
                    _ => return Err(RuntimeError::new("invalid compound assignment target")),
                };
                let current = self.env.get(n).ok_or_else(|| RuntimeError::new(format!("undefined variable '{}'", n)))?;
                let rhs = self.expr(value)?;
                let bin = match op {
                    CompoundOp::AddEq => BinOp::Add,
                    CompoundOp::SubEq => BinOp::Sub,
                    CompoundOp::MulEq => BinOp::Mul,
                    CompoundOp::DivEq => BinOp::Div,
                    CompoundOp::ModEq => BinOp::Mod,
                };
                let result = self.binop(current, &bin, rhs)?;
                self.env.set(n, result.clone());
                Ok(result)
            },
            Expr::PreIncrement(target, _) => self.step_variable(target, 1, true),
            Expr::PreDecrement(target, _) => self.step_variable(target, -1, true),
            Expr::PostIncrement(target, _) => self.step_variable(target, 1, false),
            Expr::PostDecrement(target, _) => self.step_variable(target, -1, false),
            Expr::Nil(_) => Ok(Value::Nil),
            // Null coalescing: a ?? b
            Expr::NullCoalesce(left, right, _) => {
                let l = self.expr(left)?;
                if matches!(l, Value::Nil) { self.expr(right) } else { Ok(l) }
            },
            // Optional chaining: obj?.member (returns nil if obj is nil)
            Expr::OptionalChain(obj, member, _) => {
                let ov = self.expr(obj)?;
                match ov {
                    Value::Nil => Ok(Value::Nil),
                    Value::Struct { fields, .. } => {
                        Ok(fields.get(member).cloned().unwrap_or(Value::Nil))
                    },
                    other => Err(RuntimeError::new(format!("optional chain '?.{}' on {}", member, other.type_name())))
                }
            },
            // Trailing closure: func(args) { block } (closure body is not executed in the interpreter)
            Expr::TrailingClosure(call_expr, _closure_block, _) => {
                self.expr(call_expr)
            },
            // Await: await expr (simplified, just evaluates the expr)
            Expr::Await(inner, _) => self.expr(inner),
            // Range expression: start..end generates array [start, start+1, ..., end]
            Expr::Range(start, end, _) => {
                let s = self.expr(start)?;
                let e = self.expr(end)?;
                match (s, e) {
                    (Value::Int(from), Value::Int(to)) => Ok(Value::Array(range_values(from, to)?)),
                    (from, to) => Err(RuntimeError::new(format!(
                        "range requires int bounds, found {} and {}", from.type_name(), to.type_name()
                    )))
                }
            },
            Expr::Action(params, _return_type, body, _) => {
                // Capture current environment for closure
                let captured_env = self.env.scopes.clone();
                Ok(Value::Closure {
                    params: params.clone(),
                    body: *body.clone(),
                    env: captured_env,
                })
            },
        }
    }

    /// Shared implementation of ++x, --x, x++ and x--.
    fn step_variable(&mut self, target: &Expr, delta: i64, prefix: bool) -> Result<Value, RuntimeError> {
        let verb = if delta > 0 { "increment" } else { "decrement" };
        let n = match target {
            Expr::Identifier(n, _) => n,
            _ => return Err(RuntimeError::new(format!("invalid {} target", verb))),
        };
        let current = self.env.get(n).ok_or_else(|| RuntimeError::new(format!("undefined variable '{}'", n)))?;
        let old = match current {
            Value::Int(i) => i,
            other => return Err(RuntimeError::new(format!("{} requires int, found {}", verb, other.type_name()))),
        };
        let new = old.checked_add(delta).ok_or_else(|| RuntimeError::new(format!("integer overflow in {}", verb)))?;
        self.env.set(n, Value::Int(new));
        Ok(Value::Int(if prefix { new } else { old }))
    }

    fn pat(&self, p: &Pattern, v: &Value) -> bool {
        match p {
            Pattern::Wildcard | Pattern::Identifier(_) => true,
            Pattern::Literal(l) => match (l, v) {
                (Literal::Int(a,_), Value::Int(b)) => *a == *b,
                (Literal::Bool(a,_), Value::Bool(b)) => *a == *b,
                (Literal::String(a,_), Value::String(b)) => a == b,
                (Literal::Float(a,_), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
                _ => false,
            },
        }
    }

    fn binop(&self, l: Value, o: &BinOp, r: Value) -> Result<Value, RuntimeError> {
        let type_error = |op: &str, l: &Value, r: &Value| {
            RuntimeError::new(format!("operator '{}' is not defined for {} and {}", op, l.type_name(), r.type_name()))
        };
        let overflow = |op: &str| RuntimeError::new(format!("integer overflow in '{}'", op));

        Ok(match o {
            BinOp::Add => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_add(b).ok_or_else(|| overflow("+"))?),
                (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 + b),
                (Value::Float(a), Value::Int(b)) => Value::Float(a + b as f64),
                (Value::String(a), Value::String(b)) => Value::String(a + &b),
                (l, r) => return Err(type_error("+", &l, &r))
            },
            BinOp::Sub => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_sub(b).ok_or_else(|| overflow("-"))?),
                (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 - b),
                (Value::Float(a), Value::Int(b)) => Value::Float(a - b as f64),
                (l, r) => return Err(type_error("-", &l, &r))
            },
            BinOp::Mul => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_mul(b).ok_or_else(|| overflow("*"))?),
                (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 * b),
                (Value::Float(a), Value::Int(b)) => Value::Float(a * b as f64),
                (l, r) => return Err(type_error("*", &l, &r))
            },
            BinOp::Div => match (l, r) {
                (Value::Int(_), Value::Int(0)) => return Err(RuntimeError::new("division by zero")),
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_div(b).ok_or_else(|| overflow("/"))?),
                (Value::Float(a), Value::Float(b)) if b != 0.0 => Value::Float(a / b),
                (Value::Int(a), Value::Float(b)) if b != 0.0 => Value::Float(a as f64 / b),
                (Value::Float(a), Value::Int(b)) if b != 0 => Value::Float(a / b as f64),
                (Value::Float(_), _) | (_, Value::Float(_)) => return Err(RuntimeError::new("division by zero")),
                (l, r) => return Err(type_error("/", &l, &r))
            },
            BinOp::Mod => match (l, r) {
                (Value::Int(_), Value::Int(0)) => return Err(RuntimeError::new("modulo by zero")),
                (Value::Int(a), Value::Int(b)) => Value::Int(a.checked_rem(b).ok_or_else(|| overflow("%"))?),
                (Value::Float(a), Value::Float(b)) if b != 0.0 => Value::Float(a % b),
                (Value::Int(a), Value::Float(b)) if b != 0.0 => Value::Float((a as f64) % b),
                (Value::Float(a), Value::Int(b)) if b != 0 => Value::Float(a % (b as f64)),
                (Value::Float(_), _) | (_, Value::Float(_)) => return Err(RuntimeError::new("modulo by zero")),
                (l, r) => return Err(type_error("%", &l, &r))
            },
            BinOp::Eq => Value::Bool(values_equal(&l, &r)),
            BinOp::Ne => Value::Bool(!values_equal(&l, &r)),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                let ordering = match (&l, &r) {
                    (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                    (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
                    (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
                    (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
                    (Value::String(a), Value::String(b)) => a.partial_cmp(b),
                    _ => return Err(type_error(comparison_symbol(o), &l, &r)),
                };
                let ord = match ordering {
                    Some(ord) => ord,
                    // NaN compares false against everything, like IEEE 754.
                    None => return Ok(Value::Bool(false)),
                };
                Value::Bool(match o {
                    BinOp::Lt => ord.is_lt(),
                    BinOp::Gt => ord.is_gt(),
                    BinOp::Le => ord.is_le(),
                    _ => ord.is_ge(),
                })
            }
            BinOp::And => Value::Bool(l.is_truthy() && r.is_truthy()),
            BinOp::Or => Value::Bool(l.is_truthy() || r.is_truthy()),
            // Bitwise operators
            BinOp::BitwiseAnd => match (l, r) { (Value::Int(a), Value::Int(b)) => Value::Int(a & b), (l, r) => return Err(type_error("&", &l, &r)) },
            BinOp::BitwiseOr => match (l, r) { (Value::Int(a), Value::Int(b)) => Value::Int(a | b), (l, r) => return Err(type_error("|", &l, &r)) },
            BinOp::BitwiseXor => match (l, r) { (Value::Int(a), Value::Int(b)) => Value::Int(a ^ b), (l, r) => return Err(type_error("^", &l, &r)) },
            BinOp::ShiftLeft => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(shift_amount(b, "<<")?.map_or(0, |n| a.checked_shl(n).unwrap_or(0))),
                (l, r) => return Err(type_error("<<", &l, &r))
            },
            BinOp::ShiftRight => match (l, r) {
                (Value::Int(a), Value::Int(b)) => Value::Int(match shift_amount(b, ">>")? {
                    Some(n) => a.checked_shr(n).unwrap_or(if a < 0 { -1 } else { 0 }),
                    None => if a < 0 { -1 } else { 0 },
                }),
                (l, r) => return Err(type_error(">>", &l, &r))
            },
        })
    }
}

fn comparison_symbol(o: &BinOp) -> &'static str {
    match o {
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        _ => ">=",
    }
}

/// Validate a shift count. Negative counts are an error; counts of 64 or more
/// shift every bit out and are reported as `None`.
fn shift_amount(b: i64, op: &str) -> Result<Option<u32>, RuntimeError> {
    if b < 0 {
        return Err(RuntimeError::new(format!("shift amount for '{}' must not be negative, got {}", op, b)));
    }
    Ok(u32::try_from(b).ok().filter(|n| *n < 64))
}

impl Default for Interpreter { fn default() -> Self { Self::new() } }

pub fn eval(ast: &Ast) -> Result<Value, RuntimeError> { Interpreter::new().eval(ast) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn run(source: &str) -> Result<Value, RuntimeError> {
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        eval(&ast)
    }

    fn run_err(source: &str) -> RuntimeError {
        run(source).unwrap_err()
    }

    #[test]
    fn integer_overflow_is_an_error_not_a_panic() {
        let err = run_err("fn main() -> int { let x = 9223372036854775807; return x + 1; }");
        assert_eq!(err.message, "integer overflow in '+'");
        assert!(err.span.is_some());
        assert_eq!(run_err("fn main() -> int { let x = 9223372036854775807; return x * 2; }").message, "integer overflow in '*'");
        assert_eq!(run_err("fn main() -> int { let x = -9223372036854775807 - 1; return -x; }").message, "integer overflow in negation");
        assert_eq!(run_err("fn main() -> int { let x = -9223372036854775807 - 1; return x / -1; }").message, "integer overflow in '/'");
    }

    #[test]
    fn division_and_modulo_by_zero_are_errors() {
        assert_eq!(run_err("fn main() -> int { let z = 0; return 1 / z; }").message, "division by zero");
        assert_eq!(run_err("fn main() -> int { let z = 0; return 1 % z; }").message, "modulo by zero");
        assert_eq!(run_err("fn main() -> float { let z = 0.0; return 1.0 / z; }").message, "division by zero");
    }

    #[test]
    fn shifts_are_bounded() {
        assert_eq!(run("fn main() -> int { let n = 70; return 1 << n; }").unwrap(), Value::Int(0));
        assert_eq!(run("fn main() -> int { let n = 70; return -8 >> n; }").unwrap(), Value::Int(-1));
        assert_eq!(run("fn main() -> int { let n = 3; return 1 << n; }").unwrap(), Value::Int(8));
        let err = run_err("fn main() -> int { let n = -1; return 1 << n; }");
        assert!(err.message.contains("must not be negative"), "{}", err.message);
    }

    #[test]
    fn runaway_recursion_is_reported() {
        let err = run_err("fn f(n: int) -> int { return f(n + 1); } fn main() -> int { return f(0); }");
        assert_eq!(err.message, format!("call stack exceeded {} nested calls", MAX_CALL_DEPTH));
        assert!(err.span.is_some());
    }

    #[test]
    fn bounded_recursion_works_and_return_unwinds_nested_blocks() {
        let src = "fn f(n: int) -> int { if n == 0 { return 100; } return f(n - 1); } fn main() -> int { return f(400); }";
        assert_eq!(run(src).unwrap(), Value::Int(100));
        let src = "fn main() -> int { let i = 0; while true { i = i + 1; if i == 5 { return i; } } return -1; }";
        assert_eq!(run(src).unwrap(), Value::Int(5));
        let src = "fn main() -> int { for x in [1, 2, 3] { if x == 2 { return x * 10; } } return -1; }";
        assert_eq!(run(src).unwrap(), Value::Int(20));
    }

    #[test]
    fn break_outside_loop_is_an_error() {
        assert_eq!(run_err("fn main() { break; }").message, "'break' outside of a loop");
        assert_eq!(run_err("fn main() { continue; }").message, "'continue' outside of a loop");
    }

    #[test]
    fn for_over_non_array_is_an_error() {
        let err = run_err("fn main() { for x in len([1]) { } }");
        assert_eq!(err.message, "for loop requires an array, found int");
        assert_eq!(err.span.map(|s| s.column), Some(26));
    }

    #[test]
    fn range_too_large_is_an_error() {
        let err = run_err("fn main() { let r = range(0, 100000000); }");
        assert!(err.message.contains("element limit"), "{}", err.message);
        assert_eq!(run("fn main() -> int { return len(range(0, 5)); }").unwrap(), Value::Int(5));
        assert_eq!(run("fn main() -> int { return len(range(5, 0)); }").unwrap(), Value::Int(0));
        // The range expression node is evaluated through the same bounded helper.
        let err = range_values(0, MAX_RANGE_LEN).unwrap_err();
        assert!(err.message.contains("element limit"));
        assert_eq!(range_values(i64::MIN, i64::MAX).unwrap_err().message, "range is too large to represent");
        assert_eq!(range_values(1, 3).unwrap().len(), 3);
    }

    #[test]
    fn native_argument_errors_are_diagnostics() {
        let err = run_err("fn main() { let m = map_get(map_new()); }");
        assert_eq!(err.message, "map_get() expects at least 2 arguments, found 1");
        let err = run_err("fn main() { let p = push(1, 2); }");
        assert_eq!(err.message, "push() expects an array as argument 1, found int");
        let err = run_err("fn main() { let x = array_slice([1, 2, 3], 2, 1); }");
        assert_eq!(err.message, "slice start 2 is after slice end 1");
        let err = run_err("fn main() { let x = array_slice([1, 2, 3], -1, 1); }");
        assert!(err.message.contains("must not be negative"));
        let err = run_err("fn main() { let c = hex(\"aé€\"); }");
        assert!(err.message.contains("6 hex digits"), "{}", err.message);
        let err = run_err("fn main() { let c = rgb(300, 0, 0); }");
        assert!(err.message.contains("0..=255"), "{}", err.message);
        let err = run_err("fn main() { let r = random_int(5, 1); }");
        assert!(err.message.contains("lower bound 5 is above upper bound 1"));
        let err = run_err("fn main() { time_sleep(-5); }");
        assert!(err.message.contains("must not be negative"));
        let err = run_err("fn main() { let p = pow(2, 70); }");
        assert!(err.message.contains("integer overflow in pow"));
        let err = run_err("fn main() { let p = pow(2, -1); }");
        assert!(err.message.contains("exponent must be"));
        let err = run_err("fn main() { let s = array_set([1], 5, 0); }");
        assert!(err.message.contains("out of bounds"));
        let err = run_err("fn main() { let c = clamp(1, 10, 0); }");
        assert!(err.message.contains("lower bound 10 is above upper bound 0"));
    }

    #[test]
    fn natives_still_work_with_valid_arguments() {
        assert_eq!(run("fn main() -> int { return len(push([1], 2)); }").unwrap(), Value::Int(2));
        assert_eq!(run("fn main() -> int { return array_get([7, 8], 1); }").unwrap(), Value::Int(8));
        assert_eq!(run("fn main() -> int { return len(array_slice([1, 2, 3, 4], 1, 3)); }").unwrap(), Value::Int(2));
        assert_eq!(run("fn main() -> int { return pow(2, 10); }").unwrap(), Value::Int(1024));
        assert_eq!(run("fn main() -> int { return clamp(15, 0, 10); }").unwrap(), Value::Int(10));
        assert_eq!(run("fn main() -> string { return str_join([\"a\", \"b\"], \"-\"); }").unwrap(), Value::String("a-b".into()));
        // `int`, `float` and `bool` are type keywords, so their natives are only reachable directly.
        let int_native = Environment::new().get("int");
        match int_native {
            Some(Value::NativeAction(f)) => {
                assert_eq!(f(vec![Value::String("42".into())]).unwrap(), Value::Int(42));
                assert_eq!(f(vec![Value::String("nope".into())]).unwrap(), Value::Nil);
                assert!(f(vec![Value::Nil]).is_err());
            }
            other => panic!("expected native, got {:?}", other),
        }
        match run("fn main() { return hex(\"#ff8000\"); }").unwrap() {
            Value::Color { r, g, b, a } => assert_eq!((r, g, b, a), (255, 128, 0, 255)),
            other => panic!("expected color, got {:?}", other),
        }
        match run("fn main() { return hsl(120.0, 100.0, 50.0); }").unwrap() {
            Value::Color { r, g, b, .. } => assert_eq!((r, g, b), (0, 255, 0)),
            other => panic!("expected color, got {:?}", other),
        }
        assert_eq!(run("fn main() -> int { return len(\"héllo\"); }").unwrap(), Value::Int(5));
    }

    #[test]
    fn file_errors_carry_the_os_message() {
        let err = run_err("fn main() { let s = file_read(\"/nonexistent/reox/file.txt\"); }");
        assert!(err.message.starts_with("failed to read '/nonexistent/reox/file.txt'"), "{}", err.message);
        assert_eq!(run("fn main() -> bool { return file_exists(\"/nonexistent/reox\"); }").unwrap(), Value::Bool(false));
    }

    #[test]
    fn const_initializer_errors_propagate() {
        let err = run_err("const X: int = undefined_thing + 1; fn main() { }");
        assert_eq!(err.message, "undefined variable 'undefined_thing'");
        assert_eq!(err.span.map(|s| s.line), Some(1));
    }

    #[test]
    fn runtime_errors_carry_spans() {
        let err = run_err("fn main() {\n  let a = 1;\n  let b = a + \"s\";\n}");
        assert_eq!(err.message, "operator '+' is not defined for int and string");
        assert_eq!(err.span.map(|s| s.line), Some(3));
        assert!(err.display().starts_with("runtime error[3:"));
        let err = run_err("fn main() { let v = nope; }");
        assert_eq!(err.message, "undefined variable 'nope'");
        let err = run_err("fn main() { let x = 1; let y = x.field; }");
        assert_eq!(err.message, "member access '.field' on int");
        let err = run_err("fn main() { let x = 1; x(); }");
        assert_eq!(err.message, "int is not callable");
    }

    #[test]
    fn index_errors_are_reported() {
        let err = run_err("fn main() -> int { let a = [1, 2]; return a[5]; }");
        assert_eq!(err.message, "index 5 out of bounds (length 2)");
        let err = run_err("fn main() -> int { let a = [1, 2]; return a[-1]; }");
        assert_eq!(err.message, "index -1 out of bounds (length 2)");
        let err = run_err("fn main() -> int { let a = 3; return a[0]; }");
        assert_eq!(err.message, "cannot index int with int");
    }

    #[test]
    fn increments_check_overflow() {
        let err = run_err("fn main() -> int { let x = 9223372036854775807; x++; return x; }");
        assert_eq!(err.message, "integer overflow in increment");
        assert_eq!(run("fn main() -> int { let x = 1; x++; ++x; x--; return x; }").unwrap(), Value::Int(2));
        assert_eq!(run("fn main() -> int { let x = 1; return x++; }").unwrap(), Value::Int(1));
    }

    #[test]
    fn comparisons_and_equality() {
        assert_eq!(run("fn main() -> bool { return 1 < 2.5; }").unwrap(), Value::Bool(true));
        assert_eq!(run("fn main() -> bool { return \"a\" < \"b\"; }").unwrap(), Value::Bool(true));
        assert_eq!(run("fn main() -> bool { return 2 >= 2; }").unwrap(), Value::Bool(true));
        let err = run_err("fn main() -> bool { return 1 < \"a\"; }");
        assert_eq!(err.message, "operator '<' is not defined for int and string");
    }

    #[test]
    fn deep_expression_tree_is_reported_not_overflowed() {
        let span = Span::default();
        let mut expr = Expr::Literal(Literal::Int(1, span));
        for _ in 0..(MAX_EXPR_DEPTH + 10) {
            expr = Expr::Unary(UnaryOp::Neg, Box::new(expr), span);
        }
        let ast = Ast {
            declarations: vec![Decl::Function(FnDecl {
                name: "main".into(),
                params: vec![],
                return_type: None,
                body: Block { statements: vec![Stmt::Return(ReturnStmt { value: Some(expr), span })], span },
                is_async: false,
                is_pub: false,
                is_static: false,
                span,
            })],
        };
        let err = eval(&ast).unwrap_err();
        assert!(err.message.contains("nested deeper than"), "{}", err.message);
    }

    #[test]
    fn try_catch_catches_runtime_errors_but_not_control_flow() {
        let src = "fn main() -> string { let z = 0; try { let x = 1 / z; } catch e { return e; } return \"no\"; }";
        assert_eq!(run(src).unwrap(), Value::String("division by zero".into()));
        let src = "fn main() -> int { let i = 0; while true { try { i = i + 1; if i == 3 { break; } } catch e { return -1; } } return i; }";
        assert_eq!(run(src).unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_on_current_thread_matches_eval() {
        let tokens = tokenize("fn main() -> int { return 2 + 2; }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(Interpreter::new().eval_on_current_thread(&ast).unwrap(), Value::Int(4));
        assert_eq!(eval(&ast).unwrap(), Value::Int(4));
    }
}
