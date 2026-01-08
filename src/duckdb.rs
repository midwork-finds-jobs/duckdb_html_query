extern crate duckdb;
extern crate duckdb_loadable_macros;
extern crate libduckdb_sys;

use crate::{
    extract_all_text, extract_all_with_mode, js_decode, process_html, ExtractMode, HqConfig,
};
use duckdb::{
    core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId},
    ffi,
    types::DuckString,
    vscalar::{ScalarFunctionSignature, VScalar},
    vtab::arrow::WritableVector,
    Connection, Result,
};
use duckdb_loadable_macros::duckdb_entrypoint_c_api;
use libduckdb_sys::{
    duckdb_data_chunk_get_vector, duckdb_list_entry, duckdb_list_vector_get_child, duckdb_string_t,
    duckdb_vector_get_data,
};
use std::error::Error;

/// Read a list of strings from a list vector at given row index
unsafe fn read_string_list(
    input: &mut DataChunkHandle,
    col_idx: usize,
    row_idx: usize,
    chunk_size: usize,
) -> Vec<String> {
    // Get raw vector pointer
    let list_vec_ptr = duckdb_data_chunk_get_vector(input.get_ptr(), col_idx as u64);

    // Get list entries (offset/length pairs)
    let entries_ptr = duckdb_vector_get_data(list_vec_ptr) as *const duckdb_list_entry;
    let entries = std::slice::from_raw_parts(entries_ptr, chunk_size);

    let entry = &entries[row_idx];
    let offset = entry.offset as usize;
    let length = entry.length as usize;

    if length == 0 {
        return Vec::new();
    }

    // Get child vector (contains the actual strings)
    let child_vec_ptr = duckdb_list_vector_get_child(list_vec_ptr);
    let child_data_ptr = duckdb_vector_get_data(child_vec_ptr) as *const duckdb_string_t;

    // Read strings from offset to offset+length
    (offset..offset + length)
        .map(|i| {
            let str_ptr = child_data_ptr.add(i);
            DuckString::new(&mut { *str_ptr }).as_str().to_string()
        })
        .collect()
}

/// HTML query scalar function - returns first matching element
///
/// Extracts first HTML element matching CSS selector.
///
/// # Arguments
/// * `html` - VARCHAR containing HTML content
/// * `selector` - Optional VARCHAR with CSS selector (default: ":root")
/// * `extract` - Optional VARCHAR or VARCHAR[] specifying what to extract:
///   - NULL or omitted: full HTML
///   - '@text' or 'text': inner text content
///   - '@href', '@src', etc: attribute value
///   - ['@href', '@text']: multiple attributes as JSON object
///
/// # Returns
/// * VARCHAR - First matching element/attribute, or NULL if no match
///
/// # Examples
/// ```sql
/// SELECT html_query(html, 'a', '@href') FROM pages;
/// -- Returns: "/path/to/page"
///
/// SELECT html_query(html, 'a', ['@href', '@text']) FROM pages;
/// -- Returns: {"href": "/path", "text": "Link"}
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
                                .to_string(),
                        )
                    }
                })
                .collect()
        } else {
            vec![None; size]
        };

        // Get extract mode (optional, column 2) - can be VARCHAR or VARCHAR[]
        let extract_modes: Vec<ExtractMode> = if input.num_columns() > 2 {
            let col_type = input.flat_vector(2).logical_type();
            let is_list = col_type.id() == LogicalTypeId::List;

            if is_list {
                // VARCHAR[] - multi-attribute mode
                let list_entries_vector = input.flat_vector(2);
                (0..size)
                    .map(|i| {
                        if list_entries_vector.row_is_null(i as u64) {
                            ExtractMode::Html
                        } else {
                            let attrs = read_string_list(input, 2, i, size);
                            ExtractMode::from_attr_list(&attrs)
                        }
                    })
                    .collect()
            } else {
                // VARCHAR - single attribute mode
                let extract_vector = input.flat_vector(2);
                let extract_values = extract_vector.as_slice_with_len::<duckdb_string_t>(size);
                (0..size)
                    .map(|i| {
                        if extract_vector.row_is_null(i as u64) {
                            ExtractMode::Html
                        } else {
                            let s = DuckString::new(&mut { extract_values[i] })
                                .as_str()
                                .to_string();
                            ExtractMode::from_attr(Some(&s))
                        }
                    })
                    .collect()
            }
        } else {
            vec![ExtractMode::Html; size]
        };

        for i in 0..size {
            if html_vector.row_is_null(i as u64) {
                output_vector.set_null(i);
                continue;
            }

            let selector = selectors
                .get(i)
                .and_then(|s| s.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(":root");

            let mode = &extract_modes[i];

            match extract_all_with_mode(&html_contents[i], selector, mode) {
                Ok(elements) if elements.is_empty() => {
                    output_vector.set_null(i);
                }
                Ok(elements) => {
                    // Return first element only
                    output_vector.insert(i, &elements[0]);
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
            // html_query(html VARCHAR)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html BLOB)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Blob)],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html VARCHAR, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html BLOB, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html VARCHAR, selector, extract)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html BLOB, selector, extract)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html VARCHAR, selector, extract[])
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_query(html BLOB, selector, extract[])
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
        ]
    }
}

/// HTML query all scalar function - returns VARCHAR[] of all matching elements
///
/// Extracts all HTML elements matching CSS selector as a list.
///
/// # Arguments
/// * `html` - VARCHAR containing HTML content
/// * `selector` - Optional VARCHAR with CSS selector (default: ":root")
/// * `extract` - Optional VARCHAR or VARCHAR[] specifying what to extract:
///   - NULL or omitted: full HTML
///   - '@text' or 'text': inner text content
///   - '@href', '@src', etc: attribute value
///   - ['@href', '@text']: multiple attributes as JSON objects
///
/// # Returns
/// * VARCHAR[] - Array of all matching elements/attributes
///
/// # Examples
/// ```sql
/// SELECT html_query_all(html, 'a', '@href') FROM pages;
/// -- Returns: ['/page1', '/page2', '/page3']
///
/// SELECT html_query_all(html, 'a', ['@href', '@text']) FROM pages;
/// -- Returns: ['{"href":"/page1","text":"Link 1"}', '{"href":"/page2","text":"Link 2"}']
/// ```
struct HtmlQueryAllFunction;

impl VScalar for HtmlQueryAllFunction {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn Error>> {
        let size = input.len();
        let html_vector = input.flat_vector(0);

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
                                .to_string(),
                        )
                    }
                })
                .collect()
        } else {
            vec![None; size]
        };

        // Get extract mode (optional, column 2) - can be VARCHAR or VARCHAR[]
        let extract_modes: Vec<ExtractMode> = if input.num_columns() > 2 {
            let col_type = input.flat_vector(2).logical_type();
            let is_list = col_type.id() == LogicalTypeId::List;

            if is_list {
                // VARCHAR[] - multi-attribute mode
                let list_entries_vector = input.flat_vector(2);
                (0..size)
                    .map(|i| {
                        if list_entries_vector.row_is_null(i as u64) {
                            ExtractMode::Html
                        } else {
                            let attrs = read_string_list(input, 2, i, size);
                            ExtractMode::from_attr_list(&attrs)
                        }
                    })
                    .collect()
            } else {
                // VARCHAR - single attribute mode
                let extract_vector = input.flat_vector(2);
                let extract_values = extract_vector.as_slice_with_len::<duckdb_string_t>(size);
                (0..size)
                    .map(|i| {
                        if extract_vector.row_is_null(i as u64) {
                            ExtractMode::Html
                        } else {
                            let s = DuckString::new(&mut { extract_values[i] })
                                .as_str()
                                .to_string();
                            ExtractMode::from_attr(Some(&s))
                        }
                    })
                    .collect()
            }
        } else {
            vec![ExtractMode::Html; size]
        };

        // Collect all results first to calculate total capacity
        let mut all_results: Vec<Vec<String>> = Vec::with_capacity(size);
        let mut total_elements = 0;

        for i in 0..size {
            if html_vector.row_is_null(i as u64) {
                all_results.push(Vec::new());
                continue;
            }

            let selector = selectors
                .get(i)
                .and_then(|s| s.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(":root");

            let mode = &extract_modes[i];

            match extract_all_with_mode(&html_contents[i], selector, mode) {
                Ok(elements) => {
                    total_elements += elements.len();
                    all_results.push(elements);
                }
                Err(_) => {
                    all_results.push(Vec::new());
                }
            }
        }

        // Now populate the list vector
        let mut list_vector = output.list_vector();
        let child_vector = list_vector.child(total_elements);

        let mut current_offset = 0;
        for (i, elements) in all_results.iter().enumerate() {
            if html_vector.row_is_null(i as u64) {
                list_vector.set_null(i);
                continue;
            }

            // Insert each element into child vector
            for (j, element) in elements.iter().enumerate() {
                child_vector.insert(current_offset + j, element.as_str());
            }

            // Set the entry offset and length
            list_vector.set_entry(i, current_offset, elements.len());
            current_offset += elements.len();
        }

        list_vector.set_len(total_elements);

        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![
            // html_query_all(html VARCHAR)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html BLOB)
            ScalarFunctionSignature::exact(
                vec![LogicalTypeHandle::from(LogicalTypeId::Blob)],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html VARCHAR, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html BLOB, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html VARCHAR, selector, extract)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html BLOB, selector, extract)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html VARCHAR, selector, extract[])
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
            // html_query_all(html BLOB, selector, extract[])
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
                ],
                LogicalTypeHandle::list(&LogicalTypeHandle::from(LogicalTypeId::Varchar)),
            ),
        ]
    }
}

/// Parse JSON string and decode HTML entities in values
fn parse_and_decode_json(input: &str) -> Option<String> {
    let trimmed = input.trim();

    // Try to parse JSON first, then decode HTML entities in string values
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let decoded_json = decode_html_in_json(json_val);
        return serde_json::to_string(&decoded_json).ok();
    }

    // If not valid JSON, try decoding entities first then parsing
    let decoded = htmlescape::decode_html(trimmed).unwrap_or_else(|_| trimmed.to_string());
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&decoded) {
        return serde_json::to_string(&json_val).ok();
    }

    // Return decoded string as-is if not valid JSON
    Some(decoded)
}

/// Recursively decode HTML entities in JSON string values
fn decode_html_in_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let decoded = htmlescape::decode_html(&s).unwrap_or(s);
            serde_json::Value::String(decoded)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(decode_html_in_json).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, decode_html_in_json(v)))
                .collect(),
        ),
        other => other,
    }
}

/// Extract JSON from HTML - unified function for LD+JSON and JS variables
///
/// Extracts JSON from HTML script tags. Supports two modes:
/// 1. Direct JSON extraction: For ld+json scripts, decodes HTML entities
/// 2. JS variable extraction: For scripts containing var/const/let assignments
///
/// # Signatures
/// * `html_extract_json(html, selector)` - Extract JSON from script matching selector
/// * `html_extract_json(html, selector, var_pattern)` - Extract JS variable from script
///
/// # Returns
/// * VARCHAR - JSON string or NULL on error
///
/// # Examples
/// ```sql
/// -- Extract LD+JSON (decodes HTML entities automatically)
/// SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;
///
/// -- Extract JS variable from script
/// SELECT html_extract_json(html, 'script', 'var jobs') FROM pages;
///
/// -- Access JSON fields
/// SELECT json_extract_string(
///   html_extract_json(html, 'script[type="application/ld+json"]'),
///   '$.title'
/// ) FROM pages;
/// ```
struct HtmlExtractJsonFunction;

impl VScalar for HtmlExtractJsonFunction {
    type State = ();

    unsafe fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn Error>> {
        let size = input.len();
        let html_vector = input.flat_vector(0);
        let selector_vector = input.flat_vector(1);
        let mut output_vector = output.flat_vector();

        let html_values = html_vector.as_slice_with_len::<duckdb_string_t>(size);
        let selector_values = selector_vector.as_slice_with_len::<duckdb_string_t>(size);

        // Get optional var_pattern (column 2)
        let var_patterns: Vec<Option<String>> = if input.num_columns() > 2 {
            let var_vector = input.flat_vector(2);
            let var_values = var_vector.as_slice_with_len::<duckdb_string_t>(size);
            (0..size)
                .map(|i| {
                    if var_vector.row_is_null(i as u64) {
                        None
                    } else {
                        Some(DuckString::new(&mut { var_values[i] }).as_str().to_string())
                    }
                })
                .collect()
        } else {
            vec![None; size]
        };

        for i in 0..size {
            if html_vector.row_is_null(i as u64) || selector_vector.row_is_null(i as u64) {
                output_vector.set_null(i);
                continue;
            }

            let html = DuckString::new(&mut { html_values[i] })
                .as_str()
                .to_string();
            let selector = DuckString::new(&mut { selector_values[i] })
                .as_str()
                .to_string();

            // Extract script content using selector
            let config = HqConfig {
                selector: selector.clone(),
                text_only: true,
                compact: true,
                ..Default::default()
            };

            let script_content = match process_html(&html, &config) {
                Ok(content) => content,
                Err(_) => {
                    output_vector.set_null(i);
                    continue;
                }
            };

            let result = if let Some(var_pattern) = &var_patterns[i] {
                if let Some(json_key) = var_pattern.strip_prefix("@nextjs_rsc:") {
                    // Mode 3: Extract Next.js RSC data - search all scripts for JSON with key
                    match extract_all_text(&html, "script") {
                        Ok(scripts) => {
                            let mut all_results: Vec<serde_json::Value> = Vec::new();
                            for script in &scripts {
                                if let Ok(matches) = js_decode::extract_nextjs_rsc(script, json_key)
                                {
                                    all_results.extend(matches);
                                }
                            }
                            if all_results.is_empty() {
                                None
                            } else {
                                serde_json::to_string(&all_results).ok()
                            }
                        }
                        Err(_) => None,
                    }
                } else {
                    // Mode 2: Extract JS variable - always return array
                    match js_decode::extract_js_variable(&script_content, var_pattern) {
                        Ok(js_value) => {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(
                                &js_value.to_json_string(),
                            ) {
                                serde_json::to_string(&vec![parsed]).ok()
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                }
            } else {
                // Mode 1: Direct JSON (for ld+json scripts) - always return array
                match extract_all_text(&html, &selector) {
                    Ok(scripts) if scripts.is_empty() => None,
                    Ok(scripts) => {
                        let json_values: Vec<serde_json::Value> = scripts
                            .iter()
                            .filter_map(|s| {
                                parse_and_decode_json(s)
                                    .and_then(|json_str| serde_json::from_str(&json_str).ok())
                            })
                            .collect();

                        if json_values.is_empty() {
                            None
                        } else {
                            serde_json::to_string(&json_values).ok()
                        }
                    }
                    Err(_) => None,
                }
            };

            match result {
                Some(json_str) => {
                    output_vector.insert(i, json_str.as_str());
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
            // html_extract_json(html VARCHAR, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_extract_json(html BLOB, selector)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_extract_json(html VARCHAR, selector, var_pattern)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
            // html_extract_json(html BLOB, selector, var_pattern)
            ScalarFunctionSignature::exact(
                vec![
                    LogicalTypeHandle::from(LogicalTypeId::Blob),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                    LogicalTypeHandle::from(LogicalTypeId::Varchar),
                ],
                LogicalTypeHandle::from(LogicalTypeId::Varchar),
            ),
        ]
    }
}

/// # Safety
/// Called by DuckDB to initialize the extension. Must only be called once.
#[duckdb_entrypoint_c_api()]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<HtmlQueryFunction>("html_query")?;
    con.register_scalar_function::<HtmlQueryAllFunction>("html_query_all")?;
    con.register_scalar_function::<HtmlExtractJsonFunction>("html_extract_json")?;
    Ok(())
}
