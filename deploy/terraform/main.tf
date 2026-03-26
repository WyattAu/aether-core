terraform {
  required_version = ">= 1.5.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.27.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.12.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.40.0"
    }
  }
}

provider "kubernetes" {
  config_path = var.kubeconfig_path
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path
  }
}

resource "kubernetes_namespace" "aether" {
  metadata {
    name = var.namespace
    labels = {
      "app.kubernetes.io/name"      = "aether"
      "app.kubernetes.io/managed-by" = "terraform"
    }
  }
}

resource "helm_release" "aether" {
  name             = "aether"
  namespace        = kubernetes_namespace.aether.metadata[0].name
  chart            = "../helm/aether"
  create_namespace = false

  set {
    name  = "replicaCount"
    value = var.replica_count
  }

  set {
    name  = "image.tag"
    value = var.image_tag
  }

  set {
    name  = "resources.requests.cpu"
    value = "${var.cpu_limit}m"
  }

  set {
    name  = "resources.limits.cpu"
    value = var.cpu_limit
  }

  set {
    name  = "resources.limits.memory"
    value = var.memory_limit
  }

  set {
    name  = "ingress.enabled"
    value = var.enable_ingress
  }

  dynamic "set" {
    for_each = var.domain_name != "" ? [1] : []
    content {
      name  = "ingress.hosts[0].host"
      value = var.domain_name
    }
  }

  set {
    name  = "monitoring.prometheus.enabled"
    value = var.enable_monitoring
  }

  depends_on = [kubernetes_namespace.aether]
}

resource "aws_ecr_repository" "aether" {
  count = var.enable_ecr ? 1 : 0

  name                 = "aether-core"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = {
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

resource "aws_cloudwatch_log_group" "aether" {
  count = var.enable_monitoring ? 1 : 0

  name              = "/aether/${var.namespace}"
  retention_in_days = 30

  tags = {
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}
