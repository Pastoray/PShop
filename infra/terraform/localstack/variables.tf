variable "localstack_endpoint" {
  description = "LocalStack edge URL. http://localhost:4566 from the host, http://localstack:4566 inside compose."
  type        = string
  default     = "http://localhost:4566"
}

variable "region" {
  type    = string
  default = "us-east-1"
}

variable "invoice_retention_days" {
  type    = number
  default = 90
}

variable "alert_email" {
  description = "Optional direct email subscription for payment alerts (real AWS)."
  type        = string
  default     = ""
}

variable "alert_phone" {
  description = "Optional direct SMS subscription (E.164) for payment alerts (real AWS)."
  type        = string
  default     = ""
}

# ---- secret material (never commit real values; pass via TF_VAR_* or a secrets backend) ----

variable "pg_host" {
  type    = string
  default = "localhost"
}

variable "pg_port" {
  type    = number
  default = 5432
}

variable "pg_user" {
  type    = string
  default = "pshop"
}

variable "pg_db" {
  type    = string
  default = "pshop"
}

variable "pg_password" {
  type      = string
  sensitive = true
  default   = "pshop"
}

variable "invoice_signing_key" {
  type      = string
  sensitive = true
  default   = "dev-only-change-me"
}
