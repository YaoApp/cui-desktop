use axum::{
    Router,
    body::Body,
    extract::Request,
    response::Response,
};
use http::{header, HeaderValue, StatusCode};
use reqwest::Client;
use tower_http::cors::CorsLayer;
use tokio::net::TcpListener;
use tracing::{info, error, warn, debug};
use std::path::PathBuf;

use crate::config::{self, get_proxy_state};

/// Max request body size: 512 MB
const MAX_BODY_SIZE: usize = 512 * 1024 * 1024;

/// Start the local proxy server on the given port
pub async fn start_proxy_server(cui_dist_path: PathBuf, port: u16) -> Result<u16, String> {

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let cui_dist = cui_dist_path.clone();

    let app = Router::new()
        .fallback(move |req: Request| {
            let client = client.clone();
            let cui_dist = cui_dist.clone();
            async move {
                handle_request(req, client, cui_dist).await
            }
        })
        .layer(
            CorsLayer::very_permissive()
        );

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| format!("Failed to bind port {}: {}", port, e))?;

    info!("Proxy server started at http://127.0.0.1:{}", port);
    {
        let mut state = config::PROXY_STATE.write();
        state.running = true;
        state.port = port;
    }

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Proxy server error: {}", e);
            config::set_proxy_running(false);
        }
    });

    Ok(port)
}

/// Route handler:
///   /__yao_admin_root/* → local CUI static files
///   Everything else     → proxy to remote server (same-origin guarantee)
async fn handle_request(
    req: Request,
    client: Client,
    cui_dist: PathBuf,
) -> Response {
    let path = req.uri().path();

    // CUI static assets — served locally
    if path.starts_with("/__yao_admin_root/") {
        return serve_cui_static(path, &cui_dist).await;
    }

    // Redirect /__yao_admin_root (no trailing slash)
    if path == "/__yao_admin_root" {
        return Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(header::LOCATION, "/__yao_admin_root/")
            .body(Body::empty())
            .unwrap();
    }

    // Root → redirect to CUI
    if path == "/" {
        return Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(header::LOCATION, "/__yao_admin_root/")
            .body(Body::empty())
            .unwrap();
    }

    // Everything else → proxy to remote server
    // This covers /v1/*, /api/*, /web/*, /components/*, /assets/*,
    // /ai/*, /agents/*, /docs/*, /tools/*, /brands/*, /admin/*,
    // /iframe/*, /.well-known/*, and any SUI server-rendered pages.
    proxy_request(req, client).await
}

/// Forward a request to the remote Yao server
async fn proxy_request(req: Request, client: Client) -> Response {
    let state = get_proxy_state();

    if state.server_url.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from("Proxy server URL not configured"))
            .unwrap();
    }

    let method = req.method().clone();
    let uri = req.uri().clone();
    let path_and_query = uri.path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let remote_base = state.server_url.trim_end_matches('/').to_string();
    let target_url = format!("{}{}", remote_base, path_and_query);

    let local_base = format!("http://127.0.0.1:{}", state.port);
    debug!("Proxy: {} {}", method, target_url);

    // Build upstream request
    let mut builder = client.request(method, &target_url);

    // Collect browser Cookie header before iterating
    let browser_cookie_header = req.headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Copy headers (skip hop-by-hop; cookie is handled separately below)
    for (name, value) in req.headers() {
        let name_str = name.as_str().to_lowercase();
        if name_str == "host"
            || name_str == "connection"
            || name_str == "transfer-encoding"
            || name_str == "cookie"  // Handled separately: merge browser + jar
        {
            continue;
        }
        // Rewrite Origin/Referer to remote server (avoid CORS rejection)
        if name_str == "origin" {
            if let Ok(v) = HeaderValue::from_str(&remote_base) {
                builder = builder.header("Origin", v);
            }
            continue;
        }
        if name_str == "referer" {
            if let Ok(v) = value.to_str() {
                let rewritten = v.replace(&local_base, &remote_base);
                builder = builder.header("Referer", rewritten);
                continue;
            }
        }
        if let Ok(v) = value.to_str() {
            builder = builder.header(name.as_str(), v);
        }
    }

    // Merge browser cookies (e.g. __locale set by CUI JS) with jar cookies
    // (e.g. __Secure-access_token managed by proxy). Jar wins on conflict.
    let merged_cookies = config::get_merged_cookies(&browser_cookie_header, path_and_query);
    if !merged_cookies.is_empty() {
        debug!("Sending cookies: {}", &merged_cookies[..merged_cookies.len().min(120)]);
        builder = builder.header("Cookie", &merged_cookies);
    }

    // Inject auth token (if obtained via client-side login)
    if !state.token.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", state.token));
    }

    // Read request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Failed to read request body: {}", e)))
                .unwrap();
        }
    };

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    // Send request to upstream
    let upstream_resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Proxy request failed: {} -> {}", target_url, e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Proxy request failed: {}", e)))
                .unwrap();
        }
    };

    // Build response
    let status = upstream_resp.status();
    let mut response_builder = Response::builder().status(status.as_u16());

    let is_sse = upstream_resp.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/event-stream"))
        .unwrap_or(false);

    let is_redirect = status.is_redirection();

    // Copy response headers; intercept Set-Cookie into jar, rewrite Location
    for (name, value) in upstream_resp.headers() {
        let name_str = name.as_str().to_lowercase();

        // Skip hop-by-hop headers
        if name_str == "transfer-encoding"
            || name_str == "connection"
        {
            continue;
        }

        // Process Set-Cookie: store in jar, and conditionally forward to browser.
        // Secure cookies (__Secure-*, __Host-*, Secure flag) → jar only (browser
        // rejects them on HTTP). Non-secure cookies → jar + forward sanitized
        // version to browser (so CUI JS can read __locale, lang, etc.)
        if name_str == "set-cookie" {
            if let Ok(cookie_str) = value.to_str() {
                let result = config::store_cookie(cookie_str);
                if result.is_secure {
                    debug!("Secure cookie → jar only: {}", &cookie_str[..cookie_str.len().min(80)]);
                } else if let Some(ref sanitized) = result.browser_cookie {
                    debug!("Cookie → jar + browser: {}", &sanitized[..sanitized.len().min(80)]);
                    if let Ok(hv) = HeaderValue::from_str(sanitized) {
                        response_builder = response_builder.header("set-cookie", hv);
                    }
                }
            }
            continue;
        }

        // Rewrite absolute URLs in Location header
        if is_redirect && name_str == "location" {
            if let Ok(loc) = value.to_str() {
                if loc.starts_with(&remote_base) {
                    let local_loc = loc.replacen(&remote_base, &local_base, 1);
                    response_builder = response_builder.header("location", local_loc);
                    continue;
                }
            }
        }

        response_builder = response_builder.header(name.as_str(), value.clone());
    }

    if is_sse {
        // SSE: stream without buffering
        response_builder = response_builder
            .header("Cache-Control", "no-cache")
            .header("X-Accel-Buffering", "no");

        let stream = upstream_resp.bytes_stream();
        let body = Body::from_stream(stream);
        response_builder.body(body).unwrap_or_else(|e| {
            error!("Failed to build SSE response: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to build response"))
                .unwrap()
        })
    } else {
        // Normal response: read full body
        match upstream_resp.bytes().await {
            Ok(body) => {
                let len = body.len();
                response_builder = response_builder.header("content-length", len);
                response_builder.body(Body::from(body)).unwrap_or_else(|e| {
                    error!("Failed to build response: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to build response"))
                        .unwrap()
                })
            }
            Err(e) => {
                error!("Failed to read upstream response: {}", e);
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(format!("Failed to read upstream response: {}", e)))
                    .unwrap()
            }
        }
    }
}

/// Serve CUI static files from the build output directory
async fn serve_cui_static(path: &str, cui_dist: &PathBuf) -> Response {
    // Strip /__yao_admin_root/ prefix
    let relative = path.strip_prefix("/__yao_admin_root/").unwrap_or("");
    let relative = if relative.is_empty() { "index.html" } else { relative };

    let file_path = cui_dist.join(relative);

    // Path traversal protection
    let canonical = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // File not found → serve index.html (SPA routing)
            let index = cui_dist.join("index.html");
            if !index.exists() {
                return serve_cui_not_built();
            }
            index
        }
    };

    // Ensure path is within cui_dist
    let cui_dist_canonical = cui_dist.canonicalize().unwrap_or_else(|_| cui_dist.clone());
    if !canonical.starts_with(&cui_dist_canonical) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap();
    }

    // Directory → serve index.html (SPA routing)
    let file_path = if canonical.is_file() {
        canonical
    } else {
        let index = cui_dist.join("index.html");
        if !index.exists() {
            return serve_cui_not_built();
        }
        index
    };

    match tokio::fs::read(&file_path).await {
        Ok(contents) => {
            let mime = guess_mime(&file_path);
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime)
                .header("Cache-Control", "public, max-age=3600");

            // For HTML pages (e.g. index.html), inject preference cookies
            // so CUI JavaScript can read __locale and __theme from document.cookie
            if mime.starts_with("text/html") {
                let jar = config::COOKIE_JAR.read();
                for c in jar.iter() {
                    if c.name == "__locale" || c.name == "__theme" {
                        let cookie_str = format!(
                            "{}={}; Path=/; Max-Age=31536000; SameSite=Lax",
                            c.name, c.value
                        );
                        if let Ok(hv) = HeaderValue::from_str(&cookie_str) {
                            builder = builder.header("Set-Cookie", hv);
                        }
                    }
                }
            }

            builder.body(Body::from(contents)).unwrap()
        }
        Err(e) => {
            warn!("Failed to read file: {:?} -> {}", file_path, e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to read file"))
                .unwrap()
        }
    }
}

/// Placeholder page when CUI has not been built yet
fn serve_cui_not_built() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head>
<body style="font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#f5f5f5">
<div style="text-align:center">
<h2>CUI Not Built</h2>
<p>Please run <code>npm run build-cui</code> to build CUI static assets first.</p>
</div>
</body></html>"#
        ))
        .unwrap()
}

/// Guess MIME type from file extension
fn guess_mime(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("wasm") => "application/wasm",
        Some("map") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}
