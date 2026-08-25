//! Windows system speech-to-text backend (`os_speech_win`).
//!
//! Lets Handy transcribe through the OS speech stack instead of a Whisper
//! model. Three entry points, stubbed out on non-Windows platforms:
//!
//! - [`available`]: probes the OS speech recognizer stack.
//! - [`supports_on_device`]: best-effort report of whether recognition runs
//!   fully on-device (vs. a cloud service).
//! - [`transcribe_wav_file`]: transcribes a WAV file.
//!
//! # Why SAPI, not WinRT, for file transcription
//!
//! The WinRT `Windows.Media.SpeechRecognition.SpeechRecognizer` has **no
//! stream or file input** in the public API surface: `RecognizeStreamAsync`
//! does not exist in the Windows SDK metadata (and therefore cannot exist in
//! the `windows` 0.61.3 crate bindings either). Its only recognition inputs
//! are the live microphone (`RecognizeAsync`, `RecognizeWithUIAsync`, and the
//! continuous-recognition session). So file transcription goes through the
//! SAPI 5 in-process recognizer that the WinRT one is built on, using the
//! documented "WAV file input" flow (`ISpStream::BindToFile` + in-proc
//! recognizer + dictation grammar; see "Using WAV File Input with SR
//! Engines" on Microsoft Learn). The in-proc recognizer is fully local —
//! no network, no microphone — which is exactly the offline property we want.
//!
//! The WinRT `SpeechRecognizer` is still used for availability probing and
//! for reading the system speech language in [`supports_on_device`].

#[cfg(any(target_os = "windows", test))]
use std::path::Path;

/// Read a WAV file, validating that it is 16-bit integer PCM, and return its
/// sample rate plus the raw little-endian 16-bit sample bytes.
///
/// Any channel count and sample rate are accepted (the OS recognizer
/// converts); Handy always writes 16 kHz mono, but being lenient here makes
/// the module usable with external recordings too.
#[cfg(any(target_os = "windows", test))]
pub fn wav_to_pcm16(path: &Path) -> Result<(u32, Vec<u8>), String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(format!(
            "unsupported WAV format ({} Hz / {} ch / {}-bit {:?}): expected 16-bit integer PCM",
            spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
        ));
    }
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<Vec<i16>, _>>()
        .map_err(|e| format!("failed to decode {}: {}", path.display(), e))?;
    Ok((spec.sample_rate, i16_samples_to_le_bytes(&samples)))
}

/// Convert i16 samples to their little-endian byte representation.
#[cfg(any(target_os = "windows", test))]
fn i16_samples_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{wav_to_pcm16, Path};
    use log::{debug, info, warn};
    use std::ffi::c_void;
    use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
    use windows::Media::SpeechRecognition::SpeechRecognizer;
    use windows::Win32::{
        Foundation::{HANDLE, RPC_E_CHANGED_MODE, WAIT_OBJECT_0},
        Media::Speech::{
            ISpRecoResult, ISpRecognizer, ISpStream, SpInprocRecognizer, SpStream,
            SPEI_END_SR_STREAM, SPEI_RECOGNITION, SPEI_RESERVED1, SPEI_RESERVED2,
            SPET_LPARAM_IS_POINTER, SPEVENT, SPFM_OPEN_READONLY, SPLO_STATIC, SPRS_ACTIVE,
            SPRS_INACTIVE,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
            COINIT_MULTITHREADED,
        },
        System::Threading::WaitForSingleObject,
    };
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    /// The on-disk `SPEVENT` layout, as defined by `sapi.h` (24 bytes):
    /// `wParam` at offset 8, `lParam` at offset 16.
    ///
    /// The `windows` crate's `SPEVENT` projection does NOT match this ABI: it
    /// models the C `union` between the bitfield struct and
    /// `ullAudioStreamOffset` as a separate sequential field, producing a
    /// 32-byte struct with `wParam`/`lParam` shifted 8 bytes — reading events
    /// back through it (or through an array of it) yields garbage. We read
    /// into this exact-layout mirror instead and pass it to `GetEvents`
    /// through a pointer cast.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RawSpevent {
        /// `eEventId:16 | elParamType:16 | ulStreamNum:32` (union arm).
        bitfield: u64,
        wparam: usize,
        lparam: isize,
    }

    impl RawSpevent {
        fn event_id(&self) -> i32 {
            (self.bitfield & 0xFFFF) as i32
        }

        fn param_type(&self) -> i32 {
            ((self.bitfield >> 16) & 0xFFFF) as i32
        }
    }

    /// SAPI's `SPFEI` macro: `(1 << SPEI_ord) | SPFEI_FLAGCHECK`. The flag
    /// check bits are reserved flags that must always be included in an
    /// event-interest mask; they are derived from the same constants the
    /// macro uses rather than hardcoded.
    fn spfei(spei: i32) -> u64 {
        let flagcheck = (1u64 << SPEI_RESERVED1.0) | (1u64 << SPEI_RESERVED2.0);
        (1u64 << spei) | flagcheck
    }

    /// Runs `f` on a dedicated COM-initialized MTA thread.
    ///
    /// SAPI and WinRT objects are apartment-bound, and the caller (e.g. the
    /// Tauri main thread) may run in an apartment we do not control; a fresh
    /// MTA thread also guarantees we balance our own `CoUninitialize`. Returns
    /// `None` if the thread panicked.
    fn run_on_mta<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        std::thread::Builder::new()
            .name("os-speech-win".to_string())
            .spawn(f)
            .ok()?
            .join()
            .ok()
    }

    /// Balances a successful `CoInitializeEx` with `CoUninitialize` on drop.
    struct ComInit {
        initialized: bool,
    }

    impl ComInit {
        /// Initializes the current thread for COM in multithreaded mode, the
        /// apartment the WinRT/SAPI async machinery expects.
        fn new() -> Self {
            // CoInitializeEx returns a raw HRESULT here (S_FALSE is a
            // meaningful success, so the projection cannot map it to Result).
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            // S_OK (fresh init) and S_FALSE (already initialized) both leave
            // us with a refcount to release; RPC_E_CHANGED_MODE (thread was
            // initialized STA) does not. Worker threads we spawn are always
            // fresh, so the STA case only guards against misuse.
            let initialized = if hr.is_ok() {
                true
            } else if hr == RPC_E_CHANGED_MODE {
                warn!("os_speech_win: COM already initialized as STA on this thread");
                false
            } else {
                warn!("os_speech_win: CoInitializeEx failed: {hr}");
                false
            };
            Self { initialized }
        }
    }

    impl Drop for ComInit {
        fn drop(&mut self) {
            if self.initialized {
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Report whether an OS speech recognizer is available at all.
    ///
    /// True when either half of the OS speech stack is usable: the WinRT
    /// `SpeechRecognizer` (creates and compiles the dictation constraints)
    /// and/or the SAPI 5 in-proc recognizer with a loadable dictation grammar
    /// (the actual file-transcription path). Both are probed and logged;
    /// [`transcribe_wav_file`] requires the SAPI half and fails with a
    /// descriptive error if it is missing.
    pub fn available() -> bool {
        let sapi = run_on_mta(sapi_dictation_available).unwrap_or(false);
        let winrt = run_on_mta(winrt_recognizer_compiles).unwrap_or(false);
        debug!("os_speech_win availability: sapi={sapi} winrt={winrt}");
        sapi || winrt
    }

    /// Best-effort report of whether recognition runs fully on-device.
    ///
    /// Windows 11 22H2+ enables on-device recognition for supported languages
    /// and installs the engine under `%WINDIR%\Speech_OneCore\Engines\SR\
    /// <lang-tag>`; Windows 10 installs the same directories for its optional
    /// offline dictation packs. There is no public API to query on-device
    /// status, so the engine directory for the system speech language is the
    /// signal (conservative: Win11 22H2+ on-device languages always ship the
    /// directory, and Win10 offline packs show up too). The OS build number
    /// is logged for context.
    pub fn supports_on_device() -> bool {
        let present = run_on_mta(on_device_engine_present).unwrap_or(false);
        info!("os_speech_win on-device probe: {present}");
        present
    }

    /// Transcribe a WAV file via the SAPI 5 in-proc recognizer.
    ///
    /// The file is validated and read once up front (fast, descriptive
    /// failures), then fed to SAPI from disk via `ISpStream::BindToFile` with
    /// the dictation grammar active. Recognition is fully local.
    pub fn transcribe_wav_file(path: &Path) -> Result<String, String> {
        let (sample_rate, pcm) = wav_to_pcm16(path)?;
        let audio_secs = (pcm.len() / 2) as f64 / f64::from(sample_rate.max(1));
        let path = path.to_path_buf();
        run_on_mta(move || unsafe { sapi_transcribe_file(&path, audio_secs) })
            .unwrap_or_else(|| Err("Windows speech worker thread failed".to_string()))
    }

    fn winrt_recognizer_compiles() -> bool {
        let _com = ComInit::new();
        // Creating the recognizer and compiling the (empty) constraint set —
        // which compiles the default dictation grammar — exercises activation
        // plus speech-pack availability.
        let recognizer = match SpeechRecognizer::new() {
            Ok(r) => r,
            Err(e) => {
                debug!("os_speech_win: WinRT SpeechRecognizer unavailable: {e}");
                return false;
            }
        };
        let operation = match recognizer.CompileConstraintsAsync() {
            Ok(op) => op,
            Err(e) => {
                debug!("os_speech_win: CompileConstraintsAsync failed: {e}");
                return false;
            }
        };
        match operation.get() {
            Ok(_) => true,
            Err(e) => {
                debug!("os_speech_win: constraint compilation failed: {e}");
                false
            }
        }
    }

    fn sapi_dictation_available() -> bool {
        let _com = ComInit::new();
        unsafe {
            let recognizer: ISpRecognizer =
                match CoCreateInstance(&SpInprocRecognizer, None, CLSCTX_ALL) {
                    Ok(r) => r,
                    Err(e) => {
                        debug!("os_speech_win: SAPI recognizer unavailable: {e}");
                        return false;
                    }
                };
            let context = match recognizer.CreateRecoContext() {
                Ok(c) => c,
                Err(e) => {
                    debug!("os_speech_win: CreateRecoContext failed: {e}");
                    return false;
                }
            };
            let grammar = match context.CreateGrammar(0) {
                Ok(g) => g,
                Err(e) => {
                    debug!("os_speech_win: CreateGrammar failed: {e}");
                    return false;
                }
            };
            // LoadDictation is the real capability probe: it fails when no
            // speech engine or language pack is installed for the system
            // language.
            match grammar.LoadDictation(PCWSTR::null(), SPLO_STATIC) {
                Ok(()) => true,
                Err(e) => {
                    debug!("os_speech_win: SAPI dictation unavailable: {e}");
                    false
                }
            }
        }
    }

    fn on_device_engine_present() -> bool {
        let _com = ComInit::new();
        let build = current_build_number();
        let lang_tag = SpeechRecognizer::SystemSpeechLanguage()
            .ok()
            .and_then(|lang| lang.LanguageTag().ok())
            .map(|tag| tag.to_string());
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let engine_root = Path::new(&windir)
            .join("Speech_OneCore")
            .join("Engines")
            .join("SR");
        let engine_dir_for_language = lang_tag
            .as_deref()
            .map(|tag| engine_root.join(tag).is_dir())
            .unwrap_or(false);
        let any_engine_dir = read_dir_is_dir(&engine_root);
        info!(
            "os_speech_win on-device probe: build={build:?} language={lang_tag:?} engine_root={} dir_for_language={engine_dir_for_language} any_engine_dir={any_engine_dir}",
            engine_root.display()
        );
        engine_dir_for_language || (lang_tag.is_none() && any_engine_dir)
    }

    fn read_dir_is_dir(root: &Path) -> bool {
        std::fs::read_dir(root)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            })
            .unwrap_or(false)
    }

    fn current_build_number() -> Option<u32> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .ok()?;
        let build: String = key.get_value("CurrentBuild").ok()?;
        build.parse::<u32>().ok()
    }

    /// Runs the SAPI in-proc dictation session over the WAV file and returns
    /// the recognized phrases joined into one string.
    unsafe fn sapi_transcribe_file(path: &Path, audio_secs: f64) -> Result<String, String> {
        let _com = ComInit::new();

        // 1. Stream over the WAV file. Passing no format GUID makes SAPI read
        //    the format from the WAV header itself.
        let stream: ISpStream = CoCreateInstance(&SpStream, None, CLSCTX_ALL)
            .map_err(|e| format!("failed to create SAPI file stream: {e}"))?;
        let wav_path = HSTRING::from(path.to_string_lossy().as_ref());
        stream
            .BindToFile(&wav_path, SPFM_OPEN_READONLY, None, None, 0)
            .map_err(|e| format!("failed to open {} as SAPI input: {e}", path.display()))?;

        // 2. In-process (local) recognizer fed from the file stream. The
        //    in-proc recognizer never touches the microphone.
        let recognizer: ISpRecognizer = CoCreateInstance(&SpInprocRecognizer, None, CLSCTX_ALL)
            .map_err(|e| format!("failed to create SAPI recognizer: {e}"))?;
        recognizer
            .SetInput(&stream, true)
            .map_err(|e| format!("failed to attach WAV input to recognizer: {e}"))?;

        // 3. Recognition context signalled through a Win32 event, so no
        //    window or message pump is needed.
        let context = recognizer
            .CreateRecoContext()
            .map_err(|e| format!("failed to create recognition context: {e}"))?;
        context
            .SetNotifyWin32Event()
            .map_err(|e| format!("failed to arm SAPI event notification: {e}"))?;
        let notify_event: HANDLE = context.GetNotifyEventHandle();
        let interest = spfei(SPEI_RECOGNITION.0) | spfei(SPEI_END_SR_STREAM.0);
        context
            .SetInterest(interest, interest)
            .map_err(|e| format!("failed to set SAPI event interest: {e}"))?;

        // 4. Dictation grammar, activated. This is the same local dictation
        //    engine family the WinRT recognizer drives.
        let grammar = context
            .CreateGrammar(0)
            .map_err(|e| format!("failed to create dictation grammar: {e}"))?;
        grammar
            .LoadDictation(PCWSTR::null(), SPLO_STATIC)
            .map_err(|e| format!("failed to load dictation grammar: {e}"))?;
        grammar
            .SetDictationState(SPRS_ACTIVE)
            .map_err(|e| format!("failed to activate dictation: {e}"))?;

        // 5. Event loop until the end-of-stream event (file fully consumed)
        //    or a generous timeout — the documented guidance is a timeout
        //    greater than the audio length, capped so a wedged engine cannot
        //    strand the worker forever.
        let mut phrases: Vec<String> = Vec::new();
        let mut end_of_stream = false;
        let timeout_ms = (audio_secs * 1000.0) as u32 + 120_000;
        let mut events = [RawSpevent::default(); 32];
        while !end_of_stream {
            if WaitForSingleObject(notify_event, timeout_ms) != WAIT_OBJECT_0 {
                warn!(
                    "os_speech_win: timed out waiting for SAPI events ({} ms)",
                    timeout_ms
                );
                break;
            }
            let mut fetched = 0u32;
            context
                .GetEvents(
                    events.len() as u32,
                    events.as_mut_ptr() as *mut SPEVENT,
                    &mut fetched,
                )
                .map_err(|e| format!("failed to read SAPI events: {e}"))?;
            for event in events.iter().take(fetched as usize) {
                let id = event.event_id();
                if id == SPEI_RECOGNITION.0 {
                    if event.param_type() == SPET_LPARAM_IS_POINTER.0 {
                        if let Some(text) = reco_result_text(event.lparam) {
                            phrases.push(text);
                        }
                    }
                } else if id == SPEI_END_SR_STREAM.0 {
                    debug!("os_speech_win: end-of-stream event");
                    end_of_stream = true;
                }
            }
        }

        // Best-effort teardown: deactivate dictation so the engine stops
        // consuming the input before we drop the recognizer.
        let _ = grammar.SetDictationState(SPRS_INACTIVE);
        let text = phrases.join(" ");
        debug!(
            "os_speech_win: transcribed {} phrases ({} chars) from {}",
            phrases.len(),
            text.len(),
            path.display()
        );
        Ok(text)
    }

    /// Extract the recognized text from a `SPEI_RECOGNITION` event payload.
    ///
    /// SAPI hands ownership of the result pointer (event `lParam`) to the
    /// event consumer, so it is wrapped in a new interface that Releases it
    /// when dropped. Text comes back through `GetText` in a CoTaskMem buffer.
    unsafe fn reco_result_text(lparam: isize) -> Option<String> {
        let result = ISpRecoResult::from_raw(lparam as *mut c_void);
        let mut text_ptr: PWSTR = PWSTR::null();
        // ulCount = u32::MAX means "to the end of the phrase".
        if result
            .GetText(0, u32::MAX, true, &mut text_ptr, None)
            .is_err()
        {
            return None;
        }
        let text = pwstr_to_string(text_ptr);
        CoTaskMemFree(Some(text_ptr.0 as *const c_void));
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Read a NUL-terminated wide string into a `String`.
    unsafe fn pwstr_to_string(ptr: PWSTR) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let len = ptr.len();
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr.as_ptr(), len))
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
mod imp {
    use std::path::Path;

    /// The OS speech backend only exists on Windows.
    pub fn available() -> bool {
        false
    }

    /// The OS speech backend only exists on Windows.
    pub fn supports_on_device() -> bool {
        false
    }

    /// The OS speech backend only exists on Windows.
    pub fn transcribe_wav_file(_path: &Path) -> Result<String, String> {
        Err("Windows system speech recognition is only available on Windows".to_string())
    }
}

// The Windows backend is only invoked from commands/os_speech.rs, which is
// cfg-gated away on non-Windows builds; keep the re-export for a uniform API.
#[cfg_attr(not(target_os = "windows"), allow(unused_imports))]
pub use imp::{available, supports_on_device, transcribe_wav_file};

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the tempdir alongside the path; dropping the guard deletes the
    /// WAV, so callers must keep it alive for the duration of the assertion.
    fn write_test_wav(
        spec: hound::WavSpec,
        samples: &[i16],
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.wav");
        let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
        for sample in samples {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
        (dir, path)
    }

    fn test_spec() -> hound::WavSpec {
        hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }
    }

    #[test]
    fn i16_samples_to_le_bytes_encodes_little_endian() {
        let bytes = i16_samples_to_le_bytes(&[1, -2, 32767, -32768, 0]);
        assert_eq!(bytes, vec![1, 0, 0xFE, 0xFF, 0xFF, 0x7F, 0x00, 0x80, 0, 0]);
    }

    #[test]
    fn wav_to_pcm16_roundtrips_handy_format() {
        let (_dir, path) = write_test_wav(test_spec(), &[1000, -1000, 42]);
        let (rate, pcm) = wav_to_pcm16(&path).expect("wav_to_pcm16");
        assert_eq!(rate, 16_000);
        assert_eq!(pcm, i16_samples_to_le_bytes(&[1000, -1000, 42]));
    }

    #[test]
    fn wav_to_pcm16_accepts_other_sample_rates() {
        let mut spec = test_spec();
        spec.sample_rate = 44_100;
        let (_dir, path) = write_test_wav(spec, &[7, 8]);
        let (rate, pcm) = wav_to_pcm16(&path).expect("wav_to_pcm16");
        assert_eq!(rate, 44_100);
        assert_eq!(pcm, i16_samples_to_le_bytes(&[7, 8]));
    }

    #[test]
    fn wav_to_pcm16_rejects_float_pcm() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("float.wav");
        let mut spec = test_spec();
        spec.sample_format = hound::SampleFormat::Float;
        spec.bits_per_sample = 32;
        let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
        writer.write_sample(0.5f32).expect("write sample");
        writer.finalize().expect("finalize wav");
        assert!(wav_to_pcm16(&path).is_err());
    }

    #[test]
    fn wav_to_pcm16_rejects_missing_file() {
        assert!(wav_to_pcm16(Path::new("no-such-file.wav")).is_err());
    }

    #[cfg(not(target_os = "windows"))]
    mod fallback {
        use super::*;

        #[test]
        fn fallback_backend_is_unavailable() {
            assert!(!available());
            assert!(!supports_on_device());
        }

        #[test]
        fn fallback_transcribe_fails_with_message() {
            let err = transcribe_wav_file(Path::new("anything.wav")).expect_err("should fail");
            assert!(!err.is_empty());
        }
    }
}
