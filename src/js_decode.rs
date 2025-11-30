use std::error::Error;

/// Result of extracting a JavaScript variable
#[derive(Debug, Clone)]
pub enum JsValue {
    /// JSON value (object, array, string, number, boolean, null)
    Json(serde_json::Value),
    /// Raw string that couldn't be parsed as JSON
    Raw(String),
}

impl JsValue {
    /// Convert to JSON string representation
    pub fn to_json_string(&self) -> String {
        match self {
            JsValue::Json(v) => serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()),
            JsValue::Raw(s) => serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s)),
        }
    }
}

/// Extract a JavaScript variable value from script content
///
/// Handles cases like:
/// - `var jobs = 10;` -> returns integer
/// - `var jobs = "hello";` -> returns string
/// - `var jobs = {"key": "value"};` -> returns JSON object
/// - `var jobs = [1, 2, 3];` -> returns JSON array
/// - `var jobs = JSON.parse('...');` -> decodes and returns JSON
/// - `var jobs = JSON.parse("...");` -> decodes and returns JSON
///
/// The `var_pattern` should be the variable declaration prefix, e.g., "var jobs" or "const data"
pub fn extract_js_variable(
    script_content: &str,
    var_pattern: &str,
) -> Result<JsValue, Box<dyn Error>> {
    // Find the variable assignment
    let pattern_with_eq = format!("{} = ", var_pattern.trim());
    let start_pos = script_content
        .find(&pattern_with_eq)
        .ok_or_else(|| format!("Variable pattern '{}' not found", var_pattern))?;

    let value_start = start_pos + pattern_with_eq.len();
    let remaining = &script_content[value_start..];

    // Check if it's a JSON.parse() call
    if remaining.trim_start().starts_with("JSON.parse(") {
        return extract_json_parse(remaining.trim_start());
    }

    // Otherwise, try to extract the raw value until semicolon or end
    let value_str = extract_until_statement_end(remaining)?;
    let trimmed = value_str.trim();

    // Try to parse as JSON first
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(JsValue::Json(json_val));
    }

    // Try with control char escaping (for multiline JSON in HTML)
    let fixed = super::escape_json_control_chars(trimmed);
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&fixed) {
        return Ok(JsValue::Json(json_val));
    }

    // Return as raw string
    Ok(JsValue::Raw(trimmed.to_string()))
}

/// Extract value from JSON.parse('...') or JSON.parse("...")
fn extract_json_parse(input: &str) -> Result<JsValue, Box<dyn Error>> {
    // Skip "JSON.parse("
    let after_parse = input
        .strip_prefix("JSON.parse(")
        .ok_or("Expected JSON.parse(")?;

    // Determine the quote character
    let quote_char = after_parse
        .chars()
        .next()
        .ok_or("Expected quote after JSON.parse(")?;

    if quote_char != '\'' && quote_char != '"' {
        return Err(format!("Expected ' or \" after JSON.parse(, got: {}", quote_char).into());
    }

    // Find the matching closing quote (handling escaped quotes)
    let content_start = 1; // after opening quote
    let content = &after_parse[content_start..];

    let mut end_pos = 0;
    let chars = content.chars();
    let mut escape_next = false;

    for ch in chars {
        if escape_next {
            escape_next = false;
            end_pos += ch.len_utf8();
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            end_pos += 1;
            continue;
        }

        if ch == quote_char {
            // Found closing quote
            break;
        }

        end_pos += ch.len_utf8();
    }

    let encoded_content = &content[..end_pos];

    // Check if content needs JS decoding (contains \x or \u escapes)
    let decoded = if encoded_content.contains("\\x") || encoded_content.contains("\\u") {
        decode_js_string(encoded_content)?
    } else {
        // Still need to handle basic escapes like \"
        decode_js_string(encoded_content)?
    };

    // Parse the decoded content as JSON
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&decoded) {
        return Ok(JsValue::Json(json_val));
    }

    // Try with control char escaping
    let fixed = super::escape_json_control_chars(&decoded);
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&fixed) {
        return Ok(JsValue::Json(json_val));
    }

    // Return decoded string as raw
    Ok(JsValue::Raw(decoded))
}

/// Extract value until statement end (semicolon, newline with no continuation, or EOF)
fn extract_until_statement_end(input: &str) -> Result<String, Box<dyn Error>> {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    let mut brace_depth = 0;
    let mut bracket_depth = 0;
    let mut in_string = false;
    let mut string_char = '"';
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }

        if ch == '\\' {
            result.push(ch);
            escape_next = true;
            continue;
        }

        if in_string {
            result.push(ch);
            if ch == string_char {
                in_string = false;
            }
            continue;
        }

        // Check for string start
        if ch == '"' || ch == '\'' {
            in_string = true;
            string_char = ch;
            result.push(ch);
            continue;
        }

        // Track braces and brackets
        match ch {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            ';' if brace_depth == 0 && bracket_depth == 0 => {
                // End of statement
                break;
            }
            '\n' if brace_depth == 0 && bracket_depth == 0 => {
                // Newline outside of object/array might be end
                // Check if next non-whitespace is continuation
                let rest: String = chars.clone().collect();
                let trimmed = rest.trim_start();
                if trimmed.is_empty() || !trimmed.starts_with(['.', ',', '+', '-', '*', '/']) {
                    break;
                }
            }
            _ => {}
        }

        result.push(ch);
    }

    Ok(result)
}

/// Decode JavaScript string literal to plain text
/// Handles: \xNN hex escapes, \uNNNN unicode escapes, \\u double escapes, invalid escapes like \-
pub fn decode_js_string(input: &str) -> Result<String, Box<dyn Error>> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('x') => {
                    // Hex escape: \xNN
                    chars.next(); // consume 'x'
                    let hex: String = chars.by_ref().take(2).collect();
                    if hex.len() == 2 {
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        } else {
                            return Err(format!("Invalid hex escape: \\x{}", hex).into());
                        }
                    } else {
                        return Err(format!("Incomplete hex escape: \\x{}", hex).into());
                    }
                }
                Some('u') => {
                    // Unicode escape: \uNNNN
                    chars.next(); // consume 'u'
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() == 4 {
                        if let Ok(code_point) = u32::from_str_radix(&hex, 16) {
                            if let Some(unicode_char) = char::from_u32(code_point) {
                                result.push(unicode_char);
                            } else {
                                return Err(
                                    format!("Invalid unicode code point: \\u{}", hex).into()
                                );
                            }
                        } else {
                            return Err(format!("Invalid unicode escape: \\u{}", hex).into());
                        }
                    } else {
                        return Err(format!("Incomplete unicode escape: \\u{}", hex).into());
                    }
                }
                Some('\\') => {
                    // Check for double-escaped unicode: \\uNNNN
                    chars.next(); // consume first \
                    if chars.peek() == Some(&'u') {
                        chars.next(); // consume 'u'
                        let hex: String = chars.by_ref().take(4).collect();
                        if hex.len() == 4 {
                            if let Ok(code_point) = u32::from_str_radix(&hex, 16) {
                                if let Some(unicode_char) = char::from_u32(code_point) {
                                    result.push(unicode_char);
                                } else {
                                    return Err(format!(
                                        "Invalid unicode code point: \\\\u{}",
                                        hex
                                    )
                                    .into());
                                }
                            } else {
                                return Err(format!("Invalid unicode escape: \\\\u{}", hex).into());
                            }
                        } else {
                            return Err(format!("Incomplete unicode escape: \\\\u{}", hex).into());
                        }
                    } else {
                        // Just a regular backslash
                        result.push('\\');
                    }
                }
                // Standard JSON escapes
                Some('n') => {
                    chars.next();
                    result.push('\n');
                }
                Some('r') => {
                    chars.next();
                    result.push('\r');
                }
                Some('t') => {
                    chars.next();
                    result.push('\t');
                }
                Some('"') => {
                    chars.next();
                    result.push('"');
                }
                Some('\'') => {
                    chars.next();
                    result.push('\'');
                }
                Some('b') => {
                    chars.next();
                    result.push('\u{0008}');
                }
                Some('f') => {
                    chars.next();
                    result.push('\u{000C}');
                }
                Some('v') => {
                    chars.next();
                    result.push('\u{000B}');
                }
                Some('0') => {
                    chars.next();
                    result.push('\0');
                }
                // Invalid escapes that JavaScript allows but JSON doesn't
                Some('-') => {
                    chars.next(); // consume '-'
                    result.push('-');
                }
                Some('/') => {
                    chars.next(); // consume '/'
                    result.push('/');
                }
                Some(&next_ch) => {
                    // Unknown escape, just output the character literally
                    chars.next();
                    result.push(next_ch);
                }
                None => {
                    result.push('\\');
                }
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Attempt to fix UTF-8 mojibake (text encoded as UTF-8 but interpreted as Latin-1)
pub fn fix_mojibake(input: &str) -> String {
    // Try to re-encode as latin-1, then decode as UTF-8
    let bytes: Vec<u8> = input
        .chars()
        .filter_map(|c| {
            let code = c as u32;
            if code <= 255 { Some(code as u8) } else { None }
        })
        .collect();

    match String::from_utf8(bytes) {
        Ok(fixed) => fixed,
        Err(_) => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== decode_js_string tests ====================

    #[test]
    fn test_hex_escapes() {
        let input = r#"[\x22Salary\x22:\x2250000$\x22]"#;
        let expected = r#"["Salary":"50000$"]"#;
        assert_eq!(decode_js_string(input).unwrap(), expected);
    }

    #[test]
    fn test_unicode_escapes() {
        let input = r#"Title\u2013Profil"#;
        let expected = "Title–Profil";
        assert_eq!(decode_js_string(input).unwrap(), expected);
    }

    #[test]
    fn test_double_unicode_escapes() {
        let input = r#"Title\\u2013Profil"#;
        let expected = "Title–Profil";
        assert_eq!(decode_js_string(input).unwrap(), expected);
    }

    #[test]
    fn test_invalid_escapes() {
        let input = r#"50000$\-80000$"#;
        let expected = "50000$-80000$";
        assert_eq!(decode_js_string(input).unwrap(), expected);
    }

    #[test]
    fn test_combined() {
        let input =
            r#"[\x22Salary\x22:\x2250000$ \- 80000$\x22,\x22Langue\x22:[\x22Français\x22]]"#;
        let expected = r#"["Salary":"50000$ - 80000$","Langue":["Français"]]"#;
        assert_eq!(decode_js_string(input).unwrap(), expected);
    }

    // ==================== extract_js_variable tests ====================

    #[test]
    fn test_extract_simple_integer() {
        let script = "var count = 42;";
        let result = extract_js_variable(script, "var count").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(42)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_simple_float() {
        let script = "var price = 19.99;";
        let result = extract_js_variable(script, "var price").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(19.99)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_simple_string() {
        let script = r#"var message = "Hello World";"#;
        let result = extract_js_variable(script, "var message").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!("Hello World")),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_boolean_true() {
        let script = "var enabled = true;";
        let result = extract_js_variable(script, "var enabled").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(true)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_boolean_false() {
        let script = "var disabled = false;";
        let result = extract_js_variable(script, "var disabled").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(false)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_null() {
        let script = "var empty = null;";
        let result = extract_js_variable(script, "var empty").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(null)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_object() {
        let script = r#"var config = {"debug": true, "name": "test"};"#;
        let result = extract_js_variable(script, "var config").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v["debug"], serde_json::json!(true));
                assert_eq!(v["name"], serde_json::json!("test"));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_array() {
        let script = r#"var items = [1, 2, 3, "hello"];"#;
        let result = extract_js_variable(script, "var items").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v, serde_json::json!([1, 2, 3, "hello"]));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_nested_json() {
        let script = r#"var data = {"users": [{"name": "Alice"}, {"name": "Bob"}]};"#;
        let result = extract_js_variable(script, "var data").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v["users"][0]["name"], serde_json::json!("Alice"));
                assert_eq!(v["users"][1]["name"], serde_json::json!("Bob"));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_const_keyword() {
        let script = r#"const API_KEY = "abc123";"#;
        let result = extract_js_variable(script, "const API_KEY").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!("abc123")),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_let_keyword() {
        let script = "let counter = 100;";
        let result = extract_js_variable(script, "let counter").unwrap();
        match result {
            JsValue::Json(v) => assert_eq!(v, serde_json::json!(100)),
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_parse_single_quote() {
        let script = r#"var jobs = JSON.parse('[{"id": 1}]');"#;
        let result = extract_js_variable(script, "var jobs").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v[0]["id"], serde_json::json!(1));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_parse_double_quote() {
        let script = r#"var jobs = JSON.parse("[{\"id\": 2}]");"#;
        let result = extract_js_variable(script, "var jobs").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v[0]["id"], serde_json::json!(2));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_parse_with_hex_escapes() {
        let script = r#"var data = JSON.parse('[{\x22name\x22:\x22Test\x22}]');"#;
        let result = extract_js_variable(script, "var data").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v[0]["name"], serde_json::json!("Test"));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_parse_with_unicode_escapes() {
        let script = r#"var title = JSON.parse('{"name": "Caf\u00e9"}');"#;
        let result = extract_js_variable(script, "var title").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v["name"], serde_json::json!("Café"));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_json_parse_complex() {
        // Simulates the zoho career page pattern
        let script = r#"var jobs = JSON.parse('[{\x22Salary\x22:\x2250000$\x22,\x22City\x22:\x22Montreal\x22}]');"#;
        let result = extract_js_variable(script, "var jobs").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v[0]["Salary"], serde_json::json!("50000$"));
                assert_eq!(v[0]["City"], serde_json::json!("Montreal"));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_variable_not_found() {
        let script = "var other = 123;";
        let result = extract_js_variable(script, "var missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_among_multiple_vars() {
        let script = r#"
            var first = 1;
            var second = 2;
            var target = {"found": true};
            var fourth = 4;
        "#;
        let result = extract_js_variable(script, "var target").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v["found"], serde_json::json!(true));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_multiline_json() {
        let script = r#"var config = {
            "name": "test",
            "value": 42
        };"#;
        let result = extract_js_variable(script, "var config").unwrap();
        match result {
            JsValue::Json(v) => {
                assert_eq!(v["name"], serde_json::json!("test"));
                assert_eq!(v["value"], serde_json::json!(42));
            }
            _ => panic!("Expected Json value"),
        }
    }

    #[test]
    fn test_extract_raw_value_fallback() {
        // When value can't be parsed as JSON, return as raw string
        let script = "var expr = someFunction();";
        let result = extract_js_variable(script, "var expr").unwrap();
        match result {
            JsValue::Raw(s) => assert_eq!(s, "someFunction()"),
            _ => panic!("Expected Raw value"),
        }
    }

    #[test]
    fn test_js_value_to_json_string() {
        let json_val = JsValue::Json(serde_json::json!({"key": "value"}));
        assert_eq!(json_val.to_json_string(), r#"{"key":"value"}"#);

        let raw_val = JsValue::Raw("hello".to_string());
        assert_eq!(raw_val.to_json_string(), r#""hello""#);
    }
}
