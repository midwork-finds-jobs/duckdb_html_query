pub mod link;
pub mod pretty_print;

use kuchikiki::NodeRef;
use kuchikiki::traits::{NodeIterator, TendrilSink};
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
        for attr in attributes {
            if let Ok(elem_atts) = as_element.attributes.try_borrow()
                && let Some(val) = elem_atts.get(attr.as_str())
            {
                writeln!(output, "{val}").ok();
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

    // Compact output if requested
    if config.compact {
        // Try to parse as JSON first (trim whitespace before parsing)
        let trimmed = result.trim();

        // Try direct parse first
        let json_value = serde_json::from_str::<serde_json::Value>(trimmed)
            .or_else(|_| {
                // If it fails, try fixing malformed JSON by escaping control chars inside strings
                let mut fixed = String::with_capacity(trimmed.len() * 2);
                let mut in_string = false;
                let mut escape_next = false;

                for c in trimmed.chars() {
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

                serde_json::from_str::<serde_json::Value>(&fixed)
            });

        if let Ok(json_value) = json_value {
            // If it's valid JSON, serialize it compactly
            // This preserves spaces within text values while removing structural whitespace
            result = serde_json::to_string(&json_value)?;
        } else {
            // If not JSON, minify HTML by removing whitespace between tags
            result = result
                .split('>')
                .map(|s| s.trim_start())
                .collect::<Vec<_>>()
                .join(">");
        }
    }

    Ok(result)
}
