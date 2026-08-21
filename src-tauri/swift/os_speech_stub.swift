import Foundation

// Stub implementation for the macOS system speech-to-text bridge.
// This file is compiled via Cargo build script when the build environment
// cannot build the real SFSpeechRecognizer path (e.g. Command Line Tools
// only, mirroring apple_intelligence_stub.swift).

@_cdecl("os_speech_available")
public func osSpeechAvailable() -> Int32 {
    return 0
}

@_cdecl("os_speech_authorization_status")
public func osSpeechAuthorizationStatus() -> UnsafeMutablePointer<CChar>? {
    return strdup("notDetermined")
}

@_cdecl("os_speech_request_authorization")
public func osSpeechRequestAuthorization() -> Int32 {
    return 0
}

@_cdecl("os_speech_transcribe_pcm")
public func osSpeechTranscribePCM(
    _ pcm: UnsafePointer<Float>?,
    _ len: Int32,
    _ sampleRate: Int32,
    _ outError: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutablePointer<CChar>? {
    outError?.pointee = strdup(
        "macOS speech recognition is not available in this build (SDK requirement not met)."
    )
    return nil
}

@_cdecl("os_speech_free_string")
public func osSpeechFreeString(_ pointer: UnsafeMutablePointer<CChar>?) {
    guard let pointer = pointer else { return }
    free(pointer)
}
