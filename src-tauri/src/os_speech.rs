//! macOS system speech-to-text (SFSpeechRecognizer) backend.
//!
//! Wraps the Swift bridge compiled by `build.rs` (`swift/os_speech.swift`,
//! mirroring the Apple Intelligence bridge pattern). Recognition runs fully
//! on-device (offline) via `requiresOnDeviceRecognition`. When the build
//! linked the stub (Command Line Tools-only toolchain) or the target is not
//! macOS, every operation reports "not available".

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::path::Path;

#[cfg(target_os = "macos")]
extern "C" {
    fn os_speech_available() -> c_int;
    fn os_speech_authorization_status() -> *mut c_char;
    fn os_speech_request_authorization() -> c_int;
    fn os_speech_transcribe_pcm(
        pcm: *const f32,
        len: c_int,
        sample_rate: c_int,
        out_error: *mut *mut c_char,
    ) -> *mut c_char;
    fn os_speech_free_string(ptr: *mut c_char);
}

#[cfg(target_os = "macos")]
fn native_available() -> bool {
    unsafe { os_speech_available() == 1 }
}

#[cfg(not(target_os = "macos"))]
fn native_available() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn native_authorization_status() -> String {
    let ptr = unsafe { os_speech_authorization_status() };
    if ptr.is_null() {
        return "notDetermined".to_string();
    }
    let status = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { os_speech_free_string(ptr) };
    status
}

#[cfg(not(target_os = "macos"))]
fn native_authorization_status() -> String {
    "notDetermined".to_string()
}

#[cfg(target_os = "macos")]
fn native_request_authorization() -> bool {
    unsafe { os_speech_request_authorization() == 1 }
}

#[cfg(not(target_os = "macos"))]
fn native_request_authorization() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn native_transcribe_pcm(pcm: &[f32], sample_rate: u32) -> Result<String, String> {
    let mut error_ptr: *mut c_char = std::ptr::null_mut();
    let result_ptr = unsafe {
        os_speech_transcribe_pcm(
            pcm.as_ptr(),
            pcm.len() as c_int,
            sample_rate as c_int,
            &mut error_ptr,
        )
    };
    unsafe {
        if !result_ptr.is_null() {
            let text = CStr::from_ptr(result_ptr).to_string_lossy().into_owned();
            os_speech_free_string(result_ptr);
            Ok(text)
        } else if error_ptr.is_null() {
            Err("Speech recognition failed".to_string())
        } else {
            let message = CStr::from_ptr(error_ptr).to_string_lossy().into_owned();
            os_speech_free_string(error_ptr);
            Err(message)
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn native_transcribe_pcm(_pcm: &[f32], _sample_rate: u32) -> Result<String, String> {
    log::debug!(
        "os_speech: transcription requested but macOS Speech is not available on this platform"
    );
    Err("macOS system speech recognition is not available on this platform".to_string())
}

/// Whether the macOS on-device speech recognizer is available. Returns false
/// when the Swift bridge is a stub or the platform is not macOS.
pub fn available() -> bool {
    native_available()
}

/// Current speech-recognition authorization: one of
/// "notDetermined"/"denied"/"restricted"/"authorized".
pub fn authorization_status() -> String {
    native_authorization_status()
}

/// Prompt the user for speech-recognition permission, blocking until the
/// system answers. Returns true when authorized.
pub fn request_authorization() -> bool {
    native_request_authorization()
}

/// Transcribe raw PCM audio (16 kHz mono float32, Handy's capture format).
///
/// BLOCKING: this call blocks the current thread for the duration of
/// recognition (it pumps the main runloop internally when needed) and must be
/// invoked on a background thread (e.g. `tauri::async_runtime::spawn_blocking`),
/// never on the UI thread.
pub fn transcribe_pcm(pcm: &[f32], sample_rate: u32) -> Result<String, String> {
    if sample_rate == 0 || sample_rate > c_int::MAX as u32 {
        return Err(format!("Invalid sample rate: {sample_rate}"));
    }
    if pcm.len() > c_int::MAX as usize {
        return Err(format!("Audio buffer too large: {} samples", pcm.len()));
    }
    native_transcribe_pcm(pcm, sample_rate)
}

/// Transcribe a 16 kHz mono 16-bit PCM WAV file.
///
/// Reads the WAV via `hound`, normalizes samples to f32, then forwards them to
/// [`transcribe_pcm`] — the same BLOCKING caveat applies.
pub fn transcribe_wav_file(path: &Path) -> Result<String, String> {
    let (samples, sample_rate) = read_wav_pcm(path)?;
    transcribe_pcm(&samples, sample_rate)
}

/// Read a 16 kHz mono 16-bit PCM WAV file into normalized f32 samples.
///
/// Handy only ever saves 16 kHz mono 16-bit PCM WAVs (see
/// `audio_toolkit::audio::utils::save_wav_file`), and the Speech framework
/// expects 16 kHz audio, so anything else is rejected up front.
fn read_wav_pcm(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let reader = hound::WavReader::open(path).map_err(|e| format!("cannot open WAV file: {e}"))?;
    let spec = reader.spec();
    if spec.sample_rate != crate::audio_toolkit::constants::WHISPER_SAMPLE_RATE
        || spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(format!(
            "expected 16 kHz mono 16-bit PCM WAV, got {} Hz / {} ch / {}-bit {:?}",
            spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
        ));
    }
    let samples = reader
        .into_samples::<i16>()
        .map(|s| {
            s.map(|v| v as f32 / i16::MAX as f32)
                .map_err(|e| format!("failed to read WAV sample: {e}"))
        })
        .collect::<Result<Vec<f32>, String>>()?;
    Ok((samples, spec.sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_wav_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("os_speech_test_{}_{name}.wav", std::process::id()))
    }

    fn write_test_wav(path: &Path, samples: &[i16], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &sample in samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn stub_toolchain_reports_unavailable() {
        // On Command-Line-Tools-only toolchains the stub bridge is linked and
        // errors with "not available in this build"; with the real bridge an
        // empty buffer fails validation instead. Distinguish the two so this
        // test passes on both CLT-only and full-Xcode machines.
        let err = transcribe_pcm(&[], 16000).expect_err("empty audio must be rejected");
        if err.contains("not available in this build") {
            assert!(
                !available(),
                "stub bridge must report the recognizer as unavailable"
            );
            assert_eq!(authorization_status(), "notDetermined");
            assert!(!request_authorization());
        } else {
            assert!(matches!(
                authorization_status().as_str(),
                "notDetermined" | "denied" | "restricted" | "authorized"
            ));
        }
    }

    #[test]
    fn transcribe_pcm_rejects_bad_sample_rate() {
        assert!(transcribe_pcm(&[0.0; 160], 0).is_err());
    }

    #[test]
    fn read_wav_pcm_normalizes_i16_samples() {
        let path = temp_wav_path("normalize");
        write_test_wav(&path, &[0, 16384, -16384, i16::MAX, i16::MIN], 16000);
        let (samples, sample_rate) = read_wav_pcm(&path).unwrap();
        assert_eq!(sample_rate, 16000);
        let expected = vec![
            0.0,
            16384.0 / i16::MAX as f32,
            -16384.0 / i16::MAX as f32,
            1.0,
            i16::MIN as f32 / i16::MAX as f32,
        ];
        assert_eq!(samples, expected);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn read_wav_pcm_rejects_non_16k_sample_rate() {
        let path = temp_wav_path("wrong_rate");
        write_test_wav(&path, &[1, 2, 3], 44100);
        let err = read_wav_pcm(&path).unwrap_err();
        assert!(err.contains("expected 16 kHz mono 16-bit PCM WAV"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn read_wav_pcm_rejects_missing_file() {
        let err = read_wav_pcm(Path::new("/nonexistent/does_not_exist.wav")).unwrap_err();
        assert!(err.contains("cannot open WAV file"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn transcribe_wav_file_not_available_off_macos() {
        let path = temp_wav_path("off_macos");
        write_test_wav(&path, &[0, 0], 16000);
        let err = transcribe_wav_file(&path).unwrap_err();
        assert!(err.contains("not available"));
        let _ = std::fs::remove_file(path);
    }
}
