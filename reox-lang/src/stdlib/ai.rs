use crate::interpreter::Value;
use reqwest::blocking::Client;
use serde_json::json;

pub fn generate(args: Vec<Value>) -> Value {
    if args.len() < 2 {
        return Value::String("Error: Expected model and prompt".to_string());
    }

    let model = match &args[0] {
        Value::String(s) => s,
        _ => return Value::String("Error: Model must be a string".to_string()),
    };

    let prompt = match &args[1] {
        Value::String(s) => s,
        _ => return Value::String("Error: Prompt must be a string".to_string()),
    };

    // Optional API Key (if provided as 3rd arg, else use env or default)
    let api_key = if args.len() > 2 {
        match &args[2] {
            Value::String(s) => s.clone(),
            _ => String::new(),
        }
    } else {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    };

    if api_key.is_empty() {
        return Value::String("Error: API Key not provided".to_string());
    }

    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let body = json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }]
    });

    match client.post(&url).json(&body).send() {
        Ok(res) => {
            if res.status().is_success() {
                match res.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            Value::String(text.to_string())
                        } else {
                            Value::String(format!("Error: Unexpected response format: {}", json))
                        }
                    }
                    Err(e) => Value::String(format!("Error parsing JSON: {}", e)),
                }
            } else {
                Value::String(format!("Error: API request failed with status {}", res.status()))
            }
        }
        Err(e) => Value::String(format!("Error sending request: {}", e)),
    }
}

// ============== AI Helper Functions ==============

/// Ask AI to complete code
/// ai_complete("fn calculate_sum(") -> "fn calculate_sum(a: int, b: int) -> int { return a + b; }"
pub fn ai_complete(code_fragment: &str) -> String {
    let prompt = format!(
        "Complete this REOX code. Only return the completed code, no explanations:\n\n{}",
        code_fragment
    );
    format!("AI_COMPLETE: {}", prompt)  // Placeholder - actual impl would call generate
}

/// Ask AI to explain code
/// ai_explain("let x = map.filter(|k, v| v > 10);") -> "This filters a map..."
pub fn ai_explain(code: &str) -> String {
    let prompt = format!(
        "Explain this REOX code in simple terms. Be concise:\n\n{}",
        code
    );
    format!("AI_EXPLAIN: {}", prompt)
}

/// Ask AI to fix code error
/// ai_fix("type mismatch: expected int, got string") -> suggested fix
pub fn ai_fix(error_message: &str, code_context: &str) -> String {
    let prompt = format!(
        "Fix this REOX code error. Error: {}\n\nCode:\n{}\n\nProvide the corrected code:",
        error_message, code_context
    );
    format!("AI_FIX: {}", prompt)
}

/// Generate UI component from description
/// ai_ui("a login form with email and password fields") -> REOX UI code
pub fn ai_ui(description: &str) -> String {
    let prompt = format!(
        "Generate REOX UI code for: {}. Use vstack, hstack, text, button, input components. Return only the code:",
        description
    );
    format!("AI_UI: {}", prompt)
}

/// Check if code contains potential issues
/// ai_review("fn divide(a: int, b: int) -> int { return a / b; }") -> "Warning: No zero check for divisor"
pub fn ai_review(code: &str) -> String {
    let prompt = format!(
        "Review this REOX code for potential bugs, security issues, or improvements. Be brief:\n\n{}",
        code
    );
    format!("AI_REVIEW: {}", prompt)
}
