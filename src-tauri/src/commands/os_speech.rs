/// OS-provided speech-to-text (SFSpeechRecognizer on macOS, SAPI dictation on
/// Windows). Both backends run off the UI thread via spawn_blocking.

#[tauri::command]
#[specta::specta]
pub fn os_speech_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::os_speech::available()
    }
    #[cfg(target_os = "windows")]
    {
        crate::os_speech_win::available()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// OS speech recognition authorization status ("notDetermined"/"denied"/
/// "restricted"/"authorized"). Windows reports "authorized" unconditionally
/// (no dedicated OS speech permission prompt).
#[tauri::command]
#[specta::specta]
pub fn os_speech_authorization_status() -> String {
    #[cfg(target_os = "macos")]
    {
        crate::os_speech::authorization_status()
    }
    #[cfg(not(target_os = "macos"))]
    {
        "authorized".to_string()
    }
}

/// Request OS speech recognition permission (macOS only; blocks until the
/// system prompt resolves). Returns true when recognition may proceed.
#[tauri::command]
#[specta::specta]
pub fn os_speech_request_authorization() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::os_speech::request_authorization()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Transcribe a 16 kHz mono 16-bit PCM WAV file with the OS speech engine.
/// Offline-first: on-device recognition is preferred on both platforms.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_os_wav(path: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(move || crate::os_speech::transcribe_wav_file(path.as_ref()))
            .await
            .map_err(|e| format!("OS speech task panicked: {e}"))?
    }
    #[cfg(target_os = "windows")]
    {
        crate::os_speech_win::transcribe_wav_file(path.as_ref())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Err("OS speech recognition is not available on this platform".to_string())
    }
}
