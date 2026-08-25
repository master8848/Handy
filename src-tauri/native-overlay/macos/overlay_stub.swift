// Stub for Command-Line-Tools-only toolchains (no SwiftUI panel needed).
// Provides the same @_cdecl symbols as overlay_panel.swift so Rust links
// without requiring full Xcode. All calls are no-ops.
import Foundation

@_cdecl("overlay_show")
public func overlay_show(_ cState: UnsafePointer<CChar>?, _ x: Double, _ y: Double, _ width: Double, _ height: Double) {}

@_cdecl("overlay_hide")
public func overlay_hide() {}

@_cdecl("overlay_set_levels")
public func overlay_set_levels(_ levels: UnsafePointer<Float>?, _ count: Int) {}

@_cdecl("overlay_set_stream_text")
public func overlay_set_stream_text(
    _ cCommitted: UnsafePointer<CChar>?,
    _ cTentative: UnsafePointer<CChar>?,
    _ cPhase: UnsafePointer<CChar>?,
    _ cWorkKind: UnsafePointer<CChar>?
) {}
