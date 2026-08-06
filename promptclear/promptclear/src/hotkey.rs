// PromptClear — offline voice notes. MIT.

use std::sync::mpsc::Sender;

use crate::events::Hotkey;

/// Spawn the global-hotkey listener on its own thread. Alt+Space toggles
/// dictation. If the OS denies global capture (permissions), the app keeps
/// working through the in-app shortcuts and the failure is logged.
#[cfg(not(target_os = "macos"))]
pub fn spawn(tx: Sender<Hotkey>, ctx: egui::Context) {
    use std::collections::HashSet;

    use rdev::{Event, EventType, Key};

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

#[cfg(not(target_os = "macos"))]
fn is_modifier(key: rdev::Key) -> bool {
    matches!(
        key,
        rdev::Key::Alt
            | rdev::Key::AltGr
            | rdev::Key::ControlLeft
            | rdev::Key::ControlRight
            | rdev::Key::ShiftLeft
            | rdev::Key::ShiftRight
            | rdev::Key::MetaLeft
            | rdev::Key::MetaRight
    )
}

#[cfg(not(target_os = "macos"))]
fn alt_held(modifiers: &std::collections::HashSet<rdev::Key>) -> bool {
    modifiers.contains(&rdev::Key::Alt) || modifiers.contains(&rdev::Key::AltGr)
}

/// macOS: listen for Alt+Space through a raw CGEventTap.
///
/// We deliberately do NOT use `rdev::listen` here: rdev's callback converts
/// every key press into its character via the Carbon Text Input Source APIs
/// (`TISGetInputSourceProperty`). On macOS 26 those APIs assert that they run
/// on the main dispatch queue, but the event-tap callback fires on a
/// background thread, so the very first keystroke kills the app with
/// `dispatch_assert_queue_fail` (EXC_BREAKPOINT). This tap only reads the
/// virtual keycode and modifier flags — no TSM call, no crash.
#[cfg(target_os = "macos")]
pub fn spawn(tx: Sender<Hotkey>, ctx: egui::Context) {
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        EventField,
    };

    // kVK_Space.
    const KEY_SPACE: i64 = 49;

    std::thread::spawn(move || {
        let space_down = std::cell::Cell::new(false);
        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown, CGEventType::KeyUp],
            move |_proxy, event_type, event| {
                let is_space =
                    event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) == KEY_SPACE;
                if !is_space {
                    return None;
                }
                let alt_held = event
                    .get_flags()
                    .contains(core_graphics::event::CGEventFlags::CGEventFlagAlternate);
                match event_type {
                    CGEventType::KeyDown => {
                        if !space_down.get() && alt_held {
                            let _ = tx.send(Hotkey::ToggleRecording);
                            ctx.request_repaint();
                        }
                        space_down.set(true);
                    }
                    CGEventType::KeyUp => space_down.set(false),
                    _ => {}
                }
                None
            },
        );

        match tap {
            Ok(tap) => unsafe {
                let current = CFRunLoop::get_current();
                match tap.mach_port.create_runloop_source(0) {
                    Ok(loop_source) => {
                        current.add_source(&loop_source, kCFRunLoopCommonModes);
                        tap.enable();
                        CFRunLoop::run_current();
                    }
                    Err(_) => {
                        log::error!("failed to create run loop source for global hotkey tap")
                    }
                }
            },
            Err(_) => {
                log::error!("failed to create global hotkey event tap (accessibility permission?)")
            }
        }
    });
}
