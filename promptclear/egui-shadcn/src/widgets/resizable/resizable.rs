//! Resizable builder struct — a horizontal split panel.

/// A resizable split panel with a draggable handle.
#[must_use]
pub struct Resizable {
    pub(crate) height: f32,
}

impl Resizable {
    /// `initial_fraction` is the initial width ratio of the left panel (0.0..1.0).
    pub fn new(_initial_fraction: f32) -> Self {
        Self { height: 200.0 }
    }

    /// Sets the panel height in points.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}
