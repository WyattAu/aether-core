output "service_url" {
  description = "URL to access the Aether service"
  value       = var.domain_name != "" ? "https://${var.domain_name}" : "http://${helm_release.aether.name}.${var.namespace}.svc.cluster.local:${var.service_port}"
}

output "service_name" {
  description = "Kubernetes service name"
  value       = "${helm_release.aether.name}-${helm_release.aether.chart}"
}

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.aether.metadata[0].name
}

variable "service_port" {
  description = "Internal service port"
  type        = number
  default     = 8080
}
