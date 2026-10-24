output "invoice_bucket" {
  value = aws_s3_bucket.invoices.bucket
}

output "payments_dlq_url" {
  value = aws_sqs_queue.payments_dlq.url
}

output "payment_alerts_topic_arn" {
  value = aws_sns_topic.payment_alerts.arn
}

output "notify_email_queue_url" {
  value = aws_sqs_queue.notify_email.url
}

output "notify_sms_queue_url" {
  value = aws_sqs_queue.notify_sms.url
}

output "postgres_secret_arn" {
  value = aws_secretsmanager_secret.postgres.arn
}

output "app_keys_secret_arn" {
  value = aws_secretsmanager_secret.app_keys.arn
}
