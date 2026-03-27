# GitOps Workflow Guide

This document describes the GitOps deployment workflow for Aether, covering environment promotion and rollback procedures.

## Architecture

Aether uses a GitOps model where changes to the `deploy/` directory trigger automated deployments via GitHub Actions (`.github/workflows/gitops.yml`).

```
┌─────────────┐     push/deploy/**     ┌──────────────────┐     helm upgrade     ┌──────────────┐
│  Developer   │ ─────────────────────▶ │  GitHub Actions   │ ──────────────────▶ │  Kubernetes  │
│  (git push)  │                        │  (gitops.yml)     │                     │  Cluster     │
└─────────────┘                        └──────────────────┘                     └──────────────┘
```

## Environments

| Environment | Namespace  | Trigger                              |
|-------------|------------|--------------------------------------|
| Staging     | `aether`   | Push to `main` with `deploy/**` changes |
| Production  | `aether`   | Manual `workflow_dispatch` with `environment: production` |

## Workflow

### Automated Deploy (Staging)

Any push to `main` that modifies files under `deploy/` automatically triggers a staging deployment:

```bash
git checkout main
# Edit deploy/helm/aether/values.yaml or any deploy/ file
git add deploy/
git commit -m "update staging values"
git push origin main
```

### Manual Deploy (Any Environment)

Use `workflow_dispatch` to target a specific environment and version:

```bash
# Via GitHub UI: Actions → GitOps Deploy → Run workflow
# Or via gh CLI:
gh workflow run gitops.yml \
  -f environment=staging \
  -f version=v1.2.3
```

## Promoting Between Environments

### Staging → Production

1. **Validate in staging** — confirm the deployment is healthy:
   ```bash
   helm status aether -n aether
   kubectl rollout status deployment/aether -n aether
   ```

2. **Promote** — trigger production deploy with the tested version:
   ```bash
   gh workflow run gitops.yml \
     -f environment=production \
     -f version=<tested-version>
   ```

3. **Verify** — check production health:
   ```bash
   kubectl get pods -n aether -l app.kubernetes.io/name=aether
   kubectl logs -n aether -l app.kubernetes.io/name=aether --tail=100
   ```

### Production → Staging (Hotfix Backport)

If a hotfix is applied directly to production, backport the configuration change to staging:

```bash
git checkout main
git cherry-pick <hotfix-commit-sha>
git push origin main
```

## Rollback Procedures

### Option 1: Helm Rollback (Fastest)

```bash
# List recent releases
helm history aether -n aether

# Rollback to previous release
helm rollback aether 1 -n aether

# Rollback to specific revision
helm rollback aether <revision> -n aether
```

### Option 2: Git Revert (Recommended for Audit Trail)

```bash
# Find the commit that introduced the bad change
git log --oneline deploy/

# Revert it
git revert <bad-commit-sha>
git push origin main
```

This triggers the GitOps pipeline, which redeploys with the reverted configuration.

### Option 3: Manual Version Pin

Deploy a known-good version immediately:

```bash
gh workflow run gitops.yml \
  -f environment=production \
  -f version=<known-good-version>
```

## Helm Values Strategy

Environment-specific overrides live in the Helm chart:

```
deploy/helm/aether/
├── Chart.yaml
├── values.yaml            # Default values
├── values-staging.yaml    # Staging overrides
└── values-production.yaml # Production overrides
```

To select environment values, update the `--values` flag in `gitops.yml` or use `--set` parameters.

## Monitoring Deployments

After any deployment, verify via:

- **Pod health**: `kubectl get pods -n aether`
- **Rollout status**: `kubectl rollout status deployment/aether -n aether`
- **Helm status**: `helm status aether -n aether`
- **Logs**: `kubectl logs -n aether -l app.kubernetes.io/name=aether -f`
- **Metrics**: Check the Grafana dashboards in the monitoring stack (`deploy/monitoring/`)

## Troubleshooting

| Symptom                  | Action                                      |
|--------------------------|---------------------------------------------|
| Pod stuck in CrashLoop   | Check logs, revert to last known-good        |
| Helm timeout             | Increase `--timeout` or check resource limits |
| Image pull error         | Verify image tag exists in the registry       |
| Permission denied        | Check GitHub Actions environment secrets      |

See `deploy/runbooks/` for detailed incident response procedures.
