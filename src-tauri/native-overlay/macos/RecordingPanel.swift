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
