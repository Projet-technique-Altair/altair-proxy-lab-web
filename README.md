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
