// PromptClear — offline voice notes. MIT.

use std::collections::HashSet;
use std::sync::mpsc::Sender;

use rdev::{Event, EventType, Key};

use crate::events::Hotkey;

/// Spawn the global-hotkey listener on its own thread. Alt+Space toggles
/// dictation. If the OS denies global capture (permissions), the app keeps
/// working through the in-app shortcuts and the failure is logged.
pub fn spawn(tx: Sender<Hotkey>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let mut modifiers: HashSet<Key> = HashSet::new();
        let mut space_down = false;

        let listener = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => {
                if is_modifier(key) {
                    modifiers.insert(key);
                } else if key == Key::Space {
                    if !space_down && alt_held(&modifiers) {
                        let _ = tx.send(Hotkey::ToggleRecording);
                        ctx.request_repaint();
                    }
                    space_down = true;
                }
            }
            EventType::KeyRelease(key) => {
                if is_modifier(key) {
                    modifiers.remove(&key);
                } else if key == Key::Space {
                    space_down = false;
                }
            }
            _ => {}
        };

        if let Err(error) = rdev::listen(listener) {
            log::error!("global hotkey listener failed: {error:?}");
        }
    });
}

fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::Alt
            | Key::AltGr
            | Key::ControlLeft
            | Key::ControlRight
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::MetaLeft
            | Key::MetaRight
    )
}

fn alt_held(modifiers: &HashSet<Key>) -> bool {
    modifiers.contains(&Key::Alt) || modifiers.contains(&Key::AltGr)
}
