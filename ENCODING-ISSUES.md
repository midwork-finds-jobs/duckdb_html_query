# Extracting JavaScript JSON with Multiple Encoding Layers

## Problem Statement

Some websites embed JSON data inside JavaScript code with multiple layers of encoding:

```javascript
var jobs = JSON.parse('[{\x22Salary\x22:\x2250000$ \- 80000$\x22,\x22Langue\x22:[\x22Français\x22]}]');
```

This creates several challenges:
1. **JavaScript hex escapes**: `\x22` (quote), `\x2d` (dash)
2. **Unicode escapes**: `\u2013` (EN DASH), `\\u2013` (double-escaped)
3. **Invalid escapes**: `\-` (not valid in JSON)
4. **UTF-8 mojibake**: Text like `Français` stored as `Ã©` → needs re-encoding

Standard tools like `jq` cannot parse this directly because they expect valid JSON, not JavaScript string literals.

## The Solution

### Step 1: Extract with hq

Use `hq` to extract the script content:

```bash
cat ~/zoho-career.html | hq -t 'script' | \
  grep "var jobs = JSON.parse(" | \
  sed -E "s/.*JSON\.parse\('(.*)'\).*/\1/"
```

**What this does:**
- `hq -t 'script'` - Extract text content from all `<script>` tags (no HTML)
- `grep "var jobs = JSON.parse("` - Find the line with the variable
- `sed -E "s/.*JSON\.parse\('(.*)'\).*/\1/"` - Extract just the string inside `JSON.parse('...')`

### Step 2: Decode with Python

Create a Python script to handle all encoding layers:

```python
#!/usr/bin/env python3
import codecs
import json
import re
import sys

def decode_zoho_json(js_string):
    """Decode JavaScript string with hex/unicode escapes and fix UTF-8 encoding"""

    # Step 1: Decode hex escapes (\xNN -> character)
    # Example: \x22 -> "
    def decode_hex_escapes(s):
        return re.sub(r'\\x([0-9a-fA-F]{2})',
                      lambda m: chr(int(m.group(1), 16)), s)

    # Step 2: Process various escape sequences
    decoded = decode_hex_escapes(js_string)

    # Fix double-escaped Unicode: \\u2013 -> \u2013
    decoded = decoded.replace('\\\\u', '\\u')

    # Remove invalid JSON escapes
    decoded = decoded.replace('\\-', '-')   # \- is invalid
    decoded = decoded.replace('\\/', '/')   # \/ -> /

    # Step 3: Decode Unicode escapes (\uNNNN -> character)
    # Example: \u2013 -> –
    decoded = codecs.decode(decoded, 'unicode_escape')

    # Step 4: Parse as JSON
    data = json.loads(decoded)

    # Step 5: Fix UTF-8 encoding issues (mojibake)
    # Some text was encoded as UTF-8 but interpreted as Latin-1
    # Example: "Ã©" should be "é"
    def fix_encoding(obj):
        if isinstance(obj, str):
            try:
                # Re-encode as latin-1, then decode as UTF-8
                return obj.encode('latin-1').decode('utf-8')
            except:
                return obj
        elif isinstance(obj, dict):
            return {k: fix_encoding(v) for k, v in obj.items()}
        elif isinstance(obj, list):
            return [fix_encoding(item) for item in obj]
        return obj

    return fix_encoding(data)

if __name__ == '__main__':
    # Read JavaScript string literal from stdin
    js_string = sys.stdin.read().strip()

    try:
        data = decode_zoho_json(js_string)

        # Output clean JSON to stdout
        print(json.dumps(data, indent=2, ensure_ascii=False))

        sys.stderr.write(f"✓ Extracted {len(data)} job entries\n")

    except Exception as e:
        sys.stderr.write(f"Error: {e}\n")
        sys.exit(1)
```

### Step 3: Complete Pipeline

Save the script and use it:

```bash
# Save the Python script
cat > extract-zoho-jobs.py << 'EOF'
[... script from above ...]
EOF
chmod +x extract-zoho-jobs.py

# Extract JSON
cat ~/zoho-career.html | hq -t 'script' | \
  grep "var jobs = JSON.parse(" | \
  sed -E "s/.*JSON\.parse\('(.*)'\).*/\1/" | \
  ./extract-zoho-jobs.py > zoho-jobs.json

# Query the data
jq '.[] | {title: .Posting_Title, city: .City, salary: .Salary}' zoho-jobs.json
```

## How It Works

### Encoding Transformation Chain

```
Original JavaScript:
var jobs = JSON.parse('[{\x22Salary\x22:\x2250000$\x22}]');

↓ After grep + sed:
[{\x22Salary\x22:\x2250000$\x22}]

↓ After decode_hex_escapes():
[{"Salary":"50000$"}]

↓ After fix double Unicode escapes:
[{"Title":"Intégrateur \\u2013 Profil"}]
→ [{"Title":"Intégrateur \u2013 Profil"}]

↓ After remove invalid escapes:
[{"Salary":"50000$ - 80000$"}]  # \- becomes -

↓ After codecs.decode('unicode_escape'):
[{"Salary":"50000$ - 80000$"}]  # \u2013 becomes –

↓ After json.loads():
Python dict/list structure

↓ After fix_encoding():
[{"Langue":["Français","English"]}]  # Fixes mojibake
```

### Why Each Step Is Needed

1. **Hex Escapes (`\xNN`)**: JavaScript uses these for non-ASCII chars
   - `\x22` = `"`
   - Must decode BEFORE `json.loads()` because JSON doesn't understand `\x`

2. **Double Unicode Escapes (`\\u` → `\u`)**:
   - JavaScript sometimes double-escapes: `\\u2013`
   - Need to fix before Unicode decoding

3. **Invalid Escapes (`\-`, `\/`)**:
   - JavaScript allows these, JSON doesn't
   - Must remove before `json.loads()`

4. **Unicode Escapes (`\uNNNN`)**:
   - Standard Unicode encoding
   - `codecs.decode('unicode_escape')` handles this

5. **UTF-8 Mojibake**:
   - Source data stored as UTF-8 bytes but interpreted as Latin-1
   - `encode('latin-1').decode('utf-8')` reverses this
   - Example: `Ã©` (2 bytes: C3 A9) → `é` (U+00E9)

## Limitations of hq

### ✅ What hq CAN Do

- Extract HTML elements by CSS selector
- Extract text content from tags (`-t, --text`)
- Extract attributes (`-a, --attributes`)
- Decode HTML entities when using `-t -c` flags
- Parse LD+JSON: `hq -t 'script[type="application/ld+json"]'`

### ❌ What hq CANNOT Do

- Parse JavaScript code
- Decode JavaScript escape sequences (`\x`, `\u`)
- Extract data from inside JS variables
- Use regex patterns for text matching

### When to Use hq vs. Other Tools

Use **hq** for:
```bash
# Extract LD+JSON (works great!)
cat page.html | hq -t 'script[type="application/ld+json"]' | jq .

# Extract all links
cat page.html | hq -a href 'a'

# Get meta tags
cat page.html | hq -a content 'meta[property^="og:"]'
```

Use **hq + sed/grep + Python** for:
```bash
# Extract JSON from JavaScript variables
cat page.html | hq -t 'script' | \
  grep "var data =" | \
  sed 's/var data = //' | \
  ./decode-js-json.py
```

## Common Encoding Issues

### Issue 1: Mojibake (Garbled Text)

**Symptom**: `Ã©` instead of `é`, `Ã§` instead of `ç`

**Cause**: UTF-8 bytes interpreted as Latin-1 (or vice versa)

**Fix**:
```python
text.encode('latin-1').decode('utf-8')  # UTF-8 → Latin-1 → UTF-8
```

### Issue 2: JavaScript Hex Escapes

**Symptom**: `\x22Hello\x22` in the string

**Cause**: JavaScript encoding for quotes and special chars

**Fix**:
```python
re.sub(r'\\x([0-9a-fA-F]{2})', lambda m: chr(int(m.group(1), 16)), text)
```

### Issue 3: Invalid JSON Escapes

**Symptom**: `JSONDecodeError: Invalid \escape: \-`

**Cause**: JavaScript allows `\-` and `\/`, JSON doesn't

**Fix**:
```python
text = text.replace('\\-', '-').replace('\\/', '/')
```

## Example Output

```json
[
  {
    "Salary": "50000$ - 80000$",
    "Remote_Job": false,
    "Langue": ["Français", "English"],
    "Posting_Title": "Intégrateur Zoho – Profil orienté affaires et ERP",
    "City": "Terrebonne",
    "State": "Quebec",
    "Country": "Canada"
  }
]
```

## Querying the Data

```bash
# Get all job titles
jq -r '.[].Posting_Title' zoho-jobs.json

# Filter by city
jq '.[] | select(.City == "Terrebonne")' zoho-jobs.json

# Get remote jobs only
jq '.[] | select(.Remote_Job == true)' zoho-jobs.json

# Format as CSV
jq -r '.[] | [.Posting_Title, .City, .Salary] | @csv' zoho-jobs.json
```

## Alternative: Node.js Approach

If you prefer Node.js over Python:

```bash
cat ~/zoho-career.html | hq -t 'script' | \
  grep "var jobs = JSON.parse(" | \
  sed -E "s/.*JSON\.parse\('(.*)'\).*/\1/" | \
  node -e "
    const fs = require('fs');
    const input = fs.readFileSync(0, 'utf-8');
    // eval() safely decodes JavaScript escapes
    const data = JSON.parse(eval('\'' + input + '\''));
    console.log(JSON.stringify(data, null, 2));
  "
```

⚠️ **Warning**: Using `eval()` is dangerous with untrusted input. The Python approach is safer.

## References

- [Unicode in Python](https://docs.python.org/3/howto/unicode.html)
- [JavaScript String Escapes](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String#escape_sequences)
- [UTF-8 Encoding Issues (Mojibake)](https://en.wikipedia.org/wiki/Mojibake)
- [hq Documentation](https://github.com/MultisampledNight/hq)
