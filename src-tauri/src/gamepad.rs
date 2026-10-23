//! Gamepads, read natively with gilrs (Windows.Gaming.Input on Windows, evdev on Linux and the
//! Steam Deck, Steam Input's virtual pad included) and handed to the UI as `gamepad` events.
//! Nothing is sent while the window lacks focus, so a pad held for the game never drives the
//! app behind it. The UI turns the events into focus moves and actions (src/pad.ts).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gamepad, Gilrs};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PadEvent {
    Button { button: &'static str, pressed: bool },
    Axis { axis: &'static str, value: f32 },
    /// The connected pads changed.
    Pads { pads: Vec<PadInfo> },
    /// The window lost focus: whatever was held is let go.
    Blur,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PadInfo {
    name: String,
    /// Which button glyphs to show: "xbox", "playstation" or "deck".
    family: &'static str,
}

#[derive(Default)]
pub struct Pads(Arc<Mutex<Vec<PadInfo>>>);

#[tauri::command]
pub fn gamepads(pads: State<'_, Pads>) -> Vec<PadInfo> {
    pads.0.lock().unwrap().clone()
}

/// Starts reading pads on a thread of its own. Call once the main window exists.
pub fn start(app: &AppHandle) {
    let focused = Arc::new(AtomicBool::new(false));
    if let Some(window) = app.get_webview_window("main") {
        focused.store(window.is_focused().unwrap_or(false), Ordering::Relaxed);
        let (focused, app) = (focused.clone(), app.clone());
        window.on_window_event(move |e| {
            if let WindowEvent::Focused(f) = e {
                focused.store(*f, Ordering::Relaxed);
                if !f {
                    let _ = app.emit("gamepad", PadEvent::Blur);
                }
            }
        });
    }
    let pads = app.state::<Pads>().0.clone();
    let app = app.clone();
    let _ = std::thread::Builder::new().name("gamepad".into()).spawn(move || run(&app, &focused, &pads));
}

fn run(app: &AppHandle, focused: &AtomicBool, pads: &Mutex<Vec<PadInfo>>) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(gilrs::Error::NotImplemented(g)) => g,
        Err(_) => return,
    };
    let list = |g: &Gilrs| g.gamepads().map(|(_, pad)| info(&pad)).collect::<Vec<_>>();
    *pads.lock().unwrap() = list(&gilrs);
    loop {
        let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(500))) else { continue };
        let out = match event.event {
            EventType::Connected | EventType::Disconnected => {
                let now = list(&gilrs);
                *pads.lock().unwrap() = now.clone();
                let _ = app.emit("gamepad", PadEvent::Pads { pads: now });
                continue;
            }
            EventType::ButtonPressed(b, _) => button(b).map(|button| PadEvent::Button { button, pressed: true }),
            EventType::ButtonReleased(b, _) => button(b).map(|button| PadEvent::Button { button, pressed: false }),
            EventType::AxisChanged(a, value, _) => axis(a).map(|axis| PadEvent::Axis { axis, value }),
            _ => None,
        };
        if let Some(out) = out {
            if focused.load(Ordering::Relaxed) {
                let _ = app.emit("gamepad", out);
            }
        }
    }
}

fn button(b: Button) -> Option<&'static str> {
    Some(match b {
        Button::South => "south",
        Button::East => "east",
        Button::North => "north",
        Button::West => "west",
        Button::LeftTrigger => "lb",
        Button::RightTrigger => "rb",
        Button::LeftTrigger2 => "lt",
        Button::RightTrigger2 => "rt",
        Button::Select => "select",
        Button::Start => "start",
        Button::LeftThumb => "l3",
        Button::RightThumb => "r3",
        Button::DPadUp => "up",
        Button::DPadDown => "down",
        Button::DPadLeft => "left",
        Button::DPadRight => "right",
        _ => return None,
    })
}

fn axis(a: Axis) -> Option<&'static str> {
    Some(match a {
        Axis::LeftStickX => "lx",
        Axis::LeftStickY => "ly",
        Axis::RightStickX => "rx",
        Axis::RightStickY => "ry",
        _ => return None,
    })
}

const SONY: u16 = 0x054C;
const VALVE: u16 = 0x28DE;
const STEAM_DECK: u16 = 0x1205;

fn info(pad: &Gamepad<'_>) -> PadInfo {
    let name = pad.name().to_string();
    let lower = name.to_lowercase();
    let family = if pad.vendor_id() == Some(SONY) || ["dualsense", "dualshock", "playstation"].iter().any(|n| lower.contains(n)) {
        "playstation"
    } else if pad.vendor_id() == Some(VALVE)
        && (pad.product_id() == Some(STEAM_DECK) || std::env::var_os("SteamDeck").is_some_and(|v| v == "1"))
    {
        // In Game Mode the Deck's controls arrive as Steam Input's virtual pad.
        "deck"
    } else {
        "xbox"
    };
    PadInfo { name, family }
}
