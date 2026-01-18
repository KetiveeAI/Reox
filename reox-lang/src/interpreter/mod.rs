// REOX Interpreter - Evaluates REOX AST for REPL mode

#![allow(dead_code)]

use crate::parser::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Nil, Bool(bool), Int(i64), Float(f64), String(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Color { r: u8, g: u8, b: u8, a: u8 },
    Struct { name: String, fields: HashMap<String, Value> },
    NativeAction(fn(Vec<Value>) -> Value),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self { Value::Nil => false, Value::Bool(b) => *b, Value::Int(i) => *i != 0, _ => true }
    }
    pub fn type_name(&self) -> &'static str {
        match self { Value::Nil => "nil", Value::Bool(_) => "bool", Value::Int(_) => "int",
                     Value::Float(_) => "float", Value::String(_) => "string", Value::Array(_) => "array",
                     Value::Map(_) => "map", Value::Color {..} => "color",
                     Value::Struct {..} => "struct", Value::NativeAction(_) => "native" }
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment { scopes: Vec<HashMap<String, Value>> }

impl Environment {
    pub fn new() -> Self {
        let mut e = Self { scopes: vec![HashMap::new()] };
        // I/O
        e.define("print", Value::NativeAction(|a| { for x in &a { print!("{} ", x); } println!(); Value::Nil }));
        // Collections
        e.define("len", Value::NativeAction(|a| match a.first() { Some(Value::Array(v)) => Value::Int(v.len() as i64), Some(Value::String(s)) => Value::Int(s.len() as i64), Some(Value::Map(m)) => Value::Int(m.len() as i64), _ => Value::Int(0) }));
        e.define("push", Value::NativeAction(|a| {
            if a.len() >= 2 { if let Value::Array(mut arr) = a[0].clone() { arr.push(a[1].clone()); return Value::Array(arr); } }
            Value::Nil
        }));
        e.define("pop", Value::NativeAction(|a| {
            if let Some(Value::Array(mut arr)) = a.first().cloned() { arr.pop().unwrap_or(Value::Nil) } else { Value::Nil }
        }));
        e.define("map_new", Value::NativeAction(|_| Value::Map(HashMap::new())));
        e.define("map_set", Value::NativeAction(|a| {
            if a.len() >= 3 { if let (Value::Map(mut m), Value::String(k)) = (a[0].clone(), a[1].clone()) { m.insert(k, a[2].clone()); return Value::Map(m); } }
            Value::Nil
        }));
        e.define("map_get", Value::NativeAction(|a| {
            if a.len() >= 2 { if let (Value::Map(m), Value::String(k)) = (&a[0], &a[1]) { return m.get(k).cloned().unwrap_or(Value::Nil); } }
            Value::Nil
        }));
        // Color management
        e.define("rgba", Value::NativeAction(|a| {
            let r = if let Some(Value::Int(v)) = a.get(0) { *v as u8 } else { 0 };
            let g = if let Some(Value::Int(v)) = a.get(1) { *v as u8 } else { 0 };
            let b = if let Some(Value::Int(v)) = a.get(2) { *v as u8 } else { 0 };
            let alpha = if let Some(Value::Int(v)) = a.get(3) { *v as u8 } else { 255 };
            Value::Color { r, g, b, a: alpha }
        }));
        e.define("rgb", Value::NativeAction(|a| {
            let r = if let Some(Value::Int(v)) = a.get(0) { *v as u8 } else { 0 };
            let g = if let Some(Value::Int(v)) = a.get(1) { *v as u8 } else { 0 };
            let b = if let Some(Value::Int(v)) = a.get(2) { *v as u8 } else { 0 };
            Value::Color { r, g, b, a: 255 }
        }));
        e.define("hex", Value::NativeAction(|a| {
            if let Some(Value::String(s)) = a.first() {
                let hex = s.trim_start_matches('#');
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                    return Value::Color { r, g, b, a: 255 };
                }
            }
            Value::Color { r: 0, g: 0, b: 0, a: 255 }
        }));
        // AI
        e.define("ai_generate", Value::NativeAction(crate::stdlib::ai::generate));
        
        // ============ System Module ============
        // File I/O
        e.define("file_read", Value::NativeAction(|a| {
            if let Some(Value::String(path)) = a.first() {
                match std::fs::read_to_string(path) {
                    Ok(content) => Value::String(content),
                    Err(_) => Value::String(String::new()),
                }
            } else { Value::String(String::new()) }
        }));
        e.define("file_write", Value::NativeAction(|a| {
            if a.len() >= 2 {
                if let (Value::String(path), Value::String(content)) = (&a[0], &a[1]) {
                    return Value::Bool(std::fs::write(path, content).is_ok());
                }
            }
            Value::Bool(false)
        }));
        e.define("file_exists", Value::NativeAction(|a| {
            if let Some(Value::String(path)) = a.first() {
                Value::Bool(std::path::Path::new(path).exists())
            } else { Value::Bool(false) }
        }));
        e.define("file_delete", Value::NativeAction(|a| {
            if let Some(Value::String(path)) = a.first() {
                Value::Bool(std::fs::remove_file(path).is_ok())
            } else { Value::Bool(false) }
        }));
        e.define("file_size", Value::NativeAction(|a| {
            if let Some(Value::String(path)) = a.first() {
                match std::fs::metadata(path) {
                    Ok(m) => Value::Int(m.len() as i64),
                    Err(_) => Value::Int(-1),
                }
            } else { Value::Int(-1) }
        }));
        e.define("dir_list", Value::NativeAction(|a| {
            if let Some(Value::String(path)) = a.first() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    let files: Vec<Value> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| Value::String(e.file_name().to_string_lossy().into_owned()))
                        .collect();
                    return Value::Array(files);
                }
            }
            Value::Array(vec![])
        }));
        
        // Time
        e.define("time_now", Value::NativeAction(|_| {
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => Value::Int(d.as_secs() as i64),
                Err(_) => Value::Int(0),
            }
        }));
        e.define("time_millis", Value::NativeAction(|_| {
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => Value::Int(d.as_millis() as i64),
                Err(_) => Value::Int(0),
            }
        }));
        e.define("time_sleep", Value::NativeAction(|a| {
            if let Some(Value::Int(ms)) = a.first() {
                std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
            }
            Value::Nil
        }));
        
        // Environment
        e.define("env_get", Value::NativeAction(|a| {
            if let Some(Value::String(name)) = a.first() {
                match std::env::var(name) {
                    Ok(val) => Value::String(val),
                    Err(_) => Value::String(String::new()),
                }
            } else { Value::String(String::new()) }
        }));
        e.define("env_args", Value::NativeAction(|_| {
            let args: Vec<Value> = std::env::args().map(Value::String).collect();
            Value::Array(args)
        }));
        
        // Process
        e.define("process_exec", Value::NativeAction(|a| {
            if let Some(Value::String(cmd)) = a.first() {
                match std::process::Command::new("sh").arg("-c").arg(cmd).output() {
                    Ok(out) => Value::String(String::from_utf8_lossy(&out.stdout).into_owned()),
                    Err(_) => Value::String(String::new()),
                }
            } else { Value::String(String::new()) }
        }));
        
        // Random
        e.define("random_int", Value::NativeAction(|a| {
            if a.len() >= 2 {
                if let (Value::Int(min), Value::Int(max)) = (&a[0], &a[1]) {
                    let range = (max - min + 1) as u64;
                    let random = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64 % range)
                        .unwrap_or(0);
                    return Value::Int(*min + random as i64);
                }
            }
            Value::Int(0)
        }));
        
        // ============ Network Module ============
        // HTTP (uses reqwest which is already a dependency)
        e.define("http_get", Value::NativeAction(|a| {
            if let Some(Value::String(url)) = a.first() {
                match reqwest::blocking::get(url) {
                    Ok(resp) => match resp.text() {
                        Ok(text) => Value::String(text),
                        Err(_) => Value::String(String::new()),
                    },
                    Err(_) => Value::String(String::new()),
                }
            } else { Value::String(String::new()) }
        }));
        
        e
    }
    pub fn push(&mut self) { self.scopes.push(HashMap::new()); }
    pub fn pop(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }
    pub fn define(&mut self, n: &str, v: Value) { self.scopes.last_mut().map(|s| s.insert(n.into(), v)); }
    pub fn get(&self, n: &str) -> Option<Value> { self.scopes.iter().rev().find_map(|s| s.get(n).cloned()) }
    pub fn set(&mut self, n: &str, v: Value) -> bool {
        for s in self.scopes.iter_mut().rev() { if s.contains_key(n) { s.insert(n.into(), v); return true; } }
        false
    }
}
impl Default for Environment { fn default() -> Self { Self::new() } }

#[derive(Debug, Clone)]
pub struct RuntimeError { pub message: String }
impl RuntimeError { pub fn new(m: impl Into<String>) -> Self { Self { message: m.into() } } }

pub struct Interpreter {
    env: Environment,
    structs: HashMap<String, StructDecl>,
    functions: HashMap<String, FnDecl>,
}

impl Interpreter {
    pub fn new() -> Self { Self { env: Environment::new(), structs: HashMap::new(), functions: HashMap::new() } }
    
    pub fn eval(&mut self, ast: &Ast) -> Result<Value, RuntimeError> {
        for d in &ast.declarations {
            match d { Decl::Struct(s) => { self.structs.insert(s.name.clone(), s.clone()); },
                      Decl::Function(f) => { self.functions.insert(f.name.clone(), f.clone()); }, _ => {} }
        }
        if let Some(f) = self.functions.get("main").cloned() { self.call(&f, vec![]) } else { Ok(Value::Nil) }
    }
    
    fn call(&mut self, f: &FnDecl, a: Vec<Value>) -> Result<Value, RuntimeError> {
        self.env.push();
        for (i, p) in f.params.iter().enumerate() { self.env.define(&p.name, a.get(i).cloned().unwrap_or(Value::Nil)); }
        let r = self.block(&f.body);
        self.env.pop();
        r
    }
    
    fn block(&mut self, b: &Block) -> Result<Value, RuntimeError> {
        let mut r = Value::Nil;
        for s in &b.statements { r = self.stmt(s)?; if matches!(s, Stmt::Return(_)) { return Ok(r); } }
        Ok(r)
    }
    
    fn stmt(&mut self, s: &Stmt) -> Result<Value, RuntimeError> {
        match s {
            Stmt::Let(l) => { let v = l.init.as_ref().map(|e| self.expr(e)).transpose()?.unwrap_or(Value::Nil); self.env.define(&l.name, v); Ok(Value::Nil) },
            Stmt::Expr(e) => self.expr(e),
            Stmt::Return(r) => r.value.as_ref().map(|e| self.expr(e)).transpose().map(|v| v.unwrap_or(Value::Nil)),
            Stmt::If(i) => if self.expr(&i.condition)?.is_truthy() { self.block(&i.then_block) } else { i.else_block.as_ref().map(|b| self.block(b)).transpose().map(|v| v.unwrap_or(Value::Nil)) },
            Stmt::While(w) => { while self.expr(&w.condition)?.is_truthy() { self.block(&w.body)?; } Ok(Value::Nil) },
            Stmt::For(f) => { if let Value::Array(a) = self.expr(&f.iterable)? { for i in a { self.env.push(); self.env.define(&f.var, i); self.block(&f.body)?; self.env.pop(); } } Ok(Value::Nil) },
            Stmt::Block(b) => self.block(b),
            _ => Ok(Value::Nil),
        }
    }
    
    fn expr(&mut self, e: &Expr) -> Result<Value, RuntimeError> {
        match e {
            Expr::Literal(l) => Ok(match l { Literal::Int(i,_) => Value::Int(*i), Literal::Float(f,_) => Value::Float(*f), Literal::String(s,_) => Value::String(s.clone()), Literal::Bool(b,_) => Value::Bool(*b) }),
            Expr::Identifier(n, _) => self.env.get(n).ok_or_else(|| RuntimeError::new(format!("undefined: {}", n))),
            Expr::Binary(l, o, r, _) => { let lv = self.expr(l)?; let rv = self.expr(r)?; self.binop(lv, o, rv) },
            Expr::Unary(o, x, _) => { let v = self.expr(x)?; match o { UnaryOp::Neg => match v { Value::Int(i) => Ok(Value::Int(-i)), _ => Err(RuntimeError::new("-")) }, UnaryOp::Not => Ok(Value::Bool(!v.is_truthy())), UnaryOp::BitwiseNot => match v { Value::Int(i) => Ok(Value::Int(!i)), _ => Err(RuntimeError::new("~")) } } },
            Expr::Call(c, a, _) => {
                if let Expr::Identifier(n, _) = c.as_ref() {
                    let vs: Vec<Value> = a.iter().map(|x| self.expr(x)).collect::<Result<_,_>>()?;
                    if let Some(Value::NativeAction(f)) = self.env.get(n) { return Ok(f(vs)); }
                    if let Some(f) = self.functions.get(n).cloned() { return self.call(&f, vs); }
                }
                Err(RuntimeError::new("call"))
            },
            Expr::Member(o, f, _) => { let ov = self.expr(o)?; if let Value::Struct{fields,..} = ov { fields.get(f).cloned().ok_or_else(|| RuntimeError::new("field")) } else { Err(RuntimeError::new("member")) } },
            Expr::Index(a, i, _) => { let av = self.expr(a)?; let iv = self.expr(i)?; if let (Value::Array(arr), Value::Int(idx)) = (av, iv) { arr.get(idx as usize).cloned().ok_or_else(|| RuntimeError::new("bounds")) } else { Err(RuntimeError::new("index")) } },
            Expr::Assign(t, v, _) => { let val = self.expr(v)?; if let Expr::Identifier(n, _) = t.as_ref() { if self.env.set(n, val.clone()) { Ok(val) } else { Err(RuntimeError::new("undef")) } } else { Err(RuntimeError::new("target")) } },
            Expr::ArrayLit(es, _) => Ok(Value::Array(es.iter().map(|x| self.expr(x)).collect::<Result<_,_>>()?)),
            Expr::StructLit(n, fs, _) => { let mut m = HashMap::new(); for (k,v) in fs { m.insert(k.clone(), self.expr(v)?); } Ok(Value::Struct{name:n.clone(),fields:m}) },
            Expr::Match(x, arms, _) => { let v = self.expr(x)?; for arm in arms { if self.pat(&arm.pattern, &v) { return self.expr(&arm.body); } } Ok(Value::Nil) },
            // New expression types - fallthrough to Nil for now
            Expr::Nil(_) => Ok(Value::Nil),
            Expr::NullCoalesce(left, right, _) => {
                let l = self.expr(left)?;
                if matches!(l, Value::Nil) { self.expr(right) } else { Ok(l) }
            },
            _ => Ok(Value::Nil), // Other new expressions handled as Nil
        }
    }
    
    fn pat(&self, p: &Pattern, v: &Value) -> bool {
        match p { Pattern::Wildcard | Pattern::Identifier(_) => true, Pattern::Literal(l) => match (l,v) { (Literal::Int(a,_), Value::Int(b)) => *a==*b, (Literal::Bool(a,_), Value::Bool(b)) => *a==*b, _ => false } }
    }
    
    fn binop(&self, l: Value, o: &BinOp, r: Value) -> Result<Value, RuntimeError> {
        Ok(match o {
            BinOp::Add => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a+b), (Value::String(a),Value::String(b)) => Value::String(a+&b), _ => return Err(RuntimeError::new("+")) },
            BinOp::Sub => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a-b), _ => return Err(RuntimeError::new("-")) },
            BinOp::Mul => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a*b), _ => return Err(RuntimeError::new("*")) },
            BinOp::Div => match (l,r) { (Value::Int(a),Value::Int(b)) if b!=0 => Value::Int(a/b), _ => return Err(RuntimeError::new("/")) },
            BinOp::Mod => match (l,r) { (Value::Int(a),Value::Int(b)) if b!=0 => Value::Int(a%b), _ => return Err(RuntimeError::new("%")) },
            BinOp::Eq => Value::Bool(self.eq(&l,&r)), BinOp::Ne => Value::Bool(!self.eq(&l,&r)),
            BinOp::Lt => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Bool(a<b), _ => return Err(RuntimeError::new("<")) },
            BinOp::Gt => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Bool(a>b), _ => return Err(RuntimeError::new(">")) },
            BinOp::Le => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Bool(a<=b), _ => return Err(RuntimeError::new("<=")) },
            BinOp::Ge => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Bool(a>=b), _ => return Err(RuntimeError::new(">=")) },
            BinOp::And => Value::Bool(l.is_truthy() && r.is_truthy()), BinOp::Or => Value::Bool(l.is_truthy() || r.is_truthy()),
            // Bitwise operators
            BinOp::BitwiseAnd => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a&b), _ => return Err(RuntimeError::new("&")) },
            BinOp::BitwiseOr => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a|b), _ => return Err(RuntimeError::new("|")) },
            BinOp::BitwiseXor => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a^b), _ => return Err(RuntimeError::new("^")) },
            BinOp::ShiftLeft => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a<<b), _ => return Err(RuntimeError::new("<<")) },
            BinOp::ShiftRight => match (l,r) { (Value::Int(a),Value::Int(b)) => Value::Int(a>>b), _ => return Err(RuntimeError::new(">>")) },
        })
    }
    
    fn eq(&self, a: &Value, b: &Value) -> bool { match (a,b) { (Value::Nil,Value::Nil) => true, (Value::Bool(a),Value::Bool(b)) => a==b, (Value::Int(a),Value::Int(b)) => a==b, (Value::String(a),Value::String(b)) => a==b, _ => false } }
}

impl Default for Interpreter { fn default() -> Self { Self::new() } }

pub fn eval(ast: &Ast) -> Result<Value, RuntimeError> { Interpreter::new().eval(ast) }
