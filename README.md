# html_query - DuckDB HTML Extension

DuckDB extension for querying HTML using CSS selectors.

## Installation

```sql
INSTALL html_query FROM community;
LOAD html_query;
```

## Functions

| Function | Returns | Description |
|----------|---------|-------------|
| `html_query(html, selector?, text_only?)` | VARCHAR | First matching element |
| `html_query_all(html, selector?, text_only?)` | VARCHAR[] | All matching elements as list |
| `html_extract_json(html, selector, var_pattern?)` | JSON array | JSON from script tags |

## Usage

### html_query - First matching element

```sql
-- Get page title
SELECT html_query(html, 'title', true) FROM pages;
-- Returns: "My Page Title"

-- Get HTML element
SELECT html_query(html, 'div.content') FROM pages;
-- Returns: "<div class=\"content\">...</div>"

-- Returns NULL if no match
SELECT html_query(html, '.nonexistent', true) FROM pages;
-- Returns: NULL
```

### html_query_all - All matching elements

```sql
-- Get all paragraphs as native list
SELECT html_query_all(html, 'p', true) FROM pages;
-- Returns: [First paragraph, Second paragraph]

-- Access specific element (1-indexed)
SELECT list_extract(html_query_all(html, 'p', true), 2) FROM pages;
-- Returns: "Second paragraph"

-- Use with list functions
SELECT len(html_query_all(html, 'a', true)) FROM pages;
-- Returns: 5
```

### html_extract_json - Extract JSON from scripts

```sql
-- Extract LD+JSON (returns array, decodes HTML entities)
SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;
-- Returns: [{"@type":"Product","name":"Widget"}]

-- Access first JSON object
SELECT html_extract_json(html, 'script[type="application/ld+json"]')->0->>'name' FROM pages;

-- Extract JS variables (handles hex escapes like \x22)
SELECT html_extract_json(html, 'script', 'var config') FROM pages;
-- Returns: [{"debug":true}]
```

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
make release
```

## License

MIT
