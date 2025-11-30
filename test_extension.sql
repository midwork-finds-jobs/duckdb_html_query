-- Load the hq extension
LOAD './build/release/hq.duckdb_extension';

-- Test 1: Basic HTML parsing
SELECT 'Test 1: Basic HTML parsing' as test;
SELECT html_query('<html><head><title>Test Page</title></head></html>', 'title') as result;

-- Test 2: Text extraction
SELECT 'Test 2: Text extraction' as test;
SELECT html_query('<html><body><p>Hello World</p></body></html>', 'p', true) as result;

-- Test 3: CSS pseudo-selectors
SELECT 'Test 3: CSS pseudo-selectors' as test;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:last-child', true) as last_p;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:nth-child(2)', true) as second_p;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:first-child', true) as first_p;

-- Test 4: With table
SELECT 'Test 4: With table' as test;
CREATE TABLE pages AS
SELECT '<html><head><title>Page 1</title></head></html>' as html
UNION ALL SELECT '<html><head><title>Page 2</title></head></html>' as html;

SELECT html_query(html, 'title', true) as page_title FROM pages ORDER BY 1;

-- Test 5: LD+JSON extraction with html_extract_json
SELECT 'Test 5: LD+JSON extraction' as test;
SELECT html_extract_json('
<html>
<head>
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Example Product",
    "price": "29.99"
  }
  </script>
</head>
</html>', 'script[type="application/ld+json"]') as ld_json;

-- Test 6: Parse LD+JSON with JSON functions
SELECT 'Test 6: Parse LD+JSON' as test;
SELECT
  json_extract_string(html_extract_json('
    <html>
    <head>
      <script type="application/ld+json">
      {"@type": "Product", "name": "Widget", "price": "19.99"}
      </script>
    </head>
    </html>', 'script[type="application/ld+json"]'), '$.name') as product_name,
  json_extract_string(html_extract_json('
    <html>
    <head>
      <script type="application/ld+json">
      {"@type": "Product", "name": "Widget", "price": "19.99"}
      </script>
    </head>
    </html>', 'script[type="application/ld+json"]'), '$.price') as price;

-- Test 7: JS variable extraction with html_extract_json
SELECT 'Test 7: JS variable extraction' as test;
SELECT html_extract_json('
<html>
<script>
var config = {"debug": true, "version": "1.0"};
</script>
</html>', 'script', 'var config') as config_json;

-- Test 8: HTML entity decoding in LD+JSON
SELECT 'Test 8: HTML entity decoding' as test;
SELECT json_extract_string(
  html_extract_json('
    <html>
    <script type="application/ld+json">
    {"title": "Test &amp; Demo", "desc": "&lt;p&gt;Hello&lt;/p&gt;"}
    </script>
    </html>', 'script[type="application/ld+json"]'),
  '$.title'
) as title;

-- Cleanup
DROP TABLE pages;
