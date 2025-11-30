# HQ DuckDB Extension

DuckDB extension for querying HTML using CSS selectors, similar to jq for JSON.

## Building

```sh
make configure
make debug    # or make release
```

## Installation

### From Local Build

```sql
LOAD './build/debug/hq.duckdb_extension';
```

## Usage

### hq() function

Extract HTML content using CSS selectors.

```sql
-- Basic usage - extract title
SELECT hq('<html><head><title>My Page</title></head></html>', 'title');
-- Returns: <title>My Page</title>

-- Extract text only
SELECT hq('<html><body><p>Hello World</p></body></html>', 'p', true);
-- Returns: Hello World

-- Pretty print
SELECT hq('<html><body><div>Test</div></body></html>', 'body', false, true);

-- Use with tables
CREATE TABLE pages (html VARCHAR);
INSERT INTO pages VALUES
  ('<html><head><title>Page 1</title></head></html>'),
  ('<html><head><title>Page 2</title></head></html>');

SELECT hq(html, 'title', true) as title FROM pages;
-- Returns:
-- Page 1
-- Page 2
```

**Parameters:**
- `html` (VARCHAR): HTML content to parse
- `selector` (VARCHAR, optional): CSS selector (default: `:root`)
- `text_only` (BOOLEAN, optional): Extract text content only (default: false)
- `pretty` (BOOLEAN, optional): Pretty print output (default: false)

**Returns:** VARCHAR or NULL

**Note:** When `text_only` is true, HTML entities are automatically decoded (e.g., `&lt;` → `<`) and JSON is validated/compacted. This is especially useful for extracting LD+JSON from web pages.

### hq_attr() function

Extract attributes from HTML elements.

```sql
-- Extract href attributes from links
SELECT hq_attr(
  '<html><body><a href="/page1">Link 1</a><a href="/page2">Link 2</a></body></html>',
  'href',
  'a'
);
-- Returns: [/page1, /page2]

-- Extract src from images
SELECT hq_attr(html, 'src', 'img') FROM pages;

-- Get class attributes
SELECT hq_attr('<div class="header">Test</div>', 'class', 'div');
-- Returns: [header]
```

**Parameters:**
- `html` (VARCHAR): HTML content to parse
- `attribute` (VARCHAR): Attribute name to extract (e.g., 'href', 'src', 'class')
- `selector` (VARCHAR, optional): CSS selector (default: `:root`)

**Returns:** VARCHAR[] (array of strings) or NULL

### hq_decode_js_string() function

Decode JavaScript string literals with hex/unicode escapes and invalid escapes.

```sql
-- Decode JavaScript hex escapes
SELECT hq_decode_js_string('[{\x22name\x22:\x22value\x22}]');
-- Returns: [{"name":"value"}]

-- Decode unicode escapes
SELECT hq_decode_js_string('Title\\u2013Profil');
-- Returns: Title–Profil

-- Extract and decode JSON from JavaScript
SELECT hq_decode_js_string(
  regexp_extract(hq(html, 'script', true), 'JSON\.parse\(''(.*)''', 1)
) FROM pages;
```

**Parameters:**
- `js_string` (VARCHAR): JavaScript string literal to decode

**Returns:** VARCHAR or NULL

**Handles:**
- `\xNN` hex escapes → characters
- `\uNNNN` unicode escapes → unicode characters
- `\\uNNNN` double-escaped unicode → unicode characters
- `\-` and `\/` invalid JSON escapes → `-` and `/`

**See:** [JAVASCRIPT-JSON-DUCKDB.md](../JAVASCRIPT-JSON-DUCKDB.md) for complete examples

## Examples

### Web Scraping with HTTP Extension

```sql
INSTALL httpfs;
LOAD httpfs;
LOAD './build/debug/hq.duckdb_extension';

-- Extract titles from web pages
SELECT
  url,
  hq(content, 'title', true) as title
FROM read_text([
  'https://example.com'
]) as (url, content);

-- Get all links from a page
SELECT unnest(hq_attr(content, 'href', 'a')) as link
FROM read_text(['https://example.com']);
```

### HTML Analysis

```sql
-- Count paragraphs
SELECT
  url,
  length(hq(html, 'p')) - length(replace(hq(html, 'p'), '<p', '')) as paragraph_count
FROM web_pages;

-- Extract metadata
SELECT
  hq(html, 'meta[property="og:title"]') as og_title,
  hq_attr(html, 'content', 'meta[property="og:description"]')[1] as og_description
FROM pages;
```

### LD+JSON Structured Data

Extract and parse JSON-LD structured data commonly found in web pages:

```sql
-- Extract LD+JSON from HTML
SELECT hq(html, 'script[type="application/ld+json"]', true) as ld_json
FROM pages;

-- Parse LD+JSON and extract fields
WITH ld_data AS (
  SELECT hq(html, 'script[type="application/ld+json"]', true) as json_text
  FROM pages
)
SELECT
  json_extract(trim(json_text), '$.@type') as schema_type,
  json_extract(trim(json_text), '$.name') as name,
  json_extract(trim(json_text), '$.price') as price
FROM ld_data;

-- Practical example: Extract product information
CREATE TABLE products AS
SELECT '
<html>
<head>
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Widget",
    "price": "29.99",
    "offers": {
      "@type": "Offer",
      "availability": "InStock"
    }
  }
  </script>
</head>
</html>' as html;

SELECT
  json_extract(trim(hq(html, 'script[type="application/ld+json"]', true)), '$.name') as product_name,
  json_extract(trim(hq(html, 'script[type="application/ld+json"]', true)), '$.price') as price,
  json_extract(trim(hq(html, 'script[type="application/ld+json"]', true)), '$.offers.availability') as availability
FROM products;
```

## CSS Selectors

Supports all standard CSS selectors:

- Tag selectors: `div`, `p`, `a`
- Class selectors: `.classname`
- ID selectors: `#idname`
- Attribute selectors: `[href]`, `[href="/page"]`
- Combinators: `div > p`, `div p`, `div + p`
- Pseudo-classes: `:first-child`, `:last-child`
- Complex selectors: `div.class#id[attr="value"]`

## Development

### Testing

```sh
# Run with DuckDB (note: -unsigned required for local builds)
duckdb -unsigned < test_extension.sql

# Or use make test (when implemented)
make test
```

### Structure

- `src/lib.rs` - Extension implementation
- `Cargo.toml` - Rust dependencies
- `Makefile` - Build configuration
- `test/` - SQL test files

## Requirements

- Rust toolchain
- DuckDB v1.4.2+
- Python 3 (for build scripts)

## License

Same as parent project (hq).
