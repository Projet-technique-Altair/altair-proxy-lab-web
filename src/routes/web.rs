use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    extract::Path,
    http::{HeaderName, Request, Response, StatusCode, Uri},
};
use tracing::error;

const DEFAULT_COOKIE_NAME: &str = "altair_web_session";

pub async fn proxy_root_request(
    Path(container_id): Path<String>,
    request: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    validate_container_id(&container_id)?;
    let original_uri = request.uri().clone();
    let target_url = build_session_service_target_url(&container_id, None, &original_uri)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    proxy_request(target_url, request).await
}

pub async fn proxy_path_request(
    Path((container_id, path)): Path<(String, String)>,
    request: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    validate_container_id(&container_id)?;
    let path = validate_lab_relative_path(&path)?;
    let original_uri = request.uri().clone();
    let target_url = build_session_service_target_url(&container_id, Some(&path), &original_uri)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    proxy_request(target_url, request).await
}

fn validate_container_id(container_id: &str) -> Result<(), StatusCode> {
    if container_id.is_empty() || container_id.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }

    if container_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

fn validate_lab_relative_path(path: &str) -> Result<String, StatusCode> {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if segments.iter().any(|segment| {
        *segment == "."
            || *segment == ".."
            || segment.contains('\\')
            || segment.chars().any(|ch| ch.is_control())
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(segments.join("/"))
}

fn build_session_service_target_url(
    container_id: &str,
    path: Option<&str>,
    original_uri: &Uri,
) -> Result<String, String> {
    let namespace = std::env::var("WEB_PROXY_NAMESPACE").unwrap_or_else(|_| "labs-web".to_string());
    let service_suffix =
        std::env::var("WEB_PROXY_SERVICE_SUFFIX").unwrap_or_else(|_| "-web".to_string());

    let mut target_url = if let Some(service_host) =
        read_session_service_env(container_id, &service_suffix, "SERVICE_HOST")
    {
        let service_port = read_session_service_env(container_id, &service_suffix, "SERVICE_PORT")
            .unwrap_or_else(|| "80".to_string());
        format!("http://{}:{}", service_host, service_port)
    } else {
        format!(
            "http://{}{service_suffix}.{namespace}.svc.cluster.local",
            container_id
        )
    };

    let stripped_path = match path {
        Some(value) if !value.is_empty() => format!("/{}", value.trim_start_matches('/')),
        _ => "/".to_string(),
    };
    target_url.push_str(&stripped_path);

    if let Some(query) = original_uri.query() {
        target_url.push('?');
        target_url.push_str(query);
    }

    Ok(target_url)
}

fn read_session_service_env(
    container_id: &str,
    service_suffix: &str,
    suffix: &str,
) -> Option<String> {
    let service_name = format!("{}{}", container_id, service_suffix);
    let env_key = service_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    std::env::var(format!("{}_{}", env_key, suffix)).ok()
}

async fn proxy_request(
    target_url: String,
    request: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let timeout_secs = std::env::var("WEB_PROXY_REQUEST_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|error| {
            error!("failed to build LAB-WEB proxy client: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let method = reqwest::Method::from_bytes(request.method().as_str().as_bytes())
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let cookie_name =
        std::env::var("LAB_WEB_COOKIE_NAME").unwrap_or_else(|_| DEFAULT_COOKIE_NAME.to_string());

    let forwarded_headers: Vec<(HeaderName, axum::http::HeaderValue)> = request
        .headers()
        .iter()
        .filter_map(|(name, value)| filter_request_header(name, value, &cookie_name))
        .collect();

    let body_bytes = to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|error| {
            error!(
                "failed to read LAB-WEB request body for {}: {}",
                target_url, error
            );
            StatusCode::BAD_GATEWAY
        })?;

    let mut outbound = client.request(method, &target_url).body(body_bytes);
    for (name, value) in forwarded_headers {
        outbound = outbound.header(name, value);
    }

    let upstream = outbound.send().await.map_err(|error| {
        error!(
            "LAB-WEB upstream request failed for {}: {}",
            target_url, error
        );
        StatusCode::BAD_GATEWAY
    })?;
    let status = upstream.status();
    let response_headers = upstream.headers().clone();
    let body = upstream.bytes().await.map_err(|error| {
        error!(
            "failed to read LAB-WEB upstream response body for {}: {}",
            target_url, error
        );
        StatusCode::BAD_GATEWAY
    })?;

    if !status.is_success() {
        let body_preview = String::from_utf8_lossy(&body);
        error!(
            "LAB-WEB upstream returned {} for {} with body preview: {}",
            status,
            target_url,
            body_preview.chars().take(200).collect::<String>()
        );
    }
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;

    for (name, value) in &response_headers {
        if !is_hop_by_hop_header(name) {
            response.headers_mut().append(name, value.clone());
        }
    }

    Ok(response)
}

fn filter_request_header(
    name: &HeaderName,
    value: &axum::http::HeaderValue,
    lab_web_cookie_name: &str,
) -> Option<(HeaderName, axum::http::HeaderValue)> {
    if is_hop_by_hop_header(name) {
        return None;
    }

    if name.as_str().eq_ignore_ascii_case("cookie") {
        let raw = value.to_str().ok()?;
        let filtered = strip_cookie_by_name(raw, lab_web_cookie_name);

        if filtered.is_empty() {
            return None;
        }

        let rewritten = axum::http::HeaderValue::from_str(&filtered).ok()?;
        return Some((name.clone(), rewritten));
    }

    if is_platform_private_header(name) {
        return None;
    }

    Some((name.clone(), value.clone()))
}

fn is_platform_private_header(name: &HeaderName) -> bool {
    let header = name.as_str().to_ascii_lowercase();

    header == "authorization"
        || header == "origin"
        || header == "referer"
        || header.starts_with("x-altair-")
}

fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
    )
}

fn strip_cookie_by_name(cookie_header: &str, cookie_name: &str) -> String {
    cookie_header
        .split(';')
        .filter_map(|pair| {
            let trimmed = pair.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut parts = trimmed.splitn(2, '=');
            let name = parts.next()?.trim();
            let value = parts.next()?.trim();

            if name == cookie_name || value.is_empty() {
                None
            } else {
                Some(format!("{name}={value}"))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderName, HeaderValue, StatusCode, Uri};

    use super::{
        build_session_service_target_url, filter_request_header, strip_cookie_by_name,
        validate_container_id, validate_lab_relative_path,
    };

    #[test]
    fn validate_container_id_rejects_unexpected_characters() {
        assert_eq!(
            validate_container_id("ctf-session-123?target=http://evil.test").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_lab_relative_path_rejects_dot_segments() {
        assert_eq!(
            validate_lab_relative_path("../admin").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_lab_relative_path_keeps_normal_runtime_path() {
        assert_eq!(
            validate_lab_relative_path("assets/app.js").unwrap(),
            "assets/app.js"
        );
    }

    #[test]
    fn session_service_target_rewrites_root_path() {
        let uri: Uri = "/web/ctf-session-123".parse().unwrap();
        let target = build_session_service_target_url("ctf-session-123", None, &uri).unwrap();

        assert_eq!(
            target,
            "http://ctf-session-123-web.labs-web.svc.cluster.local/"
        );
    }

    #[test]
    fn session_service_target_rewrites_nested_path_and_query() {
        let uri: Uri = "/web/ctf-session-123/assets/app.js?lang=en"
            .parse()
            .unwrap();
        let target =
            build_session_service_target_url("ctf-session-123", Some("assets/app.js"), &uri)
                .unwrap();

        assert_eq!(
            target,
            "http://ctf-session-123-web.labs-web.svc.cluster.local/assets/app.js?lang=en"
        );
    }

    #[test]
    fn strip_cookie_by_name_keeps_lab_application_cookies() {
        let filtered = strip_cookie_by_name(
            "altair_web_session=token; unlocked=1; theme=dark",
            "altair_web_session",
        );

        assert_eq!(filtered, "unlocked=1; theme=dark");
    }

    #[test]
    fn filter_request_header_drops_cookie_header_when_only_lab_cookie_exists() {
        let filtered = filter_request_header(
            &HeaderName::from_static("cookie"),
            &HeaderValue::from_static("altair_web_session=token"),
            "altair_web_session",
        );

        assert!(filtered.is_none());
    }

    #[test]
    fn filter_request_header_rewrites_cookie_header_when_app_cookie_exists() {
        let filtered = filter_request_header(
            &HeaderName::from_static("cookie"),
            &HeaderValue::from_static("altair_web_session=token; unlocked=1"),
            "altair_web_session",
        );

        assert_eq!(
            filtered,
            Some((
                HeaderName::from_static("cookie"),
                HeaderValue::from_static("unlocked=1")
            ))
        );
    }
}
