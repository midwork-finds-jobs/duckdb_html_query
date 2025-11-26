terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "aws_region" {
  description = "AWS region to deploy Lambda"
  type        = string
  default     = "us-east-1"
}

variable "lambda_architecture" {
  description = "Lambda architecture (arm64 or x86_64)"
  type        = string
  default     = "arm64"
  validation {
    condition     = contains(["arm64", "x86_64"], var.lambda_architecture)
    error_message = "Architecture must be arm64 or x86_64"
  }
}

variable "function_name" {
  description = "Lambda function name"
  type        = string
  default     = "hq"
}

provider "aws" {
  region = var.aws_region
}

resource "aws_iam_role" "lambda_role" {
  name = "${var.function_name}-lambda-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "lambda_basic" {
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
  role       = aws_iam_role.lambda_role.name
}

# Optional: Add S3 read access if you need to fetch HTML from S3
resource "aws_iam_role_policy" "lambda_s3" {
  name = "${var.function_name}-s3-read"
  role = aws_iam_role.lambda_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "s3:GetObject",
        "s3:ListBucket"
      ]
      Resource = "*"
    }]
  })
}

resource "aws_lambda_function" "hq" {
  filename         = "dist/lambda/lambda-${var.lambda_architecture}.zip"
  function_name    = var.function_name
  role            = aws_iam_role.lambda_role.arn
  handler         = "bootstrap"
  source_code_hash = filebase64sha256("dist/lambda/lambda-${var.lambda_architecture}.zip")
  runtime         = "provided.al2023"
  architectures   = [var.lambda_architecture]
  timeout         = 30
  memory_size     = 256

  environment {
    variables = {
      RUST_LOG = "info"
    }
  }
}

# Optional: Lambda Function URL for direct HTTP access
resource "aws_lambda_function_url" "hq" {
  function_name      = aws_lambda_function.hq.function_name
  authorization_type = "NONE"

  cors {
    allow_credentials = false
    allow_origins     = ["*"]
    allow_methods     = ["GET"]
    max_age          = 86400
  }
}

output "lambda_arn" {
  description = "ARN of the Lambda function"
  value       = aws_lambda_function.hq.arn
}

output "lambda_function_url" {
  description = "Function URL for direct HTTP access"
  value       = aws_lambda_function_url.hq.function_url
}

output "invoke_example" {
  description = "Example invocation"
  value       = "${aws_lambda_function_url.hq.function_url}?url=https://example.com&selector=title"
}
