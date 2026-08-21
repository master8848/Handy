use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    Extension, Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
};

pub struct ServerState {
    inner: Mutex<Option<ServerInner>>,
}

pub(crate) struct ServerInner {
    pub(crate) port: u16,
    pub(crate) shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.inner.lock().ok()?.as_ref().map(|s| s.port)
    }

    pub fn set(&self, port: u16, tx: tokio::sync::oneshot::Sender<()>) {
        if let Ok(mut g) = self.inner.lock() {
            *g = Some(ServerInner {
                port,
                shutdown_tx: tx,
            });
        }
    }

    pub(crate) fn take(&self) -> Option<ServerInner> {
        self.inner.lock().ok()?.take()
    }
}

pub fn local_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

fn resolve_dist_dir(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(res_dir) = app.path().resource_dir() {
        let candidate = res_dir.join("dist");
        if candidate.join("index.html").exists() {
            return Some(candidate);
        }
        let candidate2 = res_dir.join("_up_/dist");
        if candidate2.join("index.html").exists() {
            return Some(candidate2);
        }
    }
    let manifest_dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist");
    if manifest_dist.join("index.html").exists() {
        return Some(manifest_dist);
    }
    let cwd_dist = PathBuf::from("dist");
    if cwd_dist.join("index.html").exists() {
        return Some(cwd_dist);
    }
    let cwd_dist2 = PathBuf::from("../dist");
    if cwd_dist2.join("index.html").exists() {
        return Some(cwd_dist2);
    }
    None
}

fn is_public_path(path: &str, method: &Method) -> bool {
    if *method != Method::GET && *method != Method::HEAD {
        return false;
    }
    if path == "/" || path == "/index.html" || path == "/manifest.json" {
        return true;
    }
    if path == "/api/health" {
        return true;
    }
    if path.starts_with("/assets/") {
        return true;
    }
    false
}

async fn auth_middleware(
    Extension(token): Extension<Option<String>>,
    req: Request,
    next: Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    if is_public_path(&path, &method) {
        return next.run(req).await;
    }
    let Some(expected) = token else {
        return next.run(req).await;
    };
    if expected.is_empty() {
        return next.run(req).await;
    }
    let auth_ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("Bearer {}", expected))
        .unwrap_or(false);
    if auth_ok {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    }
}

fn build_router(dist_dir: Option<PathBuf>, port: u16, token: Option<String>) -> Router {
    let health_port = port;
    let mut router: Router = Router::new().route(
        "/api/health",
        get(move || async move {
            axum::Json(serde_json::json!({ "status": "ok", "port": health_port }))
        }),
    );

    if let Some(dir) = dist_dir.clone() {
        let index = dir.join("index.html");
        let serve_dir = ServeDir::new(&dir).fallback(ServeFile::new(&index));
        router = router.fallback_service(serve_dir);
    } else {
        let fallback = get(|| async {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Handy server: frontend not built. Run `bun run build` and restart.",
            )
        });
        router = router.fallback(fallback);
    }

    let allowed = {
        let port_str = port.to_string();
        vec![
            format!("http://127.0.0.1:{port_str}"),
            format!("http://localhost:{port_str}"),
        ]
    };
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _req| {
            if let Ok(s) = origin.to_str() {
                allowed.iter().any(|o| o == s)
            } else {
                false
            }
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    router
        .layer(Extension(token.clone()))
        .layer(middleware::from_fn(auth_middleware))
        .layer(cors)
}

async fn bind_with_retry(
    bind: &str,
    preferred: u16,
) -> Result<(tokio::net::TcpListener, u16), String> {
    for port in preferred..=preferred + 10 {
        let addr: SocketAddr = format!("{bind}:{port}")
            .parse()
            .map_err(|e| format!("invalid bind {bind}:{port}: {e}"))?;
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => return Ok((l, port)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(format!("failed to bind {addr}: {e}")),
        }
    }
    Err(format!(
        "port {preferred} in use and no free port in {}..{}",
        preferred,
        preferred + 10
    ))
}

pub async fn start_server(app: AppHandle) -> Result<u16, String> {
    if let Some(state) = app.try_state::<ServerState>() {
        if let Some(p) = state.port() {
            return Ok(p);
        }
    }

    let settings = crate::settings::get_settings(&app);
    let bind = "127.0.0.1";
    let preferred = if settings.server_port == 0 {
        17373
    } else {
        settings.server_port
    };

    let token = {
        let mut s = settings.clone();
        let token = s.server_auth_token.clone();
        if token.is_none() {
            let new_token = crate::settings::generate_server_auth_token();
            s.server_auth_token = Some(new_token.clone());
            crate::settings::write_settings(&app, s);
            Some(new_token)
        } else {
            token
        }
    };

    let dist_dir = resolve_dist_dir(&app);
    if dist_dir.is_none() {
        log::warn!("Server dist not found; serving 503 for non-health routes");
    } else {
        log::info!("Server serving dist: {:?}", dist_dir);
    }

    let (listener, actual_port) = bind_with_retry(bind, preferred).await?;
    log::info!("Handy server listening on {bind}:{actual_port}");

    let router = build_router(dist_dir, actual_port, token);

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    if let Some(state) = app.try_state::<ServerState>() {
        state.set(actual_port, tx);
    }

    let app_for_shutdown = app.clone();
    tokio::spawn(async move {
        let server = axum::serve(listener, router);
        let graceful = server.with_graceful_shutdown(async {
            let _ = rx.await;
            log::info!("Handy server shutting down");
        });
        if let Err(e) = graceful.await {
            log::error!("Handy server error: {e}");
        }
        if let Some(state) = app_for_shutdown.try_state::<ServerState>() {
            let _ = state.take();
        }
    });

    crate::tray::update_tray_menu(&app, None);

    Ok(actual_port)
}

pub fn stop_server(app: &AppHandle) {
    if let Some(state) = app.try_state::<ServerState>() {
        if let Some(inner) = state.take() {
            let _ = inner.shutdown_tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_public_path;
    use axum::http::Method;

    #[test]
    fn public_paths_are_correct() {
        assert!(is_public_path("/", &Method::GET));
        assert!(is_public_path("/index.html", &Method::GET));
        assert!(is_public_path("/manifest.json", &Method::GET));
        assert!(is_public_path("/api/health", &Method::GET));
        assert!(is_public_path("/assets/main-abc.js", &Method::GET));
        assert!(!is_public_path("/api/other", &Method::GET));
        assert!(!is_public_path("/", &Method::POST));
    }
}
