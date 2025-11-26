# hq
Like [`jq`](https://stedolan.github.io/jq/), but for HTML. Uses [CSS selectors](https://developer.mozilla.org/en-US/docs/Learn/CSS/Introduction_to_CSS/Selectors) to extract bits of content from HTML files.

## Installation

### Cargo

```sh
cargo install --git https://github.com/MultisampledNight/hq
```

## Examples

### Using with cURL to find part of a page by ID

```console
$ curl --silent https://www.rust-lang.org/ | hq '#get-help'
<div class="four columns mt3 mt0-l" id="get-help">
        <h4>Get help!</h4>
        <ul>
          <li><a href="https://doc.rust-lang.org">Documentation</a></li>
          <li><a href="https://users.rust-lang.org">Ask a Question on the Users Forum</a></li>
          <li><a href="http://ping.rust-lang.org">Check Website Status</a></li>
        </ul>
        <div class="languages">
            <label class="hidden" for="language-footer">Language</label>
            <select id="language-footer">
                <option title="English (US)" value="en-US">English (en-US)</option>
<option title="French" value="fr">Français (fr)</option>
<option title="German" value="de">Deutsch (de)</option>

            </select>
        </div>
      </div>
```

### Find all the links in a page

```console
$ curl --silent https://www.rust-lang.org/ | hq --attribute href a
/
/tools/install
/learn
/tools
/governance
/community
https://blog.rust-lang.org/
/learn/get-started
https://blog.rust-lang.org/2019/04/25/Rust-1.34.1.html
https://blog.rust-lang.org/2018/12/06/Rust-1.31-and-rust-2018.html
[...]
```

### Pretty print HTML

(This is a bit of a work in progress)

```console
$ curl --silent https://mgdm.net | hq --pretty '#posts'
<section id="posts">
  <h2>I write about...
  </h2>
  <ul class="post-list">
    <li>
      <time datetime="2019-04-29 00:%i:1556496000" pubdate="">
        29/04/2019</time><a href="/weblog/nettop/">
        <h3>Debugging network connections on macOS with nettop
        </h3></a>
      <p>Using nettop to find out what network connections a program is trying to make.
      </p>
    </li>
[...]
```

### Syntax highlighting with [`bat`](https://github.com/sharkdp/bat)

```console
$ curl --silent example.com | hq 'body' | bat --language html
```

> <img alt="Syntax highlighted output" width="700" src="https://user-images.githubusercontent.com/2346707/132808980-db8991ff-9177-4cb7-a018-39ad94282374.png" />

## AWS Lambda Deployment

Deploy `hq` as an AWS Lambda function to process HTML via HTTP requests.

### Query Parameters

- `url` (required): URL to fetch HTML from (supports `http://`, `https://`, or `s3://`)
- `selector` (optional): CSS selector (default: `:root`)
- `text` (optional): Extract text only (`true`/`1`)
- `pretty` (optional): Pretty print output (`true`/`1`)
- `attribute` (optional): Extract specific attributes (can be repeated)
- `compact` (optional): Compact output (`true`/`1`)
- `offset` (optional): Byte offset for partial fetches
- `length` (optional): Byte length for partial fetches

### Download Pre-built Binaries

Download from [GitHub Releases](https://github.com/MultisampledNight/hq/releases):
- `lambda-arm64.zip` (AWS Graviton2/3)
- `lambda-x86_64.zip` (Intel/AMD)

### Build Lambda Package (Optional)

```sh
./build-lambda.sh
```

Creates:
- `dist/lambda/lambda-arm64.zip` (AWS Graviton)
- `dist/lambda/lambda-x86_64.zip` (Intel/AMD)

### Deploy with OpenTofu/Terraform

```sh
tofu init
tofu apply
```

Variables:
- `aws_region`: AWS region (default: `us-east-1`)
- `lambda_architecture`: `arm64` or `x86_64` (default: `arm64`)
- `function_name`: Lambda name (default: `hq`)

### Manual Deployment with AWS CLI

#### 1. Create IAM Role

```sh
# Create trust policy
cat > /tmp/trust-policy.json << 'EOF'
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {"Service": "lambda.amazonaws.com"},
    "Action": "sts:AssumeRole"
  }]
}
EOF

# Create role
aws iam create-role \
  --role-name hq-lambda-role \
  --assume-role-policy-document file:///tmp/trust-policy.json

# Attach basic execution policy
aws iam attach-role-policy \
  --role-name hq-lambda-role \
  --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole

# Add S3 read permissions
cat > /tmp/s3-policy.json << 'EOF'
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject", "s3:ListBucket"],
    "Resource": "*"
  }]
}
EOF

aws iam put-role-policy \
  --role-name hq-lambda-role \
  --policy-name s3-read-access \
  --policy-document file:///tmp/s3-policy.json
```

#### 2. Create Lambda Function

```sh
# Get your AWS account ID
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)

# Create function
aws lambda create-function \
  --function-name hq \
  --runtime provided.al2023 \
  --role arn:aws:iam::${ACCOUNT_ID}:role/hq-lambda-role \
  --handler bootstrap \
  --zip-file fileb://dist/lambda/lambda-arm64.zip \
  --architectures arm64 \
  --timeout 30 \
  --memory-size 256 \
  --environment Variables='{RUST_LOG=info}'
```

#### 3. Create Function URL

```sh
# Create public function URL
aws lambda create-function-url-config \
  --function-name hq \
  --auth-type NONE \
  --cors AllowOrigins="*",AllowMethods=GET,MaxAge=86400

# Add public access permission
aws lambda add-permission \
  --function-name hq \
  --statement-id FunctionURLAllowPublicAccess \
  --action lambda:InvokeFunctionUrl \
  --principal "*" \
  --function-url-auth-type NONE
```

#### 4. Get Function URL

```sh
aws lambda get-function-url-config --function-name hq
```

### Example Usage

HTTP/HTTPS URLs:
```sh
curl "https://YOUR_FUNCTION_URL?url=https://example.com&selector=title"
```

S3 URLs (including public buckets like Common Crawl):
```sh
# Extract title from S3-hosted HTML
curl "https://YOUR_FUNCTION_URL?url=s3://bucket/path/file.html&selector=title"

# Common Crawl example (note: large files may exceed Lambda response limits)
curl "https://YOUR_FUNCTION_URL?url=s3://commoncrawl/crawl-data/CC-MAIN-2024-10/warc.paths.gz&selector=body&offset=0&length=50000"
```

**Note:** Lambda responses are limited to 6MB. For large S3 files, use `offset` and `length` parameters to fetch partial content.

## Releases

To create a new release with Lambda binaries:

```sh
git tag -a v0.x.x -m "Release v0.x.x"
git push origin v0.x.x
```

GitHub Actions will automatically build Lambda binaries and create a release.
