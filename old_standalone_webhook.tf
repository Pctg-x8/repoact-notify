variable "enable_debug_log" {
  type    = bool
  default = false
}

variable "target_path" {
  type    = string
  default = "/peridot"
}

module "masq" {
  source = "./masquerade"

  base_path             = var.target_path
  enable_debug_log      = var.enable_debug_log
  api_id                = aws_apigatewayv2_api.api.id
  invocation_source_arn = "${aws_apigatewayv2_stage.default_stage.execution_arn}/*"
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

# externally defined resources

data "aws_acm_certificate" "cert" {
  domain = "*.webhook.ct2.io"
}
