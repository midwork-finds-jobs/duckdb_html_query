extern crate duckdb;
extern crate duckdb_loadable_macros;
extern crate libduckdb_sys;

use duckdb::{
    core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId},
    ffi,
    types::DuckString,
    vscalar::{ScalarFunctionSignature, VScalar},
    vtab::arrow::WritableVector,
    Connection, Result,
};
use duckdb_loadable_macros::duckdb_entrypoint_c_api;
use hq::{process_html, HqConfig};
use libduckdb_sys::duckdb_string_t;
use std::error::Error;

/// HTML query scalar function
///
/// Processes HTML content using CSS selectors, similar to jq for JSON.
///
/// # Arguments
/// * `html` - A VARCHAR or BLOB containing HTML content
/// * `selector` - Optional VARCHAR with CSS selector (default: ":root")
/// * `text_only` - Optional BOOLEAN to extract text only (default: false)
/// * `pretty` - Optional BOOLEAN to pretty print output (default: false)
///
/// # Returns
/// * VARCHAR - Processed HTML content or NULL on error
///
/// # Notes
/// * When `text_only` is true, HTML entities are automatically decoded and JSON is validated/compacted
///
/// # Examples
/// ```sql
/// -- Extract title from HTML column
/// SELECT html_query(html_content, 'title') FROM pages;
///
/// -- Get text only from specific element
/// SELECT html_query(html_content, '.content', true) FROM pages;
///
/// -- Extract LD+JSON (automatically decodes entities when text_only=true)
/// SELECT html_query(html_content, 'script[type="application/ld+json"]', true) FROM pages;
/// ```
struct HtmlQueryFunction;

impl VScalar for HtmlQueryFunction {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn Error>> {
        let size = input.len();
        let html_vector = input.flat_vector(0);
        let mut output_vector = output.flat_vector();

        // Get HTML content
        let html_values = html_vector.as_slice_with_len::<duckdb_string_t>(size);
        let html_contents: Vec<String> = html_values
            .iter()
            .map(|ptr| DuckString::new(&mut { *ptr }).as_str().to_string())
            .collect();

        // Get selector (optional, column 1)
        let selectors: Vec<Option<String>> = if input.num_columns() > 1 {
            let selector_vector = input.flat_vector(1);
            let selector_values = selector_vector.as_slice_with_len::<duckdb_string_t>(size);
            (0..size)
                .map(|i| {
                    if selector_vector.row_is_null(i as u64) {
                        None
                    } else {
                        Some(
                            DuckString::new(&mut { selector_values[i] })
                                .as_str()
                                .to_string()
                        )
                    }
                })
                .collect()
        } else {
            vec![None; size]
        };

        // Get text_only flag (optional, column 2)
        let text_only_flags: Vec<bool> = if input.num_columns() > 2 {
            let text_only_vector = input.flat_vector(2);
            let text_only_values = text_only_vector.as_slice_with_len::<bool>(size);
            (0..size)
                .map(|i| {
                    if text_only_vector.row_is_null(i as u64) {
                        false
                    } else {
                        text_only_values[i]
                    }
                })
                .collect()
        } else {
            vec![false; size]
        };

        // Get pretty flag (optional, column 3)
        let pretty_flags: Vec<bool> = if input.num_columns() > 3 {
            let pretty_vector = input.flat_vector(3);
            let pretty_values = pretty_vector.as_slice_with_len::<bool>(size);
            (0..size)
                .map(|i| {
                    if pretty_vector.row_is_null(i as u64) {
                        false
                    } else {
                        pretty_values[i]
                    }
                })
                .collect()
        } else {
            vec![false; size]
        };

        // Process each row
        for i in 0..size {
            if html_vector.row_is_null(i as u64) {
                output_vector.set_null(i);
                continue;
            }

            // Get selector for this row, defaulting to ":root" if None
            let selector = selectors.get(i)
                .and_then(|s| s.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(":root");

            let config = HqConfig {
                selector: selector.to_string(),
                text_only: text_only_flags[i],
                pretty_print: pretty_flags[i],
                attributes: vec![],
                // Always use compact mode when extracting text to decode HTML entities
                compact: text_only_flags[i],
                ..Default::default()
            };

            match process_html(&html_contents[i], &config) {
                Ok(result) => {
                    output_vector.insert(i, result.trim());
                }
                Err(_) => {
                    output_vector.set_null(i);
                }
            }
        }

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![
            // hq(html)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // hq(html, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // hq(html, selector, text_only)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Boolean),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // hq(html, selector, text_only, pretty)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Boolean),
                    LogicalTypeHandle::from(LogicalTypeId::Boolean),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
        ]
    }
}

/// HTML attribute extraction scalar function
///
/// Extracts specific attributes from HTML elements matching a CSS selector.
///
/// # Arguments
/// * `html` - A VARCHAR or BLOB containing HTML content
/// * `attribute` - VARCHAR with attribute name (e.g., "href", "src")
/// * `selector` - Optional VARCHAR with CSS selector (default: ":root")
///
/// # Returns
/// * VARCHAR[] - Array of attribute values or NULL on error
///
/// # Examples
/// ```sql
/// -- Extract all hrefs from links
/// SELECT hq_attr(html_content, 'href', 'a') FROM pages;
///
/// -- Get all image sources
/// SELECT hq_attr(html_content, 'src', 'img') FROM pages;
/// ```
struct HqAttrFunction;

impl VScalar for HqAttrFunction {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn Error>> {
        let size = input.len();
        let html_vector = input.flat_vector(0);
        let attr_vector = input.flat_vector(1);
        let mut output_vector = output.list_vector();

        // Get HTML content
        let html_values = html_vector.as_slice_with_len::<duckdb_string_t>(size);
        let html_contents: Vec<String> = html_values
            .iter()
            .map(|ptr| DuckString::new(&mut { *ptr }).as_str().to_string())
            .collect();

        // Get attributes
        let attr_values = attr_vector.as_slice_with_len::<duckdb_string_t>(size);
        let attributes: Vec<String> = attr_values
            .iter()
            .map(|ptr| DuckString::new(&mut { *ptr }).as_str().to_string())
            .collect();

        // Get selector (optional, column 2)
        let selectors: Vec<Option<String>> = if input.num_columns() > 2 {
            let selector_vector = input.flat_vector(2);
            let selector_values = selector_vector.as_slice_with_len::<duckdb_string_t>(size);
            (0..size)
                .map(|i| {
                    if selector_vector.row_is_null(i as u64) {
                        None
                    } else {
                        Some(
                            DuckString::new(&mut { selector_values[i] })
                                .as_str()
                                .to_string()
                        )
                    }
                })
                .collect()
        } else {
            vec![None; size]
        };

        // Process to get all results first
        let all_results: Vec<Option<Vec<String>>> = (0..size)
            .map(|i| {
                if html_vector.row_is_null(i as u64) || attr_vector.row_is_null(i as u64) {
                    return None;
                }

                let config = HqConfig {
                    selector: selectors[i].clone().unwrap_or_else(|| ":root".to_string()),
                    text_only: false,
                    pretty_print: false,
                    attributes: vec![attributes[i].clone()],
                    compact: false,
                    ..Default::default()
                };

                match process_html(&html_contents[i], &config) {
                    Ok(result) => {
                        // Split result by newlines to get individual attribute values
                        let values: Vec<String> = result
                            .lines()
                            .filter(|l| !l.is_empty())
                            .map(|l| l.to_string())
                            .collect();
                        if values.is_empty() {
                            None
                        } else {
                            Some(values)
                        }
                    }
                    Err(_) => None,
                }
            })
            .collect();

        // Calculate total capacity
        let total_capacity: usize = all_results
            .iter()
            .map(|r| r.as_ref().map_or(0, |v| v.len()))
            .sum();

        // Get child vector
        let child_vector = output_vector.child(total_capacity);

        // Write results
        let mut offset = 0;
        for (i, result) in all_results.iter().enumerate() {
            match result {
                Some(values) => {
                    output_vector.set_entry(i, offset, values.len());
                    for value in values {
                        child_vector.insert(offset, value.as_str());
                        offset += 1;
                    }
                }
                None => {
                    output_vector.set_null(i);
                }
            }
        }

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![
            // hq_attr(html, attribute)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // hq_attr(html, attribute, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
        ]
    }
}

/// JavaScript string decoder function
///
/// Decodes JavaScript string literals with hex escapes, unicode escapes, and invalid escapes
///
/// # Arguments
/// * `js_string` - A VARCHAR containing a JavaScript string literal
///
/// # Returns
/// * VARCHAR - Decoded string or NULL on error
///
/// # Examples
/// ```sql
/// -- Decode JavaScript string with hex escapes
/// SELECT hq_decode_js_string('[{\x22Salary\x22:\x2250000$\x22}]');
/// -- Returns: [{"Salary":"50000$"}]
///
/// -- Use with regex to extract from JSON.parse()
/// SELECT hq_decode_js_string(
///   regexp_extract(hq(html, 'script', true), 'JSON\.parse\(''(.*)''', 1)
/// ) FROM pages;
/// ```
struct HqDecodeJsStringFunction;

impl VScalar for HqDecodeJsStringFunction {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn Error>> {
        let size = input.len();
        let input_vector = input.flat_vector(0);
        let mut output_vector = output.flat_vector();

        // Get input strings
        let input_values = input_vector.as_slice_with_len::<duckdb_string_t>(size);

        for i in 0..size {
            if input_vector.row_is_null(i as u64) {
                output_vector.set_null(i);
                continue;
            }

            let js_string = DuckString::new(&mut { input_values[i] })
                .as_str()
                .to_string();

            match hq::js_decode::decode_js_string(&js_string) {
                Ok(decoded) => {
                    output_vector.insert(i, decoded.as_str());
                }
                Err(_) => {
                    output_vector.set_null(i);
                }
            }
        }

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![
            // hq_decode_js_string(js_string)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
        ]
    }
}

#[duckdb_entrypoint_c_api()]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<HtmlQueryFunction>("html_query")?;
    con.register_scalar_function::<HqAttrFunction>("hq_attr")?;
    con.register_scalar_function::<HqDecodeJsStringFunction>("hq_decode_js_string")?;
    Ok(())
}
