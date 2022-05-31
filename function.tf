variable "enable_debug_log" {
  type    = bool
  default = false
}

variable "target_path" {
  type    = string
  default = "/peridot"
}

variable "function_name" {
  type    = string
  default = "Peridot-GithubActivityNotification"
}

resource "aws_lambda_function" "function" {
  function_name = var.function_name
  description   = "Notification Sender for Activities on GitHub(Pctg-x8/peridot)"
  role          = aws_iam_role.execution_role.arn

  filename         = "package.zip"
  source_code_hash = filebase64sha256("package.zip")
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
  source_arn    = "${aws_apigatewayv2_stage.default_stage.execution_arn}/*"
}

resource "aws_apigatewayv2_integration" "api_lambda_integration" {
  api_id                 = aws_apigatewayv2_api.api.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.function.arn
  integration_method     = "POST"
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "route" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "POST ${var.target_path}"
  target    = "integrations/${aws_apigatewayv2_integration.api_lambda_integration.id}"
}

resource "aws_iam_role" "execution_role" {
  name = "ExecutionRole"
  path = "/service-role/webhook/PeridotGithubActivity/"
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
  name = "LambdaLogStream"
  path = "/webhook/PeridotGithubActivity/"
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
  name = "LambdaSecretReadPolicy"
  path = "/webhook/PeridotGithubActivity/"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect   = "Allow",
        Action   = "secretsmanager:GetSecretValue",
        Resource = data.aws_secretsmanager_secret.secrets.arn
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

resource "aws_apigatewayv2_api" "api" {
  name          = "GitHubWebhookGate"
  description   = "GitHub Webhook Gateway"
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_stage" "default_stage" {
  name        = "$default"
  api_id      = aws_apigatewayv2_api.api.id
  auto_deploy = true
  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.access_log_group.arn
    format = jsonencode({
      requestId = "$context.requestId",
      ip        = "$context.identity.sourceIp",
      caller    = "$context.identity.caller",
      status    = "$context.status"
    })
  }
}

resource "aws_apigatewayv2_api_mapping" "default_mapping" {
  api_id      = aws_apigatewayv2_api.api.id
  stage       = aws_apigatewayv2_stage.default_stage.id
  domain_name = aws_apigatewayv2_domain_name.webhook_domain.id
}

resource "aws_apigatewayv2_domain_name" "webhook_domain" {
  domain_name = "github.webhook.ct2.io"
  domain_name_configuration {
    endpoint_type   = "REGIONAL"
    certificate_arn = data.aws_acm_certificate.cert.arn
    security_policy = "TLS_1_2"
  }
}

resource "aws_cloudwatch_log_group" "access_log_group" {
  name              = "/webhook/PeridotGithubActivity/AccessLogGroup"
  retention_in_days = 1
}

resource "aws_cloudwatch_log_group" "function_log_group" {
  name              = "/aws/lambda/${var.function_name}"
  retention_in_days = 1
}

# externally defined resources

data "aws_secretsmanager_secret" "secrets" {
  name = "repoact-notify"
}

data "aws_acm_certificate" "cert" {
  domain = "*.webhook.ct2.io"
}
