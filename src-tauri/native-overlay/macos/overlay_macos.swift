import SwiftUI
import AppKit

// Theme tokens mapped from RecordingOverlay.css
// --s-surface 98% background, --s-accent #faa2ca/#f28cbb, radius 24/16/18, etc.
// Keep values in sync with theme.css.

let WAVE_BARS = 9

struct RecordingView: View {
    var state: String
    var levels: [Float]
    var committed: String
    var tentative: String
    var phase: String
    var workKind: String
    var elapsed: Int
    var position: String // "top" or "bottom"

    var body: some View {
        // Pill + live panel share one visual language.
        // v1: flat opaque, no blur, accurate geometry, waveform 60Hz via lerp.
        // Cut for v1: wavy spell underline, scroll mask, grid 0fr→1fr animation.
        // NOTE: Color(nsColor:) requires macOS 12+; deployment target is 11.0, so
        // use platform-agnostic colors / `Color.primary` palette for v1 to keep
        // swiftc happy. Full theme token mapping can be added once target is bumped.
        let isStreaming = state == "streaming"
        VStack(spacing: 0) {
            if isStreaming {
                VStack(alignment: .leading, spacing: 0) {
                    Text(committed + (committed.isEmpty ? "" : " ") + tentative)
                        .font(.system(size: 15))
                        .italic()
                        .lineSpacing(2)
                        .foregroundColor(Color.primary.opacity(0.9))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 14)
                        .padding(.top, 12)
                }
                .frame(maxHeight: 64, alignment: .bottom)
            }
            HStack(spacing: 0) {
                Circle()
                    .fill(Color.pink)
                    .frame(width: 7, height: 7)
                    .padding(.leading, 5)
                Spacer()
                HStack(spacing: 3) {
                    ForEach(0..<WAVE_BARS, id: \.self) { i in
                        let v: Float = i < levels.count ? levels[i] : 0
                        let h = max(3, min(18, 3 + pow(Double(v), 0.7) * 15))
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Color.pink)
                            .frame(width: 4, height: CGFloat(h))
                    }
                }
                .frame(height: 18)
                Spacer()
                if isStreaming {
                    Text(String(format: "%d:%02d", elapsed / 60, elapsed % 60))
                        .font(.system(size: 12).monospacedDigit())
                        .foregroundColor(Color.secondary)
                }
                Button(action: {}) {
                    Image(systemName: "xmark")
                        .font(.system(size: 8, weight: .medium))
                        .foregroundColor(Color.secondary)
                }
                .buttonStyle(.plain)
                .frame(width: 22, height: 22)
                .background(Color.gray.opacity(0.12))
                .clipShape(Circle())
            }
            .frame(height: 40)
            .padding(.horizontal, 10)
        }
        .frame(width: isStreaming ? 392 : 216)
        .background(Color.white.opacity(0.98))
        .overlay(RoundedRectangle(cornerRadius: isStreaming ? 16 : 24).stroke(Color.gray.opacity(0.22), lineWidth: 1))
        .cornerRadius(isStreaming ? 16 : 24)
        .shadow(radius: 0)
    }
}

// Lerp helper for waveform smoothing: prev*0.7 + target*0.3
// Driven by TimelineView / CADisplayLink at 60Hz in the real panel.

class OverlayState: ObservableObject {
    @Published var state: String = "recording"
    @Published var levels: [Float] = Array(repeating: 0, count: WAVE_BARS)
    @Published var committed: String = ""
    @Published var tentative: String = ""
    @Published var phase: String = "listening"
    @Published var workKind: String = "transcribing"
    @Published var elapsed: Int = 0
    @Published var position: String = "bottom"
}
private var panel: NSPanel?
private var hostingView: NSHostingView<RecordingView>?
private let state = OverlayState()

private func ensurePanel() -> NSPanel {
    if let p = panel { return p }
    let p = NSPanel(
        contentRect: NSRect(x: 0, y: 0, width: 256, height: 46),
        styleMask: [.borderless, .nonactivatingPanel],
        backing: .buffered,
        defer: false
    )
    p.isFloatingPanel = true
    p.level = .statusBar
    p.backgroundColor = .clear
    p.isOpaque = false
    p.hasShadow = false
    p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    p.isMovableByWindowBackground = false
    let view = RecordingView(
        state: state.state,
        levels: state.levels,
        committed: state.committed,
        tentative: state.tentative,
        phase: state.phase,
        workKind: state.workKind,
        elapsed: state.elapsed,
        position: state.position
    )
    let hv = NSHostingView(rootView: view)
    hv.wantsLayer = true
    p.contentView = hv
    hostingView = hv
    panel = p
    return p
}

private func updateView() {
    guard let hv = hostingView else { return }
    hv.rootView = RecordingView(
        state: state.state,
        levels: state.levels,
        committed: state.committed,
        tentative: state.tentative,
        phase: state.phase,
        workKind: state.workKind,
        elapsed: state.elapsed,
        position: state.position
    )
}

@_cdecl("overlay_show")
public func overlay_show(_ cState: UnsafePointer<CChar>?, _ x: Double, _ y: Double, _ width: Double, _ height: Double) {
    DispatchQueue.main.async {
        let s = cState.flatMap { String(cString: $0) } ?? "recording"
        state.state = s
        let p = ensurePanel()
        p.setFrame(NSRect(x: x, y: y, width: width, height: height), display: true)
        p.orderFront(nil)
        p.alphaValue = 1.0
        updateView()
    }
}

@_cdecl("overlay_hide")
public func overlay_hide() {
    DispatchQueue.main.async {
        guard let p = panel else { return }
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.2
            p.animator().alphaValue = 0
        } completionHandler: {
            p.orderOut(nil)
            p.alphaValue = 1
        }
    }
}

@_cdecl("overlay_set_levels")
public func overlay_set_levels(_ levels: UnsafePointer<Float>?, _ count: Int) {
    guard let levels = levels, count > 0 else { return }
    let arr = Array(UnsafeBufferPointer(start: levels, count: count))
    DispatchQueue.main.async {
        // Lerp smoothing: prev*0.7 + target*0.3 (mirrors RecordingOverlay.tsx)
        var smoothed = state.levels
        if smoothed.count != arr.count {
            smoothed = Array(repeating: 0, count: arr.count)
        }
        for i in 0..<min(smoothed.count, arr.count) {
            smoothed[i] = smoothed[i] * 0.7 + arr[i] * 0.3
        }
        state.levels = smoothed
        updateView()
    }
}

@_cdecl("overlay_set_stream_text")
public func overlay_set_stream_text(
    _ cCommitted: UnsafePointer<CChar>?,
    _ cTentative: UnsafePointer<CChar>?,
    _ cPhase: UnsafePointer<CChar>?,
    _ cWorkKind: UnsafePointer<CChar>?
) {
    DispatchQueue.main.async {
        if let c = cCommitted { state.committed = String(cString: c) }
        if let c = cTentative { state.tentative = String(cString: c) }
        if let c = cPhase { state.phase = String(cString: c) }
        if let c = cWorkKind { state.workKind = String(cString: c) }
        updateView()
    }
}
