terraform {
  required_version = ">= 1.5.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.27.0, < 3.0.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.12.0, < 3.0.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.40.0, < 6.0.0"
    }
  }
}
