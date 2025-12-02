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
| `html_query(html, selector?, extract?)` | VARCHAR | First matching element |
| `html_query_all(html, selector?, extract?)` | VARCHAR[] | All matching elements as list |
| `html_extract_json(html, selector, var_pattern?)` | JSON array | JSON from script tags |

### Extract Parameter

The `extract` parameter specifies what to extract from matched elements:

| Value | Description |
|-------|-------------|
| (omitted) | Full HTML of element |
| `@text` or `text` | Inner text content |
| `@href`, `href` | href attribute |
| `@src`, `src` | src attribute |
| `data-test-id` | Any attribute name |

## Usage

### html_query - First matching element

```sql
-- Get page title text
SELECT html_query(html, 'title', '@text') FROM pages;
-- Returns: "My Page Title"

-- Get link href
SELECT html_query(html, 'a.nav-link', '@href') FROM pages;
-- Returns: "/about"

-- Get data attribute
SELECT html_query(html, 'button', 'data-test-id') FROM pages;
-- Returns: "submit-btn"

-- Get HTML element (no extract param)
SELECT html_query(html, 'div.content') FROM pages;
-- Returns: "<div class=\"content\">...</div>"
```

### html_query_all - All matching elements

```sql
-- Get all link hrefs
SELECT html_query_all(html, 'a', '@href') FROM pages;
-- Returns: [/home, /about, /contact]

-- Get all paragraph texts
SELECT html_query_all(html, 'p', '@text') FROM pages;
-- Returns: [First paragraph, Second paragraph]

-- Access specific element (1-indexed)
SELECT list_extract(html_query_all(html, 'a', '@href'), 2) FROM pages;
-- Returns: "/about"

-- Count links
SELECT len(html_query_all(html, 'a', '@href')) FROM pages;
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
