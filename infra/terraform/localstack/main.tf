###############################################################################
# PShop cloud resources, provisioned into LocalStack.
#
# The same configuration is intended to apply to real AWS: delete the `endpoints`
# block and the skip_* flags in the provider and point it at a real account.
###############################################################################

terraform {
  required_version = ">= 1.6"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.70"
    }
  }
}

provider "aws" {
  region     = var.region
  access_key = "test"
  secret_key = "test"

  s3_use_path_style           = true
  skip_credentials_validation = true
  skip_metadata_api_check     = true
  skip_requesting_account_id  = true

  endpoints {
    s3             = var.localstack_endpoint
    sqs            = var.localstack_endpoint
    sns            = var.localstack_endpoint
    secretsmanager = var.localstack_endpoint
  }
}

locals {
  prefix = "pshop"
}

###############################################################################
# S3 — invoice PDFs
###############################################################################

resource "aws_s3_bucket" "invoices" {
  bucket        = "${local.prefix}-invoices"
  force_destroy = true # dev only
}

resource "aws_s3_bucket_versioning" "invoices" {
  bucket = aws_s3_bucket.invoices.id
  versioning_configuration {
    status = "Enabled" # a re-PUT after a retry never destroys the previous object
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "invoices" {
  bucket = aws_s3_bucket.invoices.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "invoices" {
  bucket                  = aws_s3_bucket.invoices.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_lifecycle_configuration" "invoices" {
  bucket = aws_s3_bucket.invoices.id
  rule {
    id     = "expire-old-invoices"
    status = "Enabled"
    filter {}
    expiration {
      days = var.invoice_retention_days
    }
    noncurrent_version_expiration {
      noncurrent_days = 30
    }
  }
}

###############################################################################
# SQS — dead-letter queue for payments that exhausted their retries
###############################################################################

resource "aws_sqs_queue" "payments_dlq" {
  name                      = "${local.prefix}-payments-dlq"
  message_retention_seconds = 1209600 # 14 days: long enough for a human to look
  sqs_managed_sse_enabled   = true
}

###############################################################################
# SNS — fan-out for "major payment error" alerts
#
# One publish -> email worker queue AND sms worker queue, in parallel.
# (On real AWS you can additionally subscribe email / SMS endpoints directly:
#  set alert_email / alert_phone.)
###############################################################################

resource "aws_sns_topic" "payment_alerts" {
  name = "${local.prefix}-payment-alerts"
}

resource "aws_sqs_queue" "notify_email" {
  name                    = "${local.prefix}-notify-email"
  sqs_managed_sse_enabled = true
}

resource "aws_sqs_queue" "notify_sms" {
  name                    = "${local.prefix}-notify-sms"
  sqs_managed_sse_enabled = true
}

data "aws_iam_policy_document" "allow_sns" {
  for_each = {
    email = aws_sqs_queue.notify_email.arn
    sms   = aws_sqs_queue.notify_sms.arn
  }
  statement {
    sid       = "AllowSnsFanOut"
    effect    = "Allow"
    actions   = ["sqs:SendMessage"]
    resources = [each.value]
    principals {
      type        = "Service"
      identifiers = ["sns.amazonaws.com"]
    }
    condition {
      test     = "ArnEquals"
      variable = "aws:SourceArn"
      values   = [aws_sns_topic.payment_alerts.arn]
    }
  }
}

resource "aws_sqs_queue_policy" "notify_email" {
  queue_url = aws_sqs_queue.notify_email.id
  policy    = data.aws_iam_policy_document.allow_sns["email"].json
}

resource "aws_sqs_queue_policy" "notify_sms" {
  queue_url = aws_sqs_queue.notify_sms.id
  policy    = data.aws_iam_policy_document.allow_sns["sms"].json
}

resource "aws_sns_topic_subscription" "email_worker" {
  topic_arn            = aws_sns_topic.payment_alerts.arn
  protocol             = "sqs"
  endpoint             = aws_sqs_queue.notify_email.arn
  raw_message_delivery = true
}

resource "aws_sns_topic_subscription" "sms_worker" {
  topic_arn            = aws_sns_topic.payment_alerts.arn
  protocol             = "sqs"
  endpoint             = aws_sqs_queue.notify_sms.arn
  raw_message_delivery = true
}

resource "aws_sns_topic_subscription" "email_direct" {
  count     = var.alert_email == "" ? 0 : 1
  topic_arn = aws_sns_topic.payment_alerts.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

resource "aws_sns_topic_subscription" "sms_direct" {
  count     = var.alert_phone == "" ? 0 : 1
  topic_arn = aws_sns_topic.payment_alerts.arn
  protocol  = "sms"
  endpoint  = var.alert_phone
}

###############################################################################
# Secrets Manager — fetched by the Rust services at startup
###############################################################################

resource "aws_secretsmanager_secret" "postgres" {
  name                    = "${local.prefix}/postgres"
  description             = "Postgres credentials for billing-service"
  recovery_window_in_days = 0 # dev: allow immediate delete/recreate
}

resource "aws_secretsmanager_secret_version" "postgres" {
  secret_id = aws_secretsmanager_secret.postgres.id
  secret_string = jsonencode({
    username = var.pg_user
    password = var.pg_password
    host     = var.pg_host
    port     = var.pg_port
    dbname   = var.pg_db
  })
}

resource "aws_secretsmanager_secret" "app_keys" {
  name                    = "${local.prefix}/app-keys"
  description             = "Application keys (invoice signing)"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "app_keys" {
  secret_id = aws_secretsmanager_secret.app_keys.id
  secret_string = jsonencode({
    invoice_signing_key = var.invoice_signing_key
  })
}
