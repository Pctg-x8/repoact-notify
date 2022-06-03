

variable "enable_debug_log" {
  type    = bool
  default = false
}

variable "base_path" {
  type = string
}

variable "invocation_source_arn" {
  type = string
}

variable "api_id" {
  type = string
}

locals {
  function_name = "Peridot-GithubActivityNotification-Configurator"
}

resource "aws_lambda_function" "function" {
  function_name = local.function_name
  description   = "repoact-notify Configurator"
  role          = aws_iam_role.execution_role.arn

  filename         = "${path.module}/../package.zip"
  source_code_hash = filebase64sha256("${path.module}/../package.zip")
  handler          = "hello.handler"
  runtime          = "provided.al2"

  environment {
    variables = {
      RUST_LOG       = var.enable_debug_log ? "trace" : "error"
      RUST_BACKTRACE = 1
    }
  }

  depends_on = [
    aws_iam_policy.logging_policy,
    aws_cloudwatch_log_group.function_log_group
  ]
}

resource "aws_lambda_permission" "invocation_permission" {
  function_name = aws_lambda_function.function.function_name
  action        = "lambda:InvokeFunction"
  principal     = "apigateway.amazonaws.com"
  source_arn    = var.invocation_source_arn
}

resource "aws_apigatewayv2_integration" "api_lambda_integration" {
  api_id                 = var.api_id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.function.arn
  integration_method     = "POST"
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "route" {
  api_id    = var.api_id
  route_key = "POST ${var.base_path}"
  target    = "integrations/${aws_apigatewayv2_integration.api_lambda_integration.id}"
}

resource "aws_iam_role" "execution_role" {
  name = "${local.function_name}-ExecutionRole"
  path = "/service-role/webhook/PeridotGithubActivity/configurator/"
  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect    = "Allow",
        Action    = "sts:AssumeRole",
        Principal = { Service = "lambda.amazonaws.com" }
      }
    ]
  })
}

resource "aws_iam_policy" "logging_policy" {
  name = "${local.function_name}-LambdaLogStream"
  path = "/webhook/PeridotGithubActivity/configurator/"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect   = "Allow",
        Action   = ["logs:CreateLogStream", "logs:PutLogEvents"],
        Resource = "${aws_cloudwatch_log_group.function_log_group.arn}:*"
      }
    ]
  })
}

resource "aws_iam_policy" "secret_read_policy" {
  name = "${local.function_name}-LambdaSecretReadPolicy"
  path = "/webhook/PeridotGithubActivity/configurator/"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Action = "secretsmanager:GetSecretValue",
        Resource = [
          data.aws_secretsmanager_secret.secrets.arn,
          data.aws_secretsmanager_secret.configurator_secrets.arn
        ]
      }
    ]
  })
}

resource "aws_iam_policy" "routemap_write_policy" {
  name = "${local.function_name}-LambdaRouteMapWritePolicy"
  path = "/webhook/PeridotGithubActivity/configurator/"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect   = "Allow",
        Action   = "dynamodb:PutItem",
        Resource = data.aws_dynamodb_table.routemap.arn
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "execution_role_logging_policy" {
  role       = aws_iam_role.execution_role.name
  policy_arn = aws_iam_policy.logging_policy.arn
}

resource "aws_iam_role_policy_attachment" "execution_role_secret_read_policy" {
  role       = aws_iam_role.execution_role.name
  policy_arn = aws_iam_policy.secret_read_policy.arn
}

resource "aws_iam_role_policy_attachment" "execution_role_routemap_write_policy" {
  role       = aws_iam_role.execution_role.name
  policy_arn = aws_iam_policy.routemap_write_policy.arn
}

resource "aws_cloudwatch_log_group" "function_log_group" {
  name              = "/aws/lambda/${local.function_name}"
  retention_in_days = 1
}

# externally defined resources

data "aws_secretsmanager_secret" "secrets" {
  name = "repoact-notify"
}

data "aws_secretsmanager_secret" "configurator_secrets" {
  name = "masquerade-configurator"
}

data "aws_dynamodb_table" "routemap" {
  name = "Peridot-GithubActivityNotification-RouteMap"
}
