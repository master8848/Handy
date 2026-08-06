//! OS-level speech recognition — the "transcribe via the OS" engine.
//!
//! On macOS this wraps the Apple Speech framework (`SFSpeechRecognizer` +
//! `SFSpeechURLRecognitionRequest`): the same recognizer behind the system's
//! built-in dictation. PCM audio is written to a temporary WAV file and handed
//! to the framework; no model download is needed. The user must grant the
//! Speech Recognition permission (System Settings → Privacy & Security →
//! Speech Recognition); the app's `Info.plist` also needs an
//! `NSSpeechRecognitionUsageDescription` key or the framework aborts.
//!
//! On every other platform the module is a stub that reports
//! [`OsSpeechError::UnsupportedPlatform`], so the rest of the crate (which is
//! not macOS-gated) keeps compiling everywhere.

use std::fmt;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// How long to wait for the system to deliver the final recognition result.
/// The Speech framework is network-backed for many locales and silently
/// retries, so the deadline is generous but bounded (matching the batch
/// streaming finalize timeout).
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to wait for the authorization prompt to be answered.
const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Errors from the OS speech-recognition engine.
#[derive(Debug)]
pub enum OsSpeechError {
    /// Compiled on a platform without the Apple Speech framework.
    UnsupportedPlatform,
    /// The user denied (or later revoked) speech recognition permission.
    PermissionDenied,
    /// Speech recognition is restricted on this device (e.g. parental controls
    /// or a managed device).
    PermissionRestricted,
    /// The recognizer could not be created, or `isAvailable` was false
    /// (offline, service limits, or an unsupported locale).
    RecognizerUnavailable,
    /// The authorization prompt did not produce an answer in time.
    AuthorizationTimeout,
    /// The system did not deliver a final result in time.
    RecognitionTimeout,
    /// The Speech framework reported an error.
    RecognitionFailed(String),
    /// Could not write or finalize the temporary WAV file.
    Wav(String),
    /// Underlying filesystem error.
    Io(std::io::Error),
}

impl fmt::Display for OsSpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsSpeechError::UnsupportedPlatform => {
                write!(f, "OS speech recognition is only available on macOS")
            }
            OsSpeechError::PermissionDenied => write!(
                f,
                "Speech recognition permission denied — enable it in System Settings → \
                 Privacy & Security → Speech Recognition"
            ),
            OsSpeechError::PermissionRestricted => {
                write!(f, "Speech recognition is restricted on this device")
            }
            OsSpeechError::RecognizerUnavailable => write!(
                f,
                "The system speech recognizer is not available (offline, service limits, \
                 or unsupported language)"
            ),
            OsSpeechError::AuthorizationTimeout => write!(
                f,
                "Timed out waiting for the speech recognition permission prompt"
            ),
            OsSpeechError::RecognitionTimeout => write!(
                f,
                "The system speech recognizer did not produce a result in time"
            ),
            OsSpeechError::RecognitionFailed(detail) => {
                write!(f, "System speech recognition failed: {}", detail)
            }
            OsSpeechError::Wav(detail) => write!(f, "Failed to write temporary WAV: {}", detail),
            OsSpeechError::Io(e) => write!(f, "OS speech recognition I/O error: {}", e),
        }
    }
}

impl std::error::Error for OsSpeechError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OsSpeechError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Request (or re-verify) speech recognition authorization. Prompts the user
/// the first time; afterwards just reflects the persisted decision.
pub fn request_authorization() -> Result<(), OsSpeechError> {
    platform::request_authorization()
}

/// Transcribe 16-bit-representable float PCM (16 kHz mono, as the pipeline
/// delivers it) via the OS speech recognizer. A temporary WAV is written into
/// `tmp_dir` and removed before returning. Returns the recognized text.
pub fn transcribe_pcm(
    pcm: &[f32],
    sample_rate: u32,
    tmp_dir: &Path,
) -> Result<String, OsSpeechError> {
    platform::transcribe_pcm(pcm, sample_rate, tmp_dir)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use hound::{WavSpec, WavWriter};
    use objc2::AnyThread;
    use objc2_foundation::{NSError, NSOperationQueue, NSString, NSURL};
    use objc2_speech::{
        SFSpeechRecognitionResult, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
        SFSpeechURLRecognitionRequest,
    };
    use std::fs;

    /// Write float PCM to a 16-bit mono WAV at `sample_rate` (the format the
    /// Speech framework's URL requests accept). Sample values saturate at the
    /// i16 range (Rust `as` casts), mirroring `crate::audio::utils::save_wav_file`.
    pub(crate) fn write_pcm_wav(
        pcm: &[f32],
        sample_rate: u32,
        path: &Path,
    ) -> Result<(), OsSpeechError> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            WavWriter::create(path, spec).map_err(|e| OsSpeechError::Wav(e.to_string()))?;
        for &sample in pcm {
            writer
                .write_sample((sample * i16::MAX as f32) as i16)
                .map_err(|e| OsSpeechError::Wav(e.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|e| OsSpeechError::Wav(e.to_string()))?;
        Ok(())
    }

    pub(super) fn request_authorization() -> Result<(), OsSpeechError> {
        // `requestAuthorization:` is async and delivers the status on an
        // arbitrary queue; block the calling (non-main) thread until it lands.
        let (tx, rx) = mpsc::channel::<SFSpeechRecognizerAuthorizationStatus>();
        let handler = block2::RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
            let _ = tx.send(status);
        });
        // SAFETY: `handler` is the required block; the framework copies it.
        unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };

        match rx.recv_timeout(AUTHORIZATION_TIMEOUT) {
            Ok(SFSpeechRecognizerAuthorizationStatus::Authorized) => Ok(()),
            Ok(SFSpeechRecognizerAuthorizationStatus::Denied) => {
                Err(OsSpeechError::PermissionDenied)
            }
            Ok(SFSpeechRecognizerAuthorizationStatus::Restricted) => {
                Err(OsSpeechError::PermissionRestricted)
            }
            Ok(_) => Err(OsSpeechError::RecognitionFailed(
                "authorization returned an unexpected status".to_string(),
            )),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(OsSpeechError::AuthorizationTimeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(OsSpeechError::AuthorizationTimeout),
        }
    }

    pub(super) fn transcribe_pcm(
        pcm: &[f32],
        sample_rate: u32,
        tmp_dir: &Path,
    ) -> Result<String, OsSpeechError> {
        let unique = format!(
            "handy_os_speech_{}_{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = tmp_dir.join(unique);
        let result = write_pcm_wav(pcm, sample_rate, &path).and_then(|()| transcribe_wav(&path));
        // Best-effort cleanup on every path — never let removal mask a result.
        let _ = fs::remove_file(&path);
        result
    }

    fn transcribe_wav(path: &Path) -> Result<String, OsSpeechError> {
        // The default recognizer follows the user's system language (with
        // dictation fallback), matching dictation behavior.
        let Some(recognizer) = (unsafe { SFSpeechRecognizer::init(SFSpeechRecognizer::alloc()) })
        else {
            return Err(OsSpeechError::RecognizerUnavailable);
        };
        if !unsafe { recognizer.isAvailable() } {
            return Err(OsSpeechError::RecognizerUnavailable);
        }

        // Route callbacks off the main queue: the default is the main queue,
        // and transcription may run from any thread.
        let queue = NSOperationQueue::new();
        // SAFETY: `queue` is a plain background operation queue.
        unsafe { recognizer.setQueue(&queue) };

        let url =
            NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&path.to_string_lossy()), false);
        let request = unsafe {
            SFSpeechURLRecognitionRequest::initWithURL(SFSpeechURLRecognitionRequest::alloc(), &url)
        };
        // We only want the single final result, not a stream of partials.
        // SAFETY: `request` is owned for the duration of the call.
        unsafe { request.setShouldReportPartialResults(false) };

        let (tx, rx) = mpsc::channel::<Result<String, OsSpeechError>>();
        let handler = block2::RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                if !error.is_null() {
                    // The framework keeps `error` alive for the call.
                    let error_ref = unsafe { &*error };
                    let _ = tx.send(Err(OsSpeechError::RecognitionFailed(
                        error_ref.localizedDescription().to_string(),
                    )));
                    return;
                }
                if !result.is_null() {
                    // The framework keeps `result` alive for the call.
                    let result_ref = unsafe { &*result };
                    if unsafe { result_ref.isFinal() } {
                        let transcription = unsafe { result_ref.bestTranscription() };
                        let text = unsafe { transcription.segments() }
                            .iter()
                            .map(|segment| {
                                // SAFETY: `segment` is owned by the array for
                                // the duration of the iteration.
                                unsafe { segment.substring() }.to_string()
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        let _ = tx.send(Ok(text));
                    }
                }
            },
        );
        // Keep the task (and thus the recognition run) alive while we wait.
        let _task =
            unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };

        match rx.recv_timeout(RECOGNITION_TIMEOUT) {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(e)) => Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(OsSpeechError::RecognitionTimeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(OsSpeechError::RecognitionTimeout),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    pub(super) fn request_authorization() -> Result<(), OsSpeechError> {
        Err(OsSpeechError::UnsupportedPlatform)
    }

    pub(super) fn transcribe_pcm(
        _pcm: &[f32],
        _sample_rate: u32,
        _tmp_dir: &Path,
    ) -> Result<String, OsSpeechError> {
        Err(OsSpeechError::UnsupportedPlatform)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use hound::WavReader;

    #[test]
    fn write_pcm_wav_roundtrips() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("handy_os_speech_test_{}.wav", std::process::id()));
        let pcm: Vec<f32> = (0..8000).map(|i| (i as f32 / 8000.0).sin()).collect();
        let sample_rate = 16_000;

        platform::write_pcm_wav(&pcm, sample_rate, &path).unwrap();

        let mut reader = WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, sample_rate);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(samples.len(), pcm.len());

        // A sine peak should round-trip to a large positive sample.
        let peak = samples.iter().max().copied().unwrap_or(0);
        assert!(peak > i16::MAX / 2, "expected a strong peak, got {}", peak);

        // 0.5 → half scale truncated toward zero (matches `save_wav_file`'s
        // `as i16` conversion): 0.5 * 32767.0 = 16383.5 → 16383.
        let probe = vec![0.5_f32, -0.5, 0.25, -0.25];
        let probe_path = dir.join(format!(
            "handy_os_speech_test_probe_{}.wav",
            std::process::id()
        ));
        platform::write_pcm_wav(&probe, sample_rate, &probe_path).unwrap();
        let mut probe_reader = WavReader::open(&probe_path).unwrap();
        let probe_samples: Vec<i16> = probe_reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(probe_samples, vec![16383, -16383, 8191, -8191]);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&probe_path);
    }

    #[test]
    fn seeding_produces_os_speech_entry() {
        let mut models = std::collections::HashMap::new();
        crate::model::ModelManager::seed_os_speech_entry(&mut models);

        let entry = models
            .get(crate::model::OS_SPEECH_MODEL_ID)
            .expect("OS speech entry should be seeded");
        assert!(matches!(
            entry.engine_type,
            crate::model::EngineType::OsSpeech
        ));
        assert!(entry.is_downloaded, "OS speech never needs a download");
        assert!(!entry.supports_streaming, "OS speech is batch-only");
        assert!(!entry.is_custom);
        assert!(entry.size_mb == 0);
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unsupported_platform() {
        assert!(matches!(
            request_authorization(),
            Err(OsSpeechError::UnsupportedPlatform)
        ));
        let tmp_dir = std::env::temp_dir();
        assert!(matches!(
            transcribe_pcm(&[], 16_000, &tmp_dir),
            Err(OsSpeechError::UnsupportedPlatform)
        ));
    }
}
