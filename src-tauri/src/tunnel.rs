use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRequest, Request};
use axum::response::Response;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use http::{HeaderValue, StatusCode};
use reqwest::Client;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info};

use crate::config;

const IDLE_TTL_SECS: u64 = 300; // 5 minutes
const REAPER_INTERVAL_SECS: u64 = 30;
const MAX_TUNNELS: usize = 20;

pub struct TunnelManager {
    tunnels: Arc<Mutex<HashMap<u16, TunnelInfo>>>,
    client: Client,
}

struct TunnelInfo {
    local_port: u16,
    last_access: Arc<AtomicU64>,
    active_ws: Arc<AtomicUsize>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl TunnelManager {
    pub fn new(client: Client) -> Self {
        let tunnels = Arc::new(Mutex::new(HashMap::new()));
        Self::start_reaper(tunnels.clone());
        Self { tunnels, client }
    }

    pub async fn get_or_create(&self, remote_port: u16) -> Result<u16, String> {
        let mut map = self.tunnels.lock().await;

        // Return existing tunnel if alive
        if let Some(info) = map.get(&remote_port) {
            info.last_access.store(now_secs(), Ordering::Relaxed);
            return Ok(info.local_port);
        }

        // Enforce max tunnel limit
        if map.len() >= MAX_TUNNELS {
            return Err(format!("tunnel limit reached (max {})", MAX_TUNNELS));
        }

        // Resolve remote host from proxy state
        let state = config::get_proxy_state();
        let server_url = url::Url::parse(&state.server_url)
            .map_err(|e| format!("invalid server_url: {}", e))?;
        let remote_host = server_url.host_str().unwrap_or("127.0.0.1").to_string();
        let scheme = server_url.scheme().to_string();

        // Bind to random available port
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("failed to bind tunnel port: {}", e))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| format!("failed to get local addr: {}", e))?
            .port();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let last_access = Arc::new(AtomicU64::new(now_secs()));
        let active_ws = Arc::new(AtomicUsize::new(0));

        let client = self.client.clone();
        let tunnels_ref = self.tunnels.clone();
        let la = last_access.clone();
        let aws = active_ws.clone();

        tokio::spawn(run_tunnel_server(
            listener,
            client,
            remote_host,
            scheme,
            remote_port,
            la,
            aws.clone(),
            shutdown_rx,
            tunnels_ref,
            remote_port,
        ));

        info!(
            "Tunnel created: 127.0.0.1:{} -> remote:{}",
            local_port, remote_port
        );

        map.insert(
            remote_port,
            TunnelInfo {
                local_port,
                last_access,
                active_ws,
                shutdown_tx: Some(shutdown_tx),
            },
        );

        Ok(local_port)
    }

    pub async fn tunnel_count(&self) -> usize {
        self.tunnels.lock().await.len()
    }

    pub async fn shutdown_all(&self) {
        let mut map = self.tunnels.lock().await;
        for (port, mut info) in map.drain() {
            if let Some(tx) = info.shutdown_tx.take() {
                let _ = tx.send(());
            }
            info!("Tunnel shutdown: remote_port={}", port);
        }
    }

    fn start_reaper(tunnels: Arc<Mutex<HashMap<u16, TunnelInfo>>>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(REAPER_INTERVAL_SECS)).await;
                let now = now_secs();
                let mut map = tunnels.lock().await;
                let mut to_remove = Vec::new();

                for (port, info) in map.iter() {
                    let ws_count = info.active_ws.load(Ordering::Relaxed);
                    let last = info.last_access.load(Ordering::Relaxed);
                    if ws_count == 0 && (now - last) > IDLE_TTL_SECS {
                        to_remove.push(*port);
                    }
                }

                for port in to_remove {
                    if let Some(mut info) = map.remove(&port) {
                        if let Some(tx) = info.shutdown_tx.take() {
                            let _ = tx.send(());
                        }
                        info!("Tunnel reaped (idle): remote_port={}", port);
                    }
                }
            }
        });
    }
}

async fn run_tunnel_server(
    listener: TcpListener,
    client: Client,
    remote_host: String,
    scheme: String,
    remote_port: u16,
    last_access: Arc<AtomicU64>,
    active_ws: Arc<AtomicUsize>,
    shutdown_rx: oneshot::Receiver<()>,
    tunnels: Arc<Mutex<HashMap<u16, TunnelInfo>>>,
    tunnel_key: u16,
) {
    let la = last_access.clone();
    let aws = active_ws.clone();
    let rh = remote_host.clone();
    let sc = scheme.clone();

    let app = Router::new()
        .fallback(move |req: Request| {
            let client = client.clone();
            let remote_host = rh.clone();
            let scheme = sc.clone();
            let la = la.clone();
            let aws = aws.clone();
            async move {
                handle_tunnel_request_inner(
                    req,
                    client,
                    &remote_host,
                    &scheme,
                    remote_port,
                    la,
                    aws,
                )
                .await
            }
        })
        .layer(CorsLayer::very_permissive());

    let graceful = async move {
        let _ = shutdown_rx.await;
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(graceful)
        .await
    {
        error!("Tunnel server error (port {}): {}", remote_port, e);
    }

    // Clean up from map on exit (in case not already removed by reaper)
    let mut map = tunnels.lock().await;
    map.remove(&tunnel_key);
}

async fn handle_tunnel_request_inner(
    req: Request,
    client: Client,
    remote_host: &str,
    scheme: &str,
    remote_port: u16,
    last_access: Arc<AtomicU64>,
    active_ws: Arc<AtomicUsize>,
) -> Response {
    last_access.store(now_secs(), Ordering::Relaxed);

    // WebSocket upgrade
    if is_websocket_upgrade(&req) {
        return handle_tunnel_ws(req, remote_host, scheme, remote_port, last_access, active_ws)
            .await;
    }

    // Regular HTTP proxy
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let target_url = format!("{}://{}:{}{}", scheme, remote_host, remote_port, path_and_query);
    debug!("Tunnel proxy: {} {}", method, target_url);

    let state = config::get_proxy_state();
    let mut builder = client.request(method, &target_url);

    // Copy headers (skip hop-by-hop)
    for (name, value) in req.headers() {
        let name_str = name.as_str().to_lowercase();
        if name_str == "host"
            || name_str == "connection"
            || name_str == "transfer-encoding"
            || name_str == "upgrade"
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            builder = builder.header(name.as_str(), v);
        }
    }

    // Inject Bearer token
    if !state.token.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", state.token));
    }

    // Read and forward body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 512 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Failed to read body: {}", e)))
                .unwrap();
        }
    };
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes);
    }

    let upstream_resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Tunnel request failed: {} -> {}", target_url, e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Tunnel request failed: {}", e)))
                .unwrap();
        }
    };

    // Build response
    let status = upstream_resp.status();
    let mut resp_builder = Response::builder().status(status);

    for (name, value) in upstream_resp.headers() {
        let n = name.as_str().to_lowercase();
        if n == "transfer-encoding" || n == "connection" {
            continue;
        }
        resp_builder = resp_builder.header(name, value);
    }

    let body_bytes = upstream_resp.bytes().await.unwrap_or_default();
    resp_builder.body(Body::from(body_bytes)).unwrap()
}

fn is_websocket_upgrade(req: &Request) -> bool {
    req.headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}

async fn handle_tunnel_ws(
    req: Request,
    remote_host: &str,
    scheme: &str,
    remote_port: u16,
    last_access: Arc<AtomicU64>,
    active_ws: Arc<AtomicUsize>,
) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let ws_scheme = if scheme == "https" { "wss" } else { "ws" };
    let remote_ws_url = format!(
        "{}://{}:{}{}",
        ws_scheme, remote_host, remote_port, path_and_query
    );

    info!("Tunnel WS: {}", remote_ws_url);

    let state = config::get_proxy_state();
    let token = state.token.clone();

    // Merge cookies from jar for upstream auth
    let browser_cookie_header = req
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let merged_cookies = config::get_merged_cookies(&browser_cookie_header, path_and_query);

    // Extract subprotocols
    let subprotocols: Vec<String> = req
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let ws_upgrade = match WebSocketUpgrade::from_request(req, &()).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Tunnel WS upgrade failed: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("WS upgrade failed: {}", e)))
                .unwrap();
        }
    };

    let ws_upgrade = if !subprotocols.is_empty() {
        ws_upgrade.protocols(subprotocols)
    } else {
        ws_upgrade
    };

    ws_upgrade.on_upgrade(move |client_ws| async move {
        active_ws.fetch_add(1, Ordering::Relaxed);
        last_access.store(now_secs(), Ordering::Relaxed);

        if let Err(e) = tunnel_ws_bridge(client_ws, &remote_ws_url, &token, &merged_cookies).await {
            debug!("Tunnel WS bridge ended: {}", e);
        }

        let prev = active_ws.fetch_sub(1, Ordering::Relaxed);
        last_access.store(now_secs(), Ordering::Relaxed);

        // WS count dropped to 0 → mark for immediate reap by setting last_access to epoch
        if prev == 1 {
            last_access.store(0, Ordering::Relaxed);
        }
    })
}

async fn tunnel_ws_bridge(
    client_ws: WebSocket,
    remote_url: &str,
    token: &str,
    cookies: &str,
) -> Result<(), String> {
    use tokio_tungstenite::connect_async_tls_with_config;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let mut request = remote_url
        .into_client_request()
        .map_err(|e| format!("Invalid WS URL: {}", e))?;

    let headers = request.headers_mut();
    if !cookies.is_empty() {
        if let Ok(v) = HeaderValue::from_str(cookies) {
            headers.insert("cookie", v);
        }
    }
    if !token.is_empty() {
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert("authorization", v);
        }
    }

    let (remote_ws, _resp) = connect_async_tls_with_config(request, None, false, None)
        .await
        .map_err(|e| format!("Failed to connect remote WS: {}", e))?;

    let (mut client_sink, mut client_stream) = client_ws.split();
    let (mut remote_sink, mut remote_stream) = remote_ws.split();

    let mut client_to_remote = tokio::spawn(async move {
        while let Some(msg) = client_stream.next().await {
            match msg {
                Ok(axum_msg) => {
                    if let Some(m) = axum_to_tungstenite(axum_msg) {
                        if remote_sink.send(m).await.is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        let _ = remote_sink.close().await;
    });

    let mut remote_to_client = tokio::spawn(async move {
        while let Some(msg) = remote_stream.next().await {
            match msg {
                Ok(tung_msg) => {
                    if let Some(m) = tungstenite_to_axum(tung_msg) {
                        if client_sink.send(m).await.is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        let _ = client_sink.close().await;
    });

    tokio::select! {
        _ = &mut client_to_remote => { remote_to_client.abort(); }
        _ = &mut remote_to_client => { client_to_remote.abort(); }
    }

    Ok(())
}

fn axum_to_tungstenite(msg: AxumMessage) -> Option<TungsteniteMessage> {
    match msg {
        AxumMessage::Text(t) => Some(TungsteniteMessage::Text(t.to_string().into())),
        AxumMessage::Binary(b) => Some(TungsteniteMessage::Binary(b)),
        AxumMessage::Ping(p) => Some(TungsteniteMessage::Ping(p)),
        AxumMessage::Pong(p) => Some(TungsteniteMessage::Pong(p)),
        AxumMessage::Close(_) => Some(TungsteniteMessage::Close(None)),
    }
}

fn tungstenite_to_axum(msg: TungsteniteMessage) -> Option<AxumMessage> {
    match msg {
        TungsteniteMessage::Text(t) => Some(AxumMessage::Text(t.to_string().into())),
        TungsteniteMessage::Binary(b) => Some(AxumMessage::Binary(b.into())),
        TungsteniteMessage::Ping(p) => Some(AxumMessage::Ping(p.into())),
        TungsteniteMessage::Pong(p) => Some(AxumMessage::Pong(p.into())),
        TungsteniteMessage::Close(_) => Some(AxumMessage::Close(None)),
        TungsteniteMessage::Frame(_) => None,
    }
}
