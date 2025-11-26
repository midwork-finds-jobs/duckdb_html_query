#!/usr/bin/env bash

set -euo pipefail

# Create output directory
mkdir -p dist/lambda

# Build Lambda function for ARM64 (AWS Graviton)
echo "Building Lambda binary for ARM64..."
cargo lambda build --release --bin lambda --features lambda --arm64

# Package ARM64 binary
echo "Packaging ARM64 binary..."
cp target/lambda/lambda/bootstrap dist/lambda/bootstrap
cd dist/lambda
zip -q lambda-arm64.zip bootstrap
cd ../..

# Build Lambda function for x86_64
echo "Building Lambda binary for x86_64..."
cargo lambda build --release --bin lambda --features lambda --x86-64

# Package x86_64 binary
echo "Packaging x86_64 binary..."
cp target/lambda/lambda/bootstrap dist/lambda/bootstrap
cd dist/lambda
zip -q lambda-x86_64.zip bootstrap
rm bootstrap
cd ../..

echo "Lambda packages created:"
echo "  - dist/lambda/lambda-arm64.zip"
echo "  - dist/lambda/lambda-x86_64.zip"
