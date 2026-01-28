// REOX Standard Library - IO Module
// Provides file and console I/O operations

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Read entire file contents as string
pub fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Write string content to file
pub fn write_file(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

/// Append content to file
pub fn append_file(path: &str, content: &str) -> Result<(), String> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes()).map_err(|e| e.to_string())
}

/// Check if file or directory exists
pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Check if path is a file
pub fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

/// Check if path is a directory
pub fn is_dir(path: &str) -> bool {
    Path::new(path).is_dir()
}

/// Read a line from stdin
pub fn read_line() -> Result<String, String> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim_end().to_string())
}

/// Print to stdout without newline
pub fn print_raw(s: &str) {
    print!("{}", s);
    let _ = io::stdout().flush();
}

/// Print to stdout with newline
pub fn println_raw(s: &str) {
    println!("{}", s);
}

/// List directory contents
pub fn list_dir(path: &str) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for entry in entries {
        if let Ok(e) = entry {
            if let Some(name) = e.file_name().to_str() {
                result.push(name.to_string());
            }
        }
    }
    Ok(result)
}

/// Get file size in bytes
pub fn file_size(path: &str) -> Result<u64, String> {
    fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| e.to_string())
}

/// Delete a file
pub fn delete_file(path: &str) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| e.to_string())
}

/// Create a directory (including parent directories)
pub fn create_dir(path: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
    #[test]
    fn test_exists() {
        assert!(exists("Cargo.toml"));
        assert!(!exists("nonexistent_file_12345.txt"));
    }
    
    #[test]
    fn test_is_file() {
        assert!(is_file("Cargo.toml"));
        assert!(!is_file("src"));
    }
    
    #[test]
    fn test_is_dir() {
        assert!(is_dir("src"));
        assert!(!is_dir("Cargo.toml"));
    }
    
    #[test]
    fn test_read_write_file() {
        let test_path = "/tmp/reox_test_io.txt";
        let content = "Hello, REOX!";
        
        // Write
        assert!(write_file(test_path, content).is_ok());
        
        // Read
        let result = read_file(test_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
        
        // Cleanup
        let _ = fs::remove_file(test_path);
    }
}
