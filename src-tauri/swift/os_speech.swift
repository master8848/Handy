import AVFoundation
import Dispatch
import Foundation
import Speech

// macOS system speech-to-text (SFSpeechRecognizer) bridge.
// Compiled via the Cargo build script (build.rs) for macOS arm64 targets;
// recognition runs fully on-device (offline) via requiresOnDeviceRecognition.

/// Block the calling thread on `semaphore` while still allowing the main
/// queue to progress. The Speech framework can deliver its completion
/// handlers on the main queue, so a plain `wait()` on the main thread would
/// deadlock; pumping the runloop drains those blocks instead. Background
/// threads wait normally. Returns false when `timeout` elapses.
private func waitForSemaphore(_ semaphore: DispatchSemaphore, timeout: TimeInterval) -> Bool {
    if Thread.isMainThread {
        let deadline = Date(timeIntervalSinceNow: timeout)
        while semaphore.wait(timeout: .now() + 0.05) == .timedOut {
            if Date() > deadline {
                return false
            }
            RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        return true
    }
    return semaphore.wait(timeout: .now() + timeout) == .success
}

private let recognitionTimeout: TimeInterval = 300

@_cdecl("os_speech_available")
public func osSpeechAvailable() -> Int32 {
    guard #available(macOS 13.0, *) else {
        return 0
    }
    guard let recognizer = SFSpeechRecognizer() else {
        return 0
    }
    return recognizer.supportsOnDeviceRecognition ? 1 : 0
}

@_cdecl("os_speech_authorization_status")
public func osSpeechAuthorizationStatus() -> UnsafeMutablePointer<CChar>? {
    let status = SFSpeechRecognizer.authorizationStatus()
    let text: String
    switch status {
    case .notDetermined: text = "notDetermined"
    case .denied: text = "denied"
    case .restricted: text = "restricted"
    case .authorized: text = "authorized"
    @unknown default: text = "notDetermined"
    }
    return text.withCString { strdup($0) }
}

@_cdecl("os_speech_request_authorization")
public func osSpeechRequestAuthorization() -> Int32 {
    // Block until the async authorization callback resolves so C callers get a
    // synchronous answer. The handler queue is not guaranteed, hence the
    // runloop-driving wait below.
    final class AuthBox: @unchecked Sendable {
        var granted = false
    }
    let box = AuthBox()
    let semaphore = DispatchSemaphore(value: 0)
    SFSpeechRecognizer.requestAuthorization { status in
        box.granted = (status == .authorized)
        semaphore.signal()
    }
    _ = waitForSemaphore(semaphore, timeout: 60)
    return box.granted ? 1 : 0
}

@_cdecl("os_speech_transcribe_pcm")
public func osSpeechTranscribePCM(
    _ pcm: UnsafePointer<Float>?,
    _ len: Int32,
    _ sampleRate: Int32,
    _ outError: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutablePointer<CChar>? {
    func fail(_ message: String) -> UnsafeMutablePointer<CChar>? {
        outError?.pointee = message.withCString { strdup($0) }
        return nil
    }
    outError?.pointee = nil

    // On-device recognition (requiresOnDeviceRecognition) is macOS 13+.
    guard #available(macOS 13.0, *) else {
        return fail("macOS system speech recognition requires macOS 13 or newer.")
    }
    guard let pcm = pcm, len > 0, sampleRate > 0 else {
        return fail("Invalid audio buffer passed to os_speech_transcribe_pcm.")
    }
    guard let recognizer = SFSpeechRecognizer() else {
        return fail("Speech recognizer is unavailable for the current locale.")
    }
    guard recognizer.supportsOnDeviceRecognition else {
        return fail("On-device speech recognition is not supported on this device.")
    }

    // The request accepts 32-bit float deinterleaved PCM; Handy records
    // 16 kHz mono, which is also what SFSpeechAudioBufferRecognitionRequest
    // expects.
    guard
        let format = AVAudioFormat(standardFormatWithSampleRate: Double(sampleRate), channels: 1),
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(len))
    else {
        return fail("Failed to create the audio buffer for recognition.")
    }
    buffer.frameLength = AVAudioFrameCount(len)
    if let channel = buffer.floatChannelData?[0] {
        for i in 0..<Int(len) {
            channel[i] = pcm[i]
        }
    }

    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = false
    request.requiresOnDeviceRecognition = true

    final class ResultBox: @unchecked Sendable {
        var text: String?
        var error: String?
    }
    let box = ResultBox()
    let semaphore = DispatchSemaphore(value: 0)

    // Keep a strong reference to the task for the whole blocking wait;
    // releasing it cancels recognition. Errors are delivered through the
    // result handler rather than a nil return.
    let task = recognizer.recognitionTask(with: request, resultHandler: { result, error in
        if let error = error {
            box.error = error.localizedDescription
            semaphore.signal()
        } else if let result = result, result.isFinal {
            box.text = result.bestTranscription.formattedString
            semaphore.signal()
        }
    })

    request.append(buffer)
    request.endAudio()

    if !waitForSemaphore(semaphore, timeout: recognitionTimeout) {
        task.cancel()
        return fail("Speech recognition timed out.")
    }

    if let text = box.text {
        return text.withCString { strdup($0) }
    }
    return fail(box.error ?? "Unknown speech recognition error.")
}

@_cdecl("os_speech_free_string")
public func osSpeechFreeString(_ pointer: UnsafeMutablePointer<CChar>?) {
    guard let pointer = pointer else { return }
    free(pointer)
}
