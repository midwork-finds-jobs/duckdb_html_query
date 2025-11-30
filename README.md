# html_query - DuckDB HTML Extension

DuckDB extension for querying HTML using CSS selectors. Both functions return JSON arrays for consistent handling.

## Installation

```sql
INSTALL html_query FROM community;
LOAD html_query;
```

## Usage

### html_query - Extract HTML elements

Returns a JSON array of matching elements.

```sql
-- Extract all paragraphs (returns JSON array)
SELECT html_query(html, 'p', true) FROM pages;
-- Returns: ["First paragraph", "Second paragraph"]

-- Access first element with ->>
SELECT html_query(html, 'title', true)->>0 as title FROM pages;
-- Returns: "My Page Title"

-- Get HTML (not just text)
SELECT html_query(html, 'div.content') FROM pages;
-- Returns: ["<div class=\"content\">...</div>"]
```

### html_extract_json - Extract JSON from scripts

Returns a JSON array of parsed JSON objects.

```sql
-- Extract LD+JSON (returns array, decodes HTML entities)
SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;
-- Returns: [{"@type":"Product","name":"Widget"}]

-- Multiple LD+JSON scripts return array with all objects
SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;
-- Returns: [{"@type":"Product",...}, {"@type":"Organization",...}]

-- Access first JSON object
SELECT html_extract_json(html, 'script[type="application/ld+json"]')->0->>'name' FROM pages;

-- Extract JS variables (handles hex escapes like \x22)
SELECT html_extract_json(html, 'script', 'var config') FROM pages;
-- Returns: [{"debug":true}]
```

## Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `html_query(html, selector?, text_only?)` | JSON array of strings | Extract HTML/text using CSS selectors |
| `html_extract_json(html, selector, var_pattern?)` | JSON array of objects | Extract JSON from script tags |

## CSS Selectors

- Tag: `div`, `p`, `a`
- Class: `.classname`
- ID: `#idname`
- Attribute: `[href]`, `[type="application/ld+json"]`
- Pseudo: `:first-child`, `:last-child`, `:nth-child(n)`
- Combinators: `div > p`, `div p`

## Building

```sh
make configure
make release    # builds build/release/html_query.duckdb_extension
```

## License

MIT
