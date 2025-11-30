-- Test JavaScript JSON decoding
-- This demonstrates extracting JSON from JavaScript variables in HTML

LOAD 'build/debug/hq.duckdb_extension';

-- Example HTML with JavaScript JSON
CREATE OR REPLACE TABLE test_html AS SELECT
    '<html><head><script>
    var jobs = JSON.parse(''[{\\x22Salary\\x22:\\x2250000$ \\- 80000$\\x22,\\x22Remote_Job\\x22:false,\\x22Langue\\x22:[\\x22Français\\x22,\\x22English\\x22],\\x22Posting_Title\\x22:\\x22Intégrateur Zoho \\\\u2013 Profil\\x22,\\x22City\\x22:\\x22Terrebonne\\x22}]'');
    </script></head></html>' AS html;

-- Step 1: Extract script content with hq
SELECT hq(html, 'script', true) AS script_text
FROM test_html;

-- Step 2: Extract the JSON.parse argument using regexp
WITH scripts AS (
    SELECT hq(html, 'script', true) AS script_text
    FROM test_html
)
SELECT regexp_extract(script_text, 'JSON\.parse\(''(.*)''', 1) AS js_string
FROM scripts
WHERE script_text LIKE '%JSON.parse%';

-- Step 3: Decode the JavaScript string
WITH scripts AS (
    SELECT hq(html, 'script', true) AS script_text
    FROM test_html
),
js_strings AS (
    SELECT regexp_extract(script_text, 'JSON\.parse\(''(.*)''', 1) AS js_string
    FROM scripts
    WHERE script_text LIKE '%JSON.parse%'
)
SELECT hq_decode_js_string(js_string) AS decoded_json
FROM js_strings;

-- Step 4: Parse as JSON and query
WITH scripts AS (
    SELECT hq(html, 'script', true) AS script_text
    FROM test_html
),
js_strings AS (
    SELECT regexp_extract(script_text, 'JSON\.parse\(''(.*)''', 1) AS js_string
    FROM scripts
    WHERE script_text LIKE '%JSON.parse%'
),
decoded AS (
    SELECT hq_decode_js_string(js_string) AS json_text
    FROM js_strings
)
SELECT
    json_extract(jobs, '$.Posting_Title') AS title,
    json_extract(jobs, '$.City') AS city,
    json_extract(jobs, '$.Salary') AS salary,
    json_extract_string(jobs, '$.Langue') AS languages
FROM decoded, LATERAL (
    SELECT unnest(json_extract_path(json_text, '$')) AS jobs
);

-- Cleanup
DROP TABLE test_html;
