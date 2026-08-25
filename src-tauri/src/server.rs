use axum::{
    
    extract::{Multipart, Request},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{net::SocketAddr, path::PathBuf, sync::Mutex, time::Instant};
use tauri::{AppHandle, Manager};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
};

const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;
const OPENAPI_JSON: &str = include_str!("openapi.json");

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
    if path == "/"
        || path == "/index.html"
        || path == "/manifest.json"
        || path == "/api/health"
        || path == "/api/models"
        || path == "/v1/models"
        || path == "/api/model/status"
        || path == "/openapi.json"
        || path == "/api/openapi.json"
        || path == "/api/docs"
    {
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    let body = serde_json::json!({ "error": message, "code": code });
    (status, axum::Json(body)).into_response()
}

fn openai_error(status: StatusCode, code: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "error": { "message": message, "type": "invalid_request_error", "code": code }
    });
    (status, axum::Json(body)).into_response()
}

fn openai_error_with_type(status: StatusCode, type_str: &str, code: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "error": { "message": message, "type": type_str, "code": code }
    });
    (status, axum::Json(body)).into_response()
}

fn map_os_error_message(msg: &str) -> Option<(StatusCode, &'static str, String)> {
    if msg.contains("os_speech_auth_required") {
        Some((
            StatusCode::FORBIDDEN,
            "os_speech_auth_required",
            "OS speech auth required. Open System Settings → Privacy & Security → Speech Recognition and enable Handy.".to_string(),
        ))
    } else if msg.contains("not available") || msg.contains("not_available") {
        Some((
            StatusCode::SERVICE_UNAVAILABLE,
            "os_speech_not_available",
            "OS speech recognition is not available on this device.".to_string(),
        ))
    } else {
        None
    }
}

fn check_policy_allows_load(app: &AppHandle) -> Option<Response> {
    let settings = crate::settings::get_settings(app);
    match settings.api_model_load_policy {
        crate::settings::ApiModelLoadPolicy::AutoAllow => None,
        crate::settings::ApiModelLoadPolicy::AlwaysAsk => Some(openai_error_with_type(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "model_load_requires_confirmation",
            "Model load requires confirmation. Preload the model in Handy UI or set API policy to auto_allow.",
        )),
        crate::settings::ApiModelLoadPolicy::Never => Some(openai_error_with_type(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "model_load_not_allowed",
            "API model loading is disabled (policy: never). Preload a model in Handy UI.",
        )),
    }
}

fn is_os_speech_model(id: &str) -> bool {
    id == crate::managers::model::OS_SPEECH_MODEL_ID
}

// ---------------------------------------------------------------------------
// Handlers: health / models / status
// ---------------------------------------------------------------------------

async fn handle_openapi() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        OPENAPI_JSON,
    )
}

async fn handle_list_models_native(Extension(app): Extension<AppHandle>) -> impl IntoResponse {
    let mgr = app.state::<std::sync::Arc<crate::managers::model::ModelManager>>();
    let mut models = mgr.get_available_models();
    if !models.iter().any(|m| m.id == crate::managers::model::OS_SPEECH_MODEL_ID) {
        let available = {
            #[cfg(target_os = "macos")]
            { crate::os_speech::available() }
            #[cfg(target_os = "windows")]
            { crate::os_speech_win::available() }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            { false }
        };
        models.push(crate::managers::model::ModelInfo {
            id: crate::managers::model::OS_SPEECH_MODEL_ID.to_string(),
            name: {
                #[cfg(target_os = "macos")] { "macOS Speech Recognition".to_string() }
                #[cfg(target_os = "windows")] { "Windows Speech Recognition".to_string() }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))] { "OS Speech Recognition".to_string() }
            },
            description: "Uses the device's built-in speech recognition — no download needed".to_string(),
            filename: String::new(),
            source: crate::managers::model::ModelSource::Local,
            size_mb: 0,
            is_downloaded: available,
            is_downloading: false,
            partial_size: 0,
            is_directory: false,
            engine_type: crate::managers::model::EngineType::OsSpeech,
            accuracy_score: 0.0,
            speed_score: 0.0,
            supports_translation: false,
            is_recommended: false,
            supported_languages: vec!["auto".to_string()],
            supports_language_selection: false,
            is_custom: false,
            supports_streaming: false,
            supports_language_detection: true,
        });
    }
    axum::Json(models).into_response()
}

async fn handle_list_models_openai(Extension(app): Extension<AppHandle>) -> impl IntoResponse {
    let mgr = app.state::<std::sync::Arc<crate::managers::model::ModelManager>>();
    let mut models = mgr.get_available_models();
    if !models.iter().any(|m| m.id == crate::managers::model::OS_SPEECH_MODEL_ID) {
        let available = {
            #[cfg(target_os = "macos")]
            { crate::os_speech::available() }
            #[cfg(target_os = "windows")]
            { crate::os_speech_win::available() }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            { false }
        };
        // push synthetic for openai list (created timestamp will map it too)
        models.push(crate::managers::model::ModelInfo {
            id: crate::managers::model::OS_SPEECH_MODEL_ID.to_string(),
            name: "OS Speech".to_string(),
            description: String::new(),
            filename: String::new(),
            source: crate::managers::model::ModelSource::Local,
            size_mb: 0,
            is_downloaded: available,
            is_downloading: false,
            partial_size: 0,
            is_directory: false,
            engine_type: crate::managers::model::EngineType::OsSpeech,
            accuracy_score: 0.0,
            speed_score: 0.0,
            supports_translation: false,
            is_recommended: false,
            supported_languages: vec!["auto".to_string()],
            supports_language_selection: false,
            is_custom: false,
            supports_streaming: false,
            supports_language_detection: true,
        });
        let _ = available;
    }
    let now = chrono::Utc::now().timestamp() as u64;
    let data: Vec<serde_json::Value> = models
        .iter()
        .map(|m| {
            serde_json::json!({ "id": m.id, "object": "model", "created": now, "owned_by": "handy" })
        })
        .collect();
    axum::Json(serde_json::json!({ "object": "list", "data": data })).into_response()
}

#[derive(Serialize)]
struct ModelStatusResponse {
    is_loaded: bool,
    current_model: Option<String>,
    loaded_models: Vec<String>,
    is_loading: bool,
}

async fn handle_model_status(Extension(app): Extension<AppHandle>) -> impl IntoResponse {
    let tm = app.state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>();
    let status = ModelStatusResponse {
        is_loaded: tm.is_model_loaded(),
        current_model: tm.get_current_model(),
        loaded_models: tm.get_loaded_models(),
        is_loading: tm.is_loading_flag(),
    };
    axum::Json(status).into_response()
}

#[derive(Deserialize)]
struct LoadRequest {
    model: String,
    device_index: Option<usize>,
}

async fn handle_model_load(
    Extension(app): Extension<AppHandle>,
    axum::Json(payload): axum::Json<LoadRequest>,
) -> Response {
    if payload.model.trim().is_empty() {
        return openai_error(StatusCode::BAD_REQUEST, "invalid_request", "model is required");
    }
    if let Some(resp) = check_policy_allows_load(&app) {
        return resp;
    }
    let model_id = payload.model.trim().to_string();
    let device_index = payload.device_index;

    if is_os_speech_model(&model_id) {
        let available = {
            #[cfg(target_os = "macos")]
            { crate::os_speech::available() }
            #[cfg(target_os = "windows")]
            { crate::os_speech_win::available() }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            { false }
        };
        if !available {
            return openai_error(StatusCode::SERVICE_UNAVAILABLE, "os_speech_not_available", "OS speech recognition is not available on this device.");
        }
        #[cfg(target_os = "macos")]
        if crate::os_speech::authorization_status() != "authorized" {
            return openai_error(StatusCode::FORBIDDEN, "os_speech_auth_required", "OS speech auth required. Open System Settings → Privacy & Security → Speech Recognition and enable Handy.");
        }
    } else {
        let mgr = app.state::<std::sync::Arc<crate::managers::model::ModelManager>>();
        let info = mgr.get_model_info(&model_id);
        let Some(info) = info else {
            return openai_error(StatusCode::NOT_FOUND, "model_not_found", &format!("Model not found: {}", model_id));
        };
        if !info.is_downloaded {
            return openai_error(StatusCode::NOT_FOUND, "model_not_downloaded", &format!("Model not downloaded: {}", model_id));
        }
    }

    let tm = app.state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>().inner().clone();
    let model_id_clone = model_id.clone();
    let result = tokio::task::spawn_blocking(move || tm.load_model_with_device(&model_id_clone, device_index)).await;
    match result {
        Ok(Ok(())) => axum::Json(serde_json::json!({ "status": "ok", "model": model_id })).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if let Some((status, code, mapped)) = map_os_error_message(&msg) {
                return openai_error(status, code, &mapped);
            }
            openai_error(StatusCode::INTERNAL_SERVER_ERROR, "model_load_failed", &msg)
        }
        Err(e) => openai_error(StatusCode::INTERNAL_SERVER_ERROR, "model_load_failed", &format!("Load task panicked: {}", e)),
    }
}

#[derive(Deserialize)]
struct UnloadRequest {
    model: Option<String>,
}

async fn handle_model_unload(
    Extension(app): Extension<AppHandle>,
    body: Option<axum::Json<UnloadRequest>>,
) -> Response {
    if let Some(resp) = check_policy_allows_load(&app) {
        return resp;
    }
    let tm = app.state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>().inner().clone();
    let target = body.and_then(|b| b.0.model).map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let result = tokio::task::spawn_blocking(move || {
        if let Some(id) = target {
            tm.unload_model_by_id(&id).map_err(|e| e.to_string())
        } else {
            tm.unload_model().map_err(|e| e.to_string())
        }
    })
    .await;
    match result {
        Ok(Ok(())) => axum::Json(serde_json::json!({ "status": "ok" })).into_response(),
        Ok(Err(e)) => openai_error(StatusCode::INTERNAL_SERVER_ERROR, "unload_failed", &e),
        Err(e) => openai_error(StatusCode::INTERNAL_SERVER_ERROR, "unload_failed", &format!("Unload panicked: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Transcribe helpers
// ---------------------------------------------------------------------------

struct ParsedMultipart {
    file_bytes: Vec<u8>,
    file_ext: String,
    model: Option<String>,
    language: Option<String>,
    translate: Option<bool>,
    #[allow(dead_code)]
    prompt: Option<String>,
    response_format: Option<String>,
}

async fn parse_multipart(mut multipart: Multipart) -> Result<ParsedMultipart, Response> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut language: Option<String> = None;
    let mut translate: Option<bool> = None;
    let mut prompt: Option<String> = None;
    let mut response_format: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| openai_error(StatusCode::BAD_REQUEST, "invalid_request", &format!(" multipart error: {}", e)))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                let bytes = field.bytes().await.map_err(|e| openai_error(StatusCode::BAD_REQUEST, "invalid_request", &format!("file read failed: {}", e)))?;
                if bytes.len() > MAX_UPLOAD_BYTES {
                    return Err(openai_error(StatusCode::PAYLOAD_TOO_LARGE, "file_too_large", "File exceeds 100 MB limit"));
                }
                if bytes.is_empty() {
                    return Err(openai_error(StatusCode::BAD_REQUEST, "invalid_request", "file is empty"));
                }
                file_bytes = Some(bytes.to_vec());
            }
            "model" => {
                let v = field.text().await.unwrap_or_default().trim().to_string();
                if !v.is_empty() {
                    model = Some(v);
                }
            }
            "language" => {
                let v = field.text().await.unwrap_or_default().trim().to_string();
                if !v.is_empty() {
                    language = Some(v);
                }
            }
            "translate" => {
                let v = field.text().await.unwrap_or_default().trim().to_lowercase();
                translate = Some(v == "true" || v == "1");
            }
            "prompt" => {
                let v = field.text().await.unwrap_or_default().trim().to_string();
                if !v.is_empty() {
                    prompt = Some(v);
                }
            }
            "response_format" => {
                let v = field.text().await.unwrap_or_default().trim().to_lowercase();
                if !v.is_empty() {
                    response_format = Some(v);
                }
            }
            "temperature" | "timestamp_granularities[]" | "timestamp_granularities" => {
                let _ = field.text().await;
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let file_bytes = match file_bytes {
        Some(b) => b,
        None => return Err(openai_error(StatusCode::BAD_REQUEST, "invalid_request", "file is required")),
    };
    let ext = file_name
        .as_deref()
        .and_then(|n| n.rsplit('.').next())
        .filter(|e| e.len() <= 8 && !e.is_empty())
        .unwrap_or("wav")
        .to_string();

    Ok(ParsedMultipart {
        file_bytes,
        file_ext: ext,
        model,
        language,
        translate,
        prompt,
        response_format,
    })
}

async fn transcribe_via_api(
    app: AppHandle,
    parsed: ParsedMultipart,
    force_translate: Option<bool>,
    is_native: bool,
) -> Response {
    let settings = crate::settings::get_settings(&app);
    let target_model = parsed
        .model
        .clone()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| settings.selected_model.clone());
    if target_model.trim().is_empty() {
        return openai_error(StatusCode::BAD_REQUEST, "invalid_request", "model is required (no model selected)");
    }
    let target_model = target_model.trim().to_string();

    // Validate model exists / downloaded (os-speech always exposed as single id)
    if is_os_speech_model(&target_model) {
        let available = {
            #[cfg(target_os = "macos")]
            { crate::os_speech::available() }
            #[cfg(target_os = "windows")]
            { crate::os_speech_win::available() }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            { false }
        };
        if !available {
            return openai_error(StatusCode::SERVICE_UNAVAILABLE, "os_speech_not_available", "OS speech recognition is not available on this device.");
        }
        #[cfg(target_os = "macos")]
        if crate::os_speech::authorization_status() != "authorized" {
            return openai_error(StatusCode::FORBIDDEN, "os_speech_auth_required", "OS speech auth required. Open System Settings → Privacy & Security → Speech Recognition and enable Handy.");
        }
    } else {
        let mgr = app.state::<std::sync::Arc<crate::managers::model::ModelManager>>();
        if let Some(info) = mgr.get_model_info(&target_model) {
            if !info.is_downloaded {
                return openai_error(StatusCode::NOT_FOUND, "model_not_found", &format!("Model not downloaded: {}", target_model));
            }
        } else {
            return openai_error(StatusCode::NOT_FOUND, "model_not_found", &format!("Model not found: {}", target_model));
        }
    }

    // Lazy load check
    let tm = app.state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>().inner().clone();
    let is_loaded = tm.is_model_loaded_id(&target_model) || (tm.get_current_model().as_deref() == Some(target_model.as_str()) && tm.is_model_loaded());
    if !is_loaded {
        if !settings.api_lazy_transcribe {
            return openai_error(StatusCode::SERVICE_UNAVAILABLE, "model_not_loaded", "Model not loaded and lazy transcribe is disabled. Load the model first via POST /api/model/load.");
        }
        if let Some(resp) = check_policy_allows_load(&app) {
            return resp;
        }
        // Auto-load
        let tm_clone = tm.clone();
        let mid = target_model.clone();
        let load_res = tokio::task::spawn_blocking(move || tm_clone.load_model_with_device(&mid, None)).await;
        match load_res {
            Ok(Ok(())) => {},
            Ok(Err(e)) => {
                let msg = e.to_string();
                if let Some((status, code, mapped)) = map_os_error_message(&msg) {
                    return openai_error(status, code, &mapped);
                }
                return openai_error(StatusCode::SERVICE_UNAVAILABLE, "model_load_failed", &format!("Failed to lazy-load model '{}': {}", target_model, msg));
            }
            Err(e) => return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "model_load_failed", &format!("Load panicked: {}", e)),
        }
    }

    // Write upload to temp file and decode to 16k mono f32
    let file_bytes = parsed.file_bytes;
    let file_ext = parsed.file_ext;
    let decode_result = tokio::task::spawn_blocking(move || {
        let mut tmp = tempfile::Builder::new()
            .prefix("handy-api-")
            .suffix(&format!(".{}", file_ext))
            .tempfile()
            .map_err(|e| format!("tempfile failed: {}", e))?;
        use std::io::Write;
        tmp.write_all(&file_bytes).map_err(|e| format!("write temp failed: {}", e))?;
        tmp.flush().map_err(|e| format!("flush temp failed: {}", e))?;
        let path = tmp.path().to_path_buf();
        // keep file alive while decoding
        let samples = crate::audio_toolkit::decode_audio_file_to_samples(&path).map_err(|e| e.to_string())?;
        // tmp dropped here, file removed
        Ok::<Vec<f32>, String>(samples)
    })
    .await;

    let samples = match decode_result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return openai_error(StatusCode::BAD_REQUEST, "decode_failed", &e),
        Err(e) => return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "decode_failed", &format!("decode panicked: {}", e)),
    };

    if samples.is_empty() {
        return openai_error(StatusCode::UNPROCESSABLE_ENTITY, "no_speech", "No audio could be decoded");
    }

    // For OS speech, need a temp WAV file path
    let needs_wav = is_os_speech_model(&target_model);
    let wav_path: Option<PathBuf> = if needs_wav {
        let samples_clone = samples.clone();
        let res = tokio::task::spawn_blocking(move || crate::audio_toolkit::save_temp_wav_file(&samples_clone).map_err(|e| e.to_string())).await;
        match res {
            Ok(Ok(p)) => Some(p),
            Ok(Err(e)) => return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", &format!("Failed to create temp WAV: {}", e)),
            Err(e) => return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", &format!("WAV task panicked: {}", e)),
        }
    } else {
        None
    };

    // Determine translate override: force_translate wins, else parsed.translate vs settings
    let should_translate = force_translate.or(parsed.translate).unwrap_or(settings.translate_to_english);
    let should_override_translate = should_translate != settings.translate_to_english;
    let language_override = parsed.language.clone().filter(|l| !l.is_empty() && l != &settings.selected_language);

    // If language/translate overrides are needed, temporarily update settings under a blocking task
    // We do settings override inside the transcribe blocking task to keep it atomic.
    let start = Instant::now();
    let tm2 = tm.clone();
    let app_clone = app.clone();
    let transcribe_result = tokio::task::spawn_blocking(move || {
        // Apply overrides if needed
        let mut original_settings: Option<crate::settings::AppSettings> = None;
        if should_override_translate || language_override.is_some() {
            let mut s = crate::settings::get_settings(&app_clone);
            original_settings = Some(s.clone());
            if should_override_translate {
                s.translate_to_english = should_translate;
            }
            if let Some(lang) = language_override.clone() {
                s.selected_language = lang;
            }
            crate::settings::write_settings(&app_clone, s);
        }
        let res = tm2.transcribe_with_wav_path(samples.clone(), wav_path.as_deref());
        // Restore original settings
        if let Some(orig) = original_settings {
            crate::settings::write_settings(&app_clone, orig);
        }
        if let Some(p) = wav_path {
            let _ = std::fs::remove_file(p);
        }
        res.map_err(|e| e.to_string())
    })
    .await;

    let text = match transcribe_result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            if let Some((status, code, mapped)) = map_os_error_message(&e) {
                return openai_error(status, code, &mapped);
            }
            return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "transcription_failed", &e);
        }
        Err(e) => return openai_error(StatusCode::INTERNAL_SERVER_ERROR, "transcription_failed", &format!("transcribe panicked: {}", e)),
    };

    if text.trim().is_empty() {
        return openai_error(StatusCode::UNPROCESSABLE_ENTITY, "no_speech", "No speech detected");
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    let fmt = parsed.response_format.as_deref().unwrap_or("json").to_lowercase();
    match fmt.as_str() {
        "text" => {
            (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response()
        }
        "verbose_json" => {
            let body = serde_json::json!({
                "task": "transcribe",
                "language": settings.selected_language,
                "duration": duration_ms as f64 / 1000.0,
                "text": text,
            });
            (StatusCode::OK, axum::Json(body)).into_response()
        }
        _ => {
            if is_native {
                let body = serde_json::json!({ "text": text, "model": target_model, "language": settings.selected_language, "duration_ms": duration_ms });
                (StatusCode::OK, axum::Json(body)).into_response()
            } else {
                let body = serde_json::json!({ "text": text });
                (StatusCode::OK, axum::Json(body)).into_response()
            }
        }
    }
}

async fn handle_transcribe_native(
    Extension(app): Extension<AppHandle>,
    multipart: Multipart,
) -> Response {
    let parsed = match parse_multipart(multipart).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    transcribe_via_api(app, parsed, None, true).await
}

async fn handle_transcribe_openai(
    Extension(app): Extension<AppHandle>,
    multipart: Multipart,
) -> Response {
    let mut parsed = match parse_multipart(multipart).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if parsed.language.as_deref() == Some("auto") {
        parsed.language = None;
    }
    transcribe_via_api(app, parsed, Some(false), false).await
}

async fn handle_translate_openai(
    Extension(app): Extension<AppHandle>,
    multipart: Multipart,
) -> Response {
    let mut parsed = match parse_multipart(multipart).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if parsed.language.as_deref() == Some("auto") {
        parsed.language = None;
    }
    transcribe_via_api(app, parsed, Some(true), false).await
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

fn build_router(app: AppHandle, dist_dir: Option<PathBuf>, port: u16, token: Option<String>) -> Router {
    let health_port = port;

    let mut router: Router = Router::new()
        .route("/api/health", get(move || async move { axum::Json(serde_json::json!({ "status": "ok", "port": health_port })) }))
        .route("/api/models", get(handle_list_models_native))
        .route("/v1/models", get(handle_list_models_openai))
        .route("/api/model/status", get(handle_model_status))
        .route("/api/model/load", post(handle_model_load))
        .route("/api/model/unload", post(handle_model_unload))
        .route("/api/transcribe", post(handle_transcribe_native))
        .route("/v1/audio/transcriptions", post(handle_transcribe_openai))
        .route("/v1/audio/translations", post(handle_translate_openai))
        .route("/openapi.json", get(handle_openapi))
        .route("/api/openapi.json", get(handle_openapi))
        .route("/api/docs", get(handle_openapi));

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
        .layer(middleware::from_fn(auth_middleware))
        .layer(Extension(token.clone()))
        .layer(Extension(app))
        .layer(cors)
        .layer(tower_http::limit::RequestBodyLimitLayer::new(MAX_UPLOAD_BYTES))
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

#[derive(Serialize, Type, Debug, Clone)]
pub struct BrowserServerStatus {
    pub running: bool,
    pub port: Option<u16>,
    pub url: Option<String>,
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

    let router = build_router(app.clone(), dist_dir, actual_port, token);

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

// --- Experimental "Show in Browser" runtime controls (no restart required) ---

#[tauri::command]
#[specta::specta]
pub fn get_browser_server_status(app: AppHandle) -> BrowserServerStatus {
    if let Some(state) = app.try_state::<ServerState>() {
        if let Some(port) = state.port() {
            return BrowserServerStatus {
                running: true,
                port: Some(port),
                url: Some(local_url(port)),
            };
        }
    }
    let settings = crate::settings::get_settings(&app);
    BrowserServerStatus {
        running: false,
        port: Some(settings.server_port),
        url: Some(local_url(settings.server_port)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_browser_server(app: AppHandle) -> Result<BrowserServerStatus, String> {
    {
        let mut s = crate::settings::get_settings(&app);
        if !s.server_mode_enabled {
            s.server_mode_enabled = true;
            if s.server_auth_token.is_none() {
                s.server_auth_token = Some(crate::settings::generate_server_auth_token());
            }
            crate::settings::write_settings(&app, s);
        }
    }
    let port = start_server(app.clone()).await?;
    Ok(BrowserServerStatus {
        running: true,
        port: Some(port),
        url: Some(local_url(port)),
    })
}

#[tauri::command]
#[specta::specta]
pub fn stop_browser_server(app: AppHandle) -> Result<BrowserServerStatus, String> {
    {
        let mut s = crate::settings::get_settings(&app);
        if s.server_mode_enabled {
            s.server_mode_enabled = false;
            crate::settings::write_settings(&app, s);
        }
    }
    stop_server(&app);
    crate::tray::update_tray_menu(&app, None);
    let settings = crate::settings::get_settings(&app);
    Ok(BrowserServerStatus {
        running: false,
        port: Some(settings.server_port),
        url: Some(local_url(settings.server_port)),
    })
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
        assert!(is_public_path("/api/models", &Method::GET));
        assert!(is_public_path("/v1/models", &Method::GET));
        assert!(is_public_path("/api/model/status", &Method::GET));
        assert!(is_public_path("/openapi.json", &Method::GET));
        assert!(is_public_path("/api/openapi.json", &Method::GET));
        assert!(is_public_path("/assets/main-abc.js", &Method::GET));
        assert!(!is_public_path("/api/other", &Method::GET));
        assert!(!is_public_path("/api/transcribe", &Method::GET));
        assert!(!is_public_path("/", &Method::POST));
        assert!(!is_public_path("/api/models", &Method::POST));
    }
}
