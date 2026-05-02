# altair-proxy-lab-web

Dedicated LAB-WEB runtime proxy.

## Responsibility
- receive `/web/{container_id}/...` traffic from the public gateway
- resolve the per-session Kubernetes Service in `labs-web`
- forward HTTP requests to the correct web lab runtime
- preserve application cookies while stripping the platform LAB-WEB cookie

## Required env vars
- `PORT` default `8086`
- `WEB_PROXY_NAMESPACE` default `labs-web`
- `WEB_PROXY_SERVICE_SUFFIX` default `-web`
- `WEB_PROXY_REQUEST_TIMEOUT_SECONDS` default `30`
- optional `LAB_WEB_COOKIE_NAME` default `altair_web_session`

## Deployment
- build image to Artifact Registry
- deploy `k8s/deployment.yaml`
- preserve the internal service endpoint `altair-web-proxy-internal` in namespace `labs-web`
- expose the new proxy workload behind that stable internal load balancer
## May 2026 Security And Platform Updates

- Runtime Docker image now installs only required packages with `--no-install-recommends` and runs as non-root UID `10001`.
- CORS origin handling is now allowlist-based through `ALLOWED_ORIGINS`; local defaults are `http://localhost:5173,http://localhost:3000`.
- Kubernetes deployment now sets `runAsNonRoot`, `runAsUser`, `runAsGroup`, `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`, drops Linux capabilities, and uses the runtime default seccomp profile.
- Latest Trivy scan status for this repo: no HIGH or CRITICAL findings.
