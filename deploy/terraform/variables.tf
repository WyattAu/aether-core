variable "namespace" {
  description = "Kubernetes namespace for Aether"
  type        = string
  default     = "aether"
}

variable "replica_count" {
  description = "Number of Aether replicas"
  type        = number
  default     = 3
}

variable "image_tag" {
  description = "Container image tag"
  type        = string
  default     = "latest"
}

variable "cpu_limit" {
  description = "CPU limit in millicores (e.g. 250 means 250m request, 0.25 limit)"
  type        = number
  default     = 250
}

variable "memory_limit" {
  description = "Memory limit (e.g. 2Gi)"
  type        = string
  default     = "2Gi"
}

variable "enable_ingress" {
  description = "Enable Ingress resource"
  type        = bool
  default     = true
}

variable "domain_name" {
  description = "Domain name for Ingress host"
  type        = string
  default     = ""
}

variable "enable_monitoring" {
  description = "Enable Prometheus annotations and CloudWatch"
  type        = bool
  default     = true
}

variable "enable_ecr" {
  description = "Create AWS ECR repository"
  type        = bool
  default     = false
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "dev"
}

variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}
