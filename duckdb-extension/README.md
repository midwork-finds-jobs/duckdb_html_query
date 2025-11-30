# HQ DuckDB Extension

DuckDB extension for querying HTML using CSS selectors.

## Building

```sh
make configure
make debug    # or make release
```

## Installation

```sql
LOAD './build/debug/hq.duckdb_extension';
```

## Functions

### html_query()

Extract HTML content using CSS selectors.

```sql
-- Extract title element
SELECT html_query('<html><title>My Page</title></html>', 'title');
-- Returns: <title>My Page</title>

-- Extract text only
SELECT html_query('<html><body><p>Hello World</p></body></html>', 'p', true);
-- Returns: Hello World

-- CSS pseudo-selectors
SELECT html_query('<div><p>First</p><p>Second</p></div>', 'p:last-child', true);
-- Returns: Second

SELECT html_query('<ul><li>A</li><li>B</li><li>C</li></ul>', 'li:nth-child(2)', true);
-- Returns: B

-- Use with tables
SELECT html_query(html, 'title', true) as title FROM pages;
```

**Parameters:**
- `html` (VARCHAR): HTML content
- `selector` (VARCHAR, optional): CSS selector (default: `:root`)
- `text_only` (BOOLEAN, optional): Extract text only (default: false)

**Returns:** VARCHAR or NULL

### html_extract_json()

Extract JSON from HTML scripts. Handles LD+JSON and JavaScript variables.

```sql
-- Extract LD+JSON (decodes HTML entities automatically)
SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;

-- Extract JS variable from script (handles JSON.parse with hex escapes)
SELECT html_extract_json(html, 'script', 'var jobs') FROM pages;

-- Access JSON fields
SELECT json_extract_string(
  html_extract_json(html, 'script[type="application/ld+json"]'),
  '$.title'
) FROM pages;

-- Extract from Zoho-style JSON.parse with hex escapes
-- var jobs = JSON.parse('[{\x22Salary\x22:\x2250000$\x22}]');
SELECT json_extract_string(
  html_extract_json(html, 'script', 'var jobs'),
  '$[0].Salary'
) FROM pages;
```

**Parameters:**
- `html` (VARCHAR): HTML content
- `selector` (VARCHAR): CSS selector for script element
- `var_pattern` (VARCHAR, optional): JS variable pattern (e.g., `'var jobs'`, `'const config'`)

**Returns:** VARCHAR (JSON string) or NULL

**Handles:**
- LD+JSON with HTML entities (`&lt;` `&gt;` `&amp;` `&quot;`)
- `JSON.parse('...')` with hex escapes (`\x22` → `"`)
- Unicode escapes (`\u00e9` → `é`)
- Plain JSON objects/arrays

## CSS Selectors

Supports standard CSS selectors:

- Tag: `div`, `p`, `a`
- Class: `.classname`
- ID: `#idname`
- Attribute: `[href]`, `[href="/page"]`, `[type="application/ld+json"]`
- Combinators: `div > p`, `div p`, `div + p`
- Pseudo-classes: `:first-child`, `:last-child`, `:nth-child(n)`

## Examples

### LD+JSON Extraction

```sql
SELECT
  json_extract_string(
    html_extract_json(html, 'script[type="application/ld+json"]'),
    '$.name'
  ) as name,
  json_extract_string(
    html_extract_json(html, 'script[type="application/ld+json"]'),
    '$.price'
  ) as price
FROM pages;
```

### JavaScript Variable Extraction

```sql
-- Extract jobs array from Zoho career pages
SELECT
  json_extract_string(
    html_extract_json(html, 'script', 'var jobs'),
    '$[0].Posting_Title'
  ) as title
FROM pages;
```

## Requirements

- DuckDB v1.4.2+
- Rust toolchain (for building)
