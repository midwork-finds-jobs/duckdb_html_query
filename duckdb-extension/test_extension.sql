-- Load the hq extension
LOAD './build/debug/hq.duckdb_extension';

-- Test basic HTML parsing
SELECT 'Test 1: Basic HTML parsing' as test;
SELECT hq('<html><head><title>Test Page</title></head><body><p>Hello World</p></body></html>', 'title') as result;

-- Test text extraction
SELECT 'Test 2: Text extraction' as test;
SELECT hq('<html><body><p>Hello World</p></body></html>', 'p', true) as result;

-- Test with table
SELECT 'Test 3: With table' as test;
CREATE TABLE pages AS
SELECT '<html><head><title>Page 1</title></head></html>' as html
UNION ALL SELECT '<html><head><title>Page 2</title></head></html>' as html;

SELECT hq(html, 'title', true) as page_title FROM pages ORDER BY 1;

-- Test attribute extraction
SELECT 'Test 4: Attribute extraction' as test;
SELECT hq_attr('<html><body><a href="/page1">Link 1</a><a href="/page2">Link 2</a></body></html>', 'href', 'a') as links;

-- Test LD+JSON extraction
SELECT 'Test 5: LD+JSON extraction' as test;
SELECT hq('
<html>
<head>
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Example Product",
    "price": "29.99",
    "currency": "USD"
  }
  </script>
</head>
</html>', 'script[type="application/ld+json"]', true) as ld_json;

-- Test LD+JSON parsing with JSON functions
SELECT 'Test 6: Parse LD+JSON as JSON' as test;
WITH ld_data AS (
  SELECT hq('
    <html>
    <head>
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Product",
        "name": "Example Product",
        "price": "29.99",
        "currency": "USD",
        "offers": {
          "@type": "Offer",
          "availability": "InStock"
        }
      }
      </script>
    </head>
    </html>', 'script[type="application/ld+json"]', true) as json_text
)
SELECT
  json_extract(trim(json_text), '$.name') as product_name,
  json_extract(trim(json_text), '$.price') as price,
  json_extract(trim(json_text), '$.offers.availability') as availability
FROM ld_data;

-- Test multiple LD+JSON blocks
SELECT 'Test 7: Multiple LD+JSON blocks' as test;
CREATE TABLE products AS
SELECT '
<html>
<head>
  <script type="application/ld+json">
  {"@type": "Product", "name": "Widget A", "price": "19.99"}
  </script>
</head>
</html>' as html
UNION ALL
SELECT '
<html>
<head>
  <script type="application/ld+json">
  {"@type": "Product", "name": "Widget B", "price": "24.99"}
  </script>
</head>
</html>' as html;

SELECT
  json_extract(trim(hq(html, 'script[type="application/ld+json"]', true)), '$.name') as product_name,
  json_extract(trim(hq(html, 'script[type="application/ld+json"]', true)), '$.price') as price
FROM products
ORDER BY product_name;
