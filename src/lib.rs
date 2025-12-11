pub mod js_decode;
pub mod link;
pub mod pretty_print;

#[cfg(feature = "duckdb")]
pub mod duckdb;

use kuchikiki::traits::{NodeIterator, TendrilSink};
use kuchikiki::NodeRef;
use std::error::Error;
use std::io::{self, Write};
use url::Url;

#[derive(Debug, Clone)]
pub struct HqConfig {
    pub selector: String,
    pub base: Option<String>,
    pub detect_base: bool,
    pub text_only: bool,
    pub ignore_whitespace: bool,
    pub pretty_print: bool,
    pub remove_nodes: Vec<String>,
    pub attributes: Vec<String>,
    pub compact: bool,
}

impl Default for HqConfig {
    fn default() -> Self {
        Self {
            selector: ":root".to_string(),
            base: None,
            detect_base: false,
            text_only: false,
            ignore_whitespace: false,
            pretty_print: false,
            remove_nodes: Vec::new(),
            attributes: Vec::new(),
            compact: false,
        }
    }
}

fn select_attributes(node: &NodeRef, attributes: &[String], output: &mut dyn io::Write) {
    if let Some(as_element) = node.as_element() {
        if let Ok(elem_atts) = as_element.attributes.try_borrow() {
            for attr in attributes {
                if let Some(val) = elem_atts.get(attr.as_str()) {
                    writeln!(output, "{val}").ok();
                }
            }
        }
    }
}

fn serialize_text(node: &NodeRef, ignore_whitespace: bool) -> String {
    let mut result = String::new();
    for text_node in node.inclusive_descendants().text_nodes() {
        if ignore_whitespace && text_node.borrow().trim().is_empty() {
            continue;
        }

        result.push_str(&text_node.borrow());

        if ignore_whitespace {
            result.push('\n');
        }
    }

    result
}

/// Extract text content from all elements matching selector, returning each separately
pub fn extract_all_text(html: &str, selector: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let document = kuchikiki::parse_html().one(html);
    let mut results = Vec::new();

    for node in document
        .select(selector)
        .map_err(|_| "Failed to parse CSS selector")?
    {
        let text = serialize_text(node.as_node(), false).trim().to_string();
        if !text.is_empty() {
            results.push(text);
        }
    }

    Ok(results)
}

/// Extraction mode for html_query functions
#[derive(Debug, Clone)]
pub enum ExtractMode {
    /// Return full HTML of element
    Html,
    /// Return text content only
    Text,
    /// Return specific attribute value
    Attribute(String),
    /// Return multiple attributes as JSON object
    MultiAttribute(Vec<String>),
}

impl ExtractMode {
    /// Parse extraction mode from optional attribute string
    /// None or empty -> Html
    /// "@text" or "text" -> Text
    /// "@attr" -> Attribute(attr)
    pub fn from_attr(attr: Option<&str>) -> Self {
        match attr {
            None | Some("") => ExtractMode::Html,
            Some("@text") | Some("text") => ExtractMode::Text,
            Some(s) if s.starts_with('@') => ExtractMode::Attribute(s[1..].to_string()),
            Some(s) => ExtractMode::Attribute(s.to_string()),
        }
    }

    /// Parse extraction mode from list of attributes
    /// Returns MultiAttribute for multiple attrs, or single mode for one
    pub fn from_attr_list(attrs: &[String]) -> Self {
        match attrs.len() {
            0 => ExtractMode::Html,
            1 => Self::from_attr(Some(&attrs[0])),
            _ => {
                let normalized: Vec<String> = attrs
                    .iter()
                    .map(|s| {
                        if s == "@text" || s == "text" {
                            "@text".to_string() // special marker for text
                        } else if let Some(stripped) = s.strip_prefix('@') {
                            stripped.to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .collect();
                ExtractMode::MultiAttribute(normalized)
            }
        }
    }
}

/// Extract all elements matching selector, returning each as separate string (HTML or text)
pub fn extract_all_elements(
    html: &str,
    selector: &str,
    text_only: bool,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mode = if text_only {
        ExtractMode::Text
    } else {
        ExtractMode::Html
    };
    extract_all_with_mode(html, selector, &mode)
}

/// Extract all elements matching selector with specified extraction mode
pub fn extract_all_with_mode(
    html: &str,
    selector: &str,
    mode: &ExtractMode,
) -> Result<Vec<String>, Box<dyn Error>> {
    let document = kuchikiki::parse_html().one(html);
    let mut results = Vec::new();

    for node in document
        .select(selector)
        .map_err(|_| "Failed to parse CSS selector")?
    {
        let content = match mode {
            ExtractMode::Text => serialize_text(node.as_node(), false).trim().to_string(),
            ExtractMode::Html => node.as_node().to_string(),
            ExtractMode::Attribute(attr) => {
                if let Some(element) = node.as_node().as_element() {
                    if let Ok(attrs) = element.attributes.try_borrow() {
                        attrs.get(attr.as_str()).unwrap_or("").to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
            ExtractMode::MultiAttribute(attr_list) => {
                let mut obj = serde_json::Map::new();
                if let Some(element) = node.as_node().as_element() {
                    if let Ok(attrs) = element.attributes.try_borrow() {
                        for attr in attr_list {
                            let value = if attr == "@text" {
                                serialize_text(node.as_node(), false).trim().to_string()
                            } else {
                                attrs.get(attr.as_str()).unwrap_or("").to_string()
                            };
                            // Use "text" as key for @text
                            let key = if attr == "@text" { "text" } else { attr };
                            obj.insert(key.to_string(), serde_json::Value::String(value));
                        }
                    }
                }
                serde_json::to_string(&obj).unwrap_or_default()
            }
        };
        if !content.is_empty() {
            results.push(content);
        }
    }

    Ok(results)
}

pub fn process_html(html: &str, config: &HqConfig) -> Result<String, Box<dyn Error>> {
    let document = kuchikiki::parse_html().one(html);

    let base: Option<Url> = match (&config.base, &config.detect_base) {
        (Some(base), true) => link::detect_base(&document).or(Url::parse(base).ok()),
        (Some(base), false) => Url::parse(base).ok(),
        (None, true) => link::detect_base(&document),
        _ => None,
    };

    let mut output = Vec::new();

    for node in document
        .select(&config.selector)
        .map_err(|_| "Failed to parse CSS selector")?
    {
        let node = node.as_node();

        // detach those nodes that should be removed
        if let Ok(targets) = node.select(&config.remove_nodes.join(",")) {
            for target in targets {
                target.as_node().detach();
            }
        }

        if let Some(base) = &base {
            link::rewrite_relative_url(node, base);
        }

        if !config.attributes.is_empty() {
            select_attributes(node, &config.attributes, &mut output);
            continue;
        }

        if config.text_only {
            writeln!(output, "{}", serialize_text(node, config.ignore_whitespace)).ok();
            continue;
        }

        if config.pretty_print {
            writeln!(output, "{}", pretty_print::pretty_print(node)).ok();
            continue;
        }

        writeln!(output, "{}", node.to_string()).ok();
    }

    let mut result = String::from_utf8(output)?;

    // Compact output if requested - produces valid JSON by escaping control chars
    if config.compact {
        let trimmed = result.trim();

        // Try direct parse first
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            result = serde_json::to_string(&json_value)?;
        } else {
            // Fix malformed JSON by escaping control chars inside strings
            let fixed = escape_json_control_chars(trimmed);
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&fixed) {
                result = serde_json::to_string(&json_value)?;
            }
            // If still not valid JSON, return as-is (might be HTML)
        }
    }

    Ok(result)
}

/// Escape control characters inside JSON strings to produce valid JSON
pub fn escape_json_control_chars(input: &str) -> String {
    let mut fixed = String::with_capacity(input.len() * 2);
    let mut in_string = false;
    let mut escape_next = false;

    for c in input.chars() {
        if escape_next {
            fixed.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' {
            fixed.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            fixed.push(c);
            continue;
        }

        // Only escape control characters when inside strings
        if in_string && c.is_control() {
            match c {
                '\n' => fixed.push_str("\\n"),
                '\r' => fixed.push_str("\\r"),
                '\t' => fixed.push_str("\\t"),
                _ => {} // Skip other control chars
            }
        } else {
            fixed.push(c);
        }
    }

    fixed
}
