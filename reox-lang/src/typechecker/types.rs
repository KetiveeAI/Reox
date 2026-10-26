// REOX Compiler - Type System Definitions
// Zero external dependencies

use std::collections::HashMap;

/// Resolved type (after type checking)
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedType {
    Int,
    Float,
    String,
    Bool,
    Void,
    // Explicit integer widths
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    USize,
    Struct(String),
    Array(Box<ResolvedType>),
    Function {
        params: Vec<ResolvedType>,
        ret: Box<ResolvedType>,
    },
    // Container types
    Optional(Box<ResolvedType>),
    Map(Box<ResolvedType>, Box<ResolvedType>),
    Tuple(Vec<ResolvedType>),
    // Special types
    Color,
    Unknown,
    Error,
}

impl ResolvedType {
    /// Convert parser Type to ResolvedType
    pub fn from_parser_type(ty: &crate::parser::Type) -> Self {
        match ty {
            crate::parser::Type::Int => ResolvedType::Int,
            crate::parser::Type::Float => ResolvedType::Float,
            crate::parser::Type::String => ResolvedType::String,
            crate::parser::Type::Bool => ResolvedType::Bool,
            crate::parser::Type::Void => ResolvedType::Void,
            crate::parser::Type::I8 => ResolvedType::I8,
            crate::parser::Type::U8 => ResolvedType::U8,
            crate::parser::Type::I16 => ResolvedType::I16,
            crate::parser::Type::U16 => ResolvedType::U16,
            crate::parser::Type::I32 => ResolvedType::I32,
            crate::parser::Type::U32 => ResolvedType::U32,
            crate::parser::Type::I64 => ResolvedType::I64,
            crate::parser::Type::U64 => ResolvedType::U64,
            crate::parser::Type::USize => ResolvedType::USize,
            crate::parser::Type::Named(name) => ResolvedType::Struct(name.clone()),
            crate::parser::Type::Array(inner) => {
                ResolvedType::Array(Box::new(Self::from_parser_type(inner)))
            }
            crate::parser::Type::Optional(inner) => {
                ResolvedType::Optional(Box::new(Self::from_parser_type(inner)))
            }
            crate::parser::Type::Function(params, ret) => {
                ResolvedType::Function {
                    params: params.iter().map(Self::from_parser_type).collect(),
                    ret: Box::new(Self::from_parser_type(ret)),
                }
            }
            crate::parser::Type::Tuple(elems) => {
                ResolvedType::Tuple(elems.iter().map(Self::from_parser_type).collect())
            }
        }
    }

    /// Convert back to a parser type, for annotations the checker fills in
    /// (for example the type of an untyped `let`). Types that have no source
    /// spelling (maps, colors, unknown, error) yield `None`.
    pub fn to_parser_type(&self) -> Option<crate::parser::Type> {
        use crate::parser::Type as P;
        Some(match self {
            ResolvedType::Int => P::Int,
            ResolvedType::Float => P::Float,
            ResolvedType::String => P::String,
            ResolvedType::Bool => P::Bool,
            ResolvedType::Void => P::Void,
            ResolvedType::I8 => P::I8,
            ResolvedType::U8 => P::U8,
            ResolvedType::I16 => P::I16,
            ResolvedType::U16 => P::U16,
            ResolvedType::I32 => P::I32,
            ResolvedType::U32 => P::U32,
            ResolvedType::I64 => P::I64,
            ResolvedType::U64 => P::U64,
            ResolvedType::USize => P::USize,
            ResolvedType::Struct(name) => P::Named(name.clone()),
            ResolvedType::Array(inner) => P::Array(Box::new(inner.to_parser_type()?)),
            ResolvedType::Optional(inner) => P::Optional(Box::new(inner.to_parser_type()?)),
            ResolvedType::Function { params, ret } => P::Function(
                params.iter().map(|p| p.to_parser_type()).collect::<Option<Vec<_>>>()?,
                Box::new(ret.to_parser_type()?),
            ),
            ResolvedType::Tuple(elems) => {
                P::Tuple(elems.iter().map(|e| e.to_parser_type()).collect::<Option<Vec<_>>>()?)
            }
            ResolvedType::Map(_, _) | ResolvedType::Color | ResolvedType::Unknown | ResolvedType::Error => {
                return None
            }
        })
    }

    /// Check if this is an integer type (including explicit widths)
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            ResolvedType::Int
                | ResolvedType::I8
                | ResolvedType::U8
                | ResolvedType::I16
                | ResolvedType::U16
                | ResolvedType::I32
                | ResolvedType::U32
                | ResolvedType::I64
                | ResolvedType::U64
                | ResolvedType::USize
        )
    }

    /// Check if types are compatible for assignment
    pub fn is_assignable_from(&self, other: &ResolvedType) -> bool {
        if *self == *other {
            return true;
        }
        match (self, other) {
            // Unknown accepts anything (used for builtins like print)
            (ResolvedType::Unknown, _) | (_, ResolvedType::Unknown) => true,
            // Error type never blocks compilation (already reported)
            (ResolvedType::Error, _) | (_, ResolvedType::Error) => true,
            // Float can be assigned from Int (widening)
            (ResolvedType::Float, ResolvedType::Int) => true,
            // Integer width coercion (all integer types are mutually assignable)
            (a, b) if a.is_integer() && b.is_integer() => true,
            // Optional<T> can be assigned from T
            (ResolvedType::Optional(inner), other) => inner.is_assignable_from(other),
            // Array<T> compatibility
            (ResolvedType::Array(a), ResolvedType::Array(b)) => a.is_assignable_from(b),
            // Map compatibility
            (ResolvedType::Map(k1, v1), ResolvedType::Map(k2, v2)) => {
                k1.is_assignable_from(k2) && v1.is_assignable_from(v2)
            }
            // Tuple compatibility (same length and compatible elements)
            (ResolvedType::Tuple(a), ResolvedType::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.is_assignable_from(y))
            }
            _ => false,
        }
    }

    /// Get display name for error messages
    pub fn display_name(&self) -> String {
        match self {
            ResolvedType::Int => "int".to_string(),
            ResolvedType::Float => "float".to_string(),
            ResolvedType::String => "string".to_string(),
            ResolvedType::Bool => "bool".to_string(),
            ResolvedType::Void => "void".to_string(),
            ResolvedType::I8 => "i8".to_string(),
            ResolvedType::U8 => "u8".to_string(),
            ResolvedType::I16 => "i16".to_string(),
            ResolvedType::U16 => "u16".to_string(),
            ResolvedType::I32 => "i32".to_string(),
            ResolvedType::U32 => "u32".to_string(),
            ResolvedType::I64 => "i64".to_string(),
            ResolvedType::U64 => "u64".to_string(),
            ResolvedType::USize => "usize".to_string(),
            ResolvedType::Struct(name) => name.clone(),
            ResolvedType::Array(inner) => format!("[{}]", inner.display_name()),
            ResolvedType::Function { params, ret } => {
                let params_str: Vec<String> = params.iter().map(|p| p.display_name()).collect();
                format!("fn({}) -> {}", params_str.join(", "), ret.display_name())
            }
            ResolvedType::Optional(inner) => format!("{}?", inner.display_name()),
            ResolvedType::Map(key, val) => format!("Map<{}, {}>", key.display_name(), val.display_name()),
            ResolvedType::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|e| e.display_name()).collect();
                format!("({})", parts.join(", "))
            }
            ResolvedType::Color => "Color".to_string(),
            ResolvedType::Unknown => "<unknown>".to_string(),
            ResolvedType::Error => "<error>".to_string(),
        }
    }
}

/// Symbol entry in symbol table
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub ty: ResolvedType,
    pub mutable: bool,
    pub kind: SymbolKind,
}

/// Kind of symbol
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Variable,
    Function,
    Struct,
    Parameter,
}

/// Struct definition info
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: HashMap<String, ResolvedType>,
}

/// Which declaration form introduced a nominal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Struct,
    Variant,
    Layer,
    Panel,
    Protocol,
}

impl TypeKind {
    pub fn keyword(&self) -> &'static str {
        match self {
            TypeKind::Struct => "struct",
            TypeKind::Variant => "variant",
            TypeKind::Layer => "layer",
            TypeKind::Panel => "panel",
            TypeKind::Protocol => "protocol",
        }
    }
}

/// Signature of a method attached to a nominal type. `params` excludes the
/// implicit `self`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodInfo {
    pub name: String,
    pub params: Vec<ResolvedType>,
    pub ret: ResolvedType,
    pub is_mut: bool,
    pub is_static: bool,
    pub span: crate::lexer::Span,
}

impl MethodInfo {
    /// `name(int, string) -> bool` for diagnostics.
    pub fn signature(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|p| p.display_name()).collect();
        let prefix = if self.is_static { "static " } else if self.is_mut { "mut " } else { "" };
        if self.ret == ResolvedType::Void {
            format!("{}{}({})", prefix, self.name, params.join(", "))
        } else {
            format!("{}{}({}) -> {}", prefix, self.name, params.join(", "), self.ret.display_name())
        }
    }
}

/// Scope in the symbol table
#[derive(Debug)]
struct Scope {
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }
}

/// Symbol table for scope management
#[derive(Debug)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    structs: HashMap<String, StructInfo>,
    functions: HashMap<String, ResolvedType>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()], // Global scope
            structs: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    /// Enter a new scope
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Exit current scope
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a symbol in current scope
    pub fn define(&mut self, symbol: Symbol) -> Result<(), String> {
        if self.scopes.is_empty() {
            // The global scope is created in `new` and `pop_scope` never removes
            // it, but recover rather than fail if the table was emptied anyway.
            self.scopes.push(Scope::new());
        }
        let scope = match self.scopes.last_mut() {
            Some(scope) => scope,
            None => return Err("no scope available for symbol definition".to_string()),
        };
        if scope.symbols.contains_key(&symbol.name) {
            return Err(format!(
                "symbol '{}' already defined in this scope",
                symbol.name
            ));
        }
        scope.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Look up a symbol by name (searches all scopes)
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Define a struct
    pub fn define_struct(&mut self, info: StructInfo) -> Result<(), String> {
        if self.structs.contains_key(&info.name) {
            return Err(format!("struct '{}' already defined", info.name));
        }
        self.structs.insert(info.name.clone(), info);
        Ok(())
    }

    /// Look up a struct
    pub fn lookup_struct(&self, name: &str) -> Option<&StructInfo> {
        self.structs.get(name)
    }

    /// Define a function
    pub fn define_function(&mut self, name: String, ty: ResolvedType) -> Result<(), String> {
        if self.functions.contains_key(&name) {
            return Err(format!("function '{}' already defined", name));
        }
        self.functions.insert(name, ty);
        Ok(())
    }

    /// Look up a function
    pub fn lookup_function(&self, name: &str) -> Option<&ResolvedType> {
        self.functions.get(name)
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table_basic() {
        let mut table = SymbolTable::new();
        
        table.define(Symbol {
            name: "x".to_string(),
            ty: ResolvedType::Int,
            mutable: false,
            kind: SymbolKind::Variable,
        }).unwrap();

        assert!(table.lookup("x").is_some());
        assert!(table.lookup("y").is_none());
    }

    #[test]
    fn test_symbol_table_scopes() {
        let mut table = SymbolTable::new();
        
        table.define(Symbol {
            name: "x".to_string(),
            ty: ResolvedType::Int,
            mutable: false,
            kind: SymbolKind::Variable,
        }).unwrap();

        table.push_scope();
        
        table.define(Symbol {
            name: "y".to_string(),
            ty: ResolvedType::Int,
            mutable: false,
            kind: SymbolKind::Variable,
        }).unwrap();

        // Can see both x and y
        assert!(table.lookup("x").is_some());
        assert!(table.lookup("y").is_some());

        table.pop_scope();

        // Can only see x now
        assert!(table.lookup("x").is_some());
        assert!(table.lookup("y").is_none());
    }

    #[test]
    fn test_type_assignability() {
        assert!(ResolvedType::Int.is_assignable_from(&ResolvedType::Int));
        assert!(ResolvedType::Float.is_assignable_from(&ResolvedType::Int));
        assert!(!ResolvedType::Int.is_assignable_from(&ResolvedType::String));
    }

    #[test]
    fn define_recovers_from_missing_scope() {
        let mut table = SymbolTable::new();
        table.scopes.clear();
        let result = table.define(Symbol {
            name: "x".into(),
            ty: ResolvedType::Int,
            mutable: false,
            kind: SymbolKind::Variable,
        });
        assert!(result.is_ok());
        assert!(table.lookup("x").is_some());
    }
}
