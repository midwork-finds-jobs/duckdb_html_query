# hq - HTML Query

DuckDB extension and CLI for querying HTML using CSS selectors.

## DuckDB Extension (Primary)

Query HTML directly in DuckDB:

```sql
INSTALL html_query FROM community;
LOAD html_query;

-- Extract text with CSS selector
SELECT html_query('<html><title>Test</title></html>', 'title', true) as title;
-- Returns: Test

-- Extract JSON from LD+JSON
SELECT html_extract_json(html, 'script[type="application/ld+json"]') FROM pages;

-- Extract JS variables (handles JSON.parse with hex escapes)
SELECT html_extract_json(html, 'script', 'var jobs') FROM pages;
```

### Functions

- `html_query(html, selector?, text_only?)` - Extract HTML/text using CSS selectors
- `html_extract_json(html, selector, var_pattern?)` - Extract JSON from scripts

### Building

```sh
make configure
make release    # builds build/release/html_query.duckdb_extension
```

### CSS Selectors

- Tag: `div`, `p`, `a`
- Class: `.classname`
- ID: `#idname`
- Attribute: `[href]`, `[type="application/ld+json"]`
- Pseudo: `:first-child`, `:last-child`, `:nth-child(n)`
- Combinators: `div > p`, `div p`

## CLI

```sh
# Build
make cli

# Usage
curl -s https://example.com | ./target/release/hq 'title'
curl -s https://example.com | ./target/release/hq --text 'p'
curl -s https://example.com | ./target/release/hq --attribute href 'a'
```

## Installation

### From Source

```sh
cargo install --git https://github.com/midwork-finds-jobs/duckdb_html_query --features cli
```

## License

MIT
