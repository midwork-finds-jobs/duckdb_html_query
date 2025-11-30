-- Load the html_query extension
LOAD './build/release/html_query.duckdb_extension';

-- Test 1: html_query returns first match
SELECT 'Test 1: html_query returns first match' as test;
SELECT html_query('<html><head><title>Test Page</title></head></html>', 'title') as result;

-- Test 2: Text extraction returns first match
SELECT 'Test 2: Text extraction' as test;
SELECT html_query('<html><body><p>Hello World</p></body></html>', 'p', true) as result;

-- Test 3: html_query returns first of multiple
SELECT 'Test 3: First of multiple' as test;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p', true) as result;

-- Test 4: html_query_all returns JSON array
SELECT 'Test 4: html_query_all returns array' as test;
SELECT html_query_all('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p', true) as result;

-- Test 5: Access array elements from html_query_all
SELECT 'Test 5: Access array elements' as test;
SELECT
  html_query_all('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p', true)->>0 as first_p,
  html_query_all('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p', true)->>1 as second_p,
  html_query_all('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p', true)->>2 as third_p;

-- Test 6: CSS pseudo-selectors
SELECT 'Test 6: CSS pseudo-selectors' as test;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:last-child', true) as last_p;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:nth-child(2)', true) as second_p;
SELECT html_query('<div><p>First</p><p>Second</p><p>Third</p></div>', 'p:first-child', true) as first_p;

-- Test 7: With table
SELECT 'Test 7: With table' as test;
CREATE TABLE pages AS
SELECT '<html><head><title>Page 1</title></head></html>' as html
UNION ALL SELECT '<html><head><title>Page 2</title></head></html>' as html;

SELECT html_query(html, 'title', true) as page_title FROM pages ORDER BY 1;

-- Test 8: LD+JSON extraction (returns JSON array)
SELECT 'Test 8: LD+JSON extraction' as test;
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

-- Test 9: Parse LD+JSON array element
SELECT 'Test 9: Parse LD+JSON' as test;
SELECT
  json_extract_string(html_extract_json('
    <html>
    <head>
      <script type="application/ld+json">
      {"@type": "Product", "name": "Widget", "price": "19.99"}
      </script>
    </head>
    </html>', 'script[type="application/ld+json"]')->0, '$.name') as product_name,
  json_extract_string(html_extract_json('
    <html>
    <head>
      <script type="application/ld+json">
      {"@type": "Product", "name": "Widget", "price": "19.99"}
      </script>
    </head>
    </html>', 'script[type="application/ld+json"]')->0, '$.price') as price;

-- Test 10: JS variable extraction (returns JSON array)
SELECT 'Test 10: JS variable extraction' as test;
SELECT html_extract_json('
<html>
<script>
var config = {"debug": true, "version": "1.0"};
</script>
</html>', 'script', 'var config') as config_json;

-- Test 11: HTML entity decoding in LD+JSON
SELECT 'Test 11: HTML entity decoding' as test;
SELECT json_extract_string(
  html_extract_json('
    <html>
    <script type="application/ld+json">
    {"title": "Test &amp; Demo", "desc": "&lt;p&gt;Hello&lt;/p&gt;"}
    </script>
    </html>', 'script[type="application/ld+json"]')->0,
  '$.title'
) as title;

-- Test 12: Multiple LD+JSON scripts return JSON array
SELECT 'Test 12: Multiple LD+JSON scripts' as test;
SELECT html_extract_json('
<html>
<head>
  <script type="application/ld+json">
  {"@type": "Product", "name": "Widget A"}
  </script>
  <script type="application/ld+json">
  {"@type": "Organization", "name": "Acme Corp"}
  </script>
</head>
</html>', 'script[type="application/ld+json"]') as ld_json_array;

-- Test 13: Access multiple LD+JSON elements
SELECT 'Test 13: Access array elements' as test;
SELECT
  json_extract_string(ld_json_array->0, '$.name') as first_name,
  json_extract_string(ld_json_array->1, '$.name') as second_name
FROM (
  SELECT html_extract_json('
    <html>
    <head>
      <script type="application/ld+json">{"@type": "Product", "name": "Widget"}</script>
      <script type="application/ld+json">{"@type": "Organization", "name": "Acme"}</script>
    </head>
    </html>', 'script[type="application/ld+json"]') as ld_json_array
);

-- Cleanup
DROP TABLE pages;
