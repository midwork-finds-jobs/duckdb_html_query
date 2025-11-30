# Extracting JavaScript JSON in DuckDB

The `hq` DuckDB extension includes `html_extract_json()` which extracts JSON from HTML, handling both LD+JSON and JavaScript variables with encoded strings.

## The Problem

Websites embed JSON data in different ways:

**LD+JSON with HTML entities:**
```html
<script type="application/ld+json">
{"title": "Test &amp; Demo", "desc": "&lt;p&gt;Hello&lt;/p&gt;"}
</script>
```

**JavaScript with hex/unicode escapes:**
```javascript
var jobs = JSON.parse('[{\x22Salary\x22:\x2250000$ \- 80000$\x22}]');
```

## The Solution

Use `html_extract_json()` to extract and decode JSON in one function call.

### Function Signatures

```sql
-- Extract LD+JSON (decodes HTML entities)
html_extract_json(html VARCHAR, selector VARCHAR) -> VARCHAR

-- Extract JS variable (decodes hex/unicode escapes)
html_extract_json(html VARCHAR, selector VARCHAR, var_pattern VARCHAR) -> VARCHAR
```

## Examples

### LD+JSON Extraction

```sql
SELECT html_extract_json('
  <html>
  <script type="application/ld+json">
  {"title": "Test &amp; Demo", "price": "29.99"}
  </script>
  </html>
', 'script[type="application/ld+json"]');
-- Returns: {"title":"Test & Demo","price":"29.99"}
```

### JavaScript Variable Extraction

```sql
SELECT html_extract_json('
  <html>
  <script>
  var config = {"debug": true, "version": "1.0"};
  </script>
  </html>
', 'script', 'var config');
-- Returns: {"debug":true,"version":"1.0"}
```

### JSON.parse with Hex Escapes

```sql
-- Handles Zoho-style encoded JSON
SELECT html_extract_json('
  <html>
  <script>
  var jobs = JSON.parse(''[{\\x22Salary\\x22:\\x2250000$\\x22}]'');
  </script>
  </html>
', 'script', 'var jobs');
-- Returns: [{"Salary":"50000$"}]
```

### Query JSON Fields

```sql
SELECT
  json_extract_string(
    html_extract_json(html, 'script[type="application/ld+json"]'),
    '$.title'
  ) as title,
  json_extract_string(
    html_extract_json(html, 'script[type="application/ld+json"]'),
    '$.price'
  ) as price
FROM pages;
```

## Encoding Support

**LD+JSON mode** decodes:
- `&lt;` `&gt;` `&amp;` `&quot;` `&#39;` and other HTML entities

**JS variable mode** decodes:
- `\xNN` hex escapes → characters
- `\uNNNN` unicode escapes → unicode characters
- `\\uNNNN` double-escaped unicode → unicode characters
- `\-` and `\/` invalid escapes → `-` and `/`
- Standard escapes (`\n`, `\r`, `\t`, etc.)

## Real-World Example

Extract job listings from career pages:

```sql
-- Teamtailor (LD+JSON)
SELECT json_extract_string(
  html_extract_json(html, 'script[type="application/ld+json"]'),
  '$.title'
) as job_title
FROM read_text('teamtailor-career.html');

-- Zoho (JavaScript variable)
SELECT json_extract_string(
  html_extract_json(html, 'script', 'var jobs'),
  '$[0].Posting_Title'
) as job_title
FROM read_text('zoho-career.html');
```

## Performance

Implemented in Rust, runs natively in DuckDB:

- No external processes
- Vectorized column processing
- Built-in NULL handling

## See Also

- [duckdb-extension/README.md](duckdb-extension/README.md) - Full DuckDB extension docs
- [ENCODING-ISSUES.md](ENCODING-ISSUES.md) - Encoding issues explanation
