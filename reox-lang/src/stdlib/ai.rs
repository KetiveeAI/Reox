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
