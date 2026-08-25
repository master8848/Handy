import AppKit
import SwiftUI

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
    p.level = .status
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
