//! Gamepads, read natively with gilrs (XInput on Windows, evdev on Linux and the
//! Steam Deck, Steam Input's virtual pad included) and handed to the UI as `gamepad` events.
//! Nothing is sent while the app is not the window in front, so a pad held for the game never
//! drives the app behind it. The UI turns the events into focus moves and actions (src/pad.ts).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
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

/// What tells whether the main window is the one in front (see `in_front`).
struct Front {
    /// tao's focus state, which Linux goes by.
    focused: Arc<AtomicBool>,
    /// The main window's handle, which Windows compares with the foreground window.
    #[cfg(windows)]
    hwnd: isize,
}

/// A press from `debug_pad`: the button, pressed or released, and whether to send it even
/// while the window is behind (tests leave the focus with whatever the user is doing).
type Fake = (Button, bool, bool);

/// Development builds only: presses handed to the reader thread as if a pad had sent them.
struct Fakes(Sender<Fake>);

/// Development builds only, for tests over CDP: a press that takes a real pad's way from the
/// reader thread (button names, the front-window gate, the `gamepad` event), unlike
/// `window.__ds2pad`, which starts in the UI.
#[tauri::command]
pub fn debug_pad(app: AppHandle, name: String, pressed: bool, even_behind: bool) -> Result<(), String> {
    use Button::*;
    let fakes = app.try_state::<Fakes>().ok_or("development builds only")?;
    let all = [South, East, North, West, LeftTrigger, RightTrigger, LeftTrigger2, RightTrigger2, Select, Start, LeftThumb, RightThumb, DPadUp, DPadDown, DPadLeft, DPadRight];
    let b = all.into_iter().find(|b| button(*b) == Some(name.as_str())).ok_or("unknown button")?;
    fakes.0.send((b, pressed, even_behind)).map_err(|e| e.to_string())
}

/// Starts reading pads on a thread of its own. Call once the main window exists.
pub fn start(app: &AppHandle) {
    let window = app.get_webview_window("main");
    let front = Front {
        focused: Arc::new(AtomicBool::new(window.as_ref().and_then(|w| w.is_focused().ok()).unwrap_or(false))),
        #[cfg(windows)]
        hwnd: window.as_ref().and_then(|w| w.hwnd().ok()).map_or(0, |h| h.0 as isize),
    };
    if let Some(window) = window {
        let focused = front.focused.clone();
        window.on_window_event(move |e| {
            if let WindowEvent::Focused(f) = e {
                focused.store(*f, Ordering::Relaxed);
            }
        });
    }
    let pads = app.state::<Pads>().0.clone();
    let fakes = cfg!(debug_assertions).then(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        app.manage(Fakes(tx));
        rx
    });
    let app = app.clone();
    let _ = std::thread::Builder::new().name("gamepad".into()).spawn(move || run(&app, &front, &pads, fakes));
}

fn run(app: &AppHandle, front_of: &Front, pads: &Mutex<Vec<PadInfo>>, fakes: Option<Receiver<Fake>>) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(gilrs::Error::NotImplemented(g)) => g,
        Err(_) => return,
    };
    let skip = Ignored::from_env();
    // Physical pads first: their glyphs, not those of Steam's stand-in, describe the buttons.
    let list = |g: &Gilrs| {
        let mut all: Vec<_> = g.gamepads().map(|(_, pad)| (is_steam_virtual(&pad), info(&pad))).collect();
        all.sort_by_key(|(virtual_pad, _)| *virtual_pad);
        all.into_iter().map(|(_, info)| info).collect::<Vec<_>>()
    };
    *pads.lock().unwrap() = list(&gilrs);
    let mut front = false;
    loop {
        let event = gilrs.next_event_blocking(Some(Duration::from_millis(250)));
        // Leaving the front lets go of whatever was held, or a held direction would keep
        // repeating in the UI with its release never sent.
        let now_front = in_front(front_of);
        if front && !now_front {
            let _ = app.emit("gamepad", PadEvent::Blur);
        }
        front = now_front;
        for (b, pressed, even_behind) in fakes.iter().flat_map(Receiver::try_iter) {
            if let (Some(button), true) = (button(b), front || even_behind) {
                let _ = app.emit("gamepad", PadEvent::Button { button, pressed });
            }
        }
        let Some(event) = event else { continue };
        let is_input = !matches!(event.event, EventType::Connected | EventType::Disconnected);
        if is_input && skip.covers(&gilrs.gamepad(event.id)) {
            continue;
        }
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
            if front {
                let _ = app.emit("gamepad", out);
            }
        }
    }
}

/// Whether the user is in the app's main window. On Windows tao's focus events can't say: as
/// soon as the window activates, WebView2 takes the keyboard focus from it (wry's `MoveFocus`),
/// and the window reports focus lost while it is still in front, which used to drop every press.
/// There the foreground window must be the main window itself: not merely one of this process,
/// since a system file dialog in front of it belongs to the process too, and Start pressed there
/// would save behind it.
#[cfg(windows)]
fn in_front(front: &Front) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: only reads which window is in front.
    let foreground = unsafe { GetForegroundWindow() } as isize;
    front.hwnd != 0 && foreground == front.hwnd
}

/// GTK reports the toplevel's focus whichever widget inside it holds the keyboard; a file dialog
/// is a toplevel of its own, so it takes that focus away.
#[cfg(not(windows))]
fn in_front(front: &Front) -> bool {
    front.focused.load(Ordering::Relaxed)
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
/// The Xbox 360-style pad Steam Input creates for each controller it manages.
const STEAM_VIRTUAL: u16 = 0x11FF;

fn is_steam_virtual(pad: &Gamepad<'_>) -> bool {
    pad.vendor_id() == Some(VALVE) && pad.product_id() == Some(STEAM_VIRTUAL)
}

/// The pads a game should leave alone. When Steam Input manages a controller, Steam hands games
/// a virtual pad for it and lists the physical one in SDL's ignore variables; gilrs reads every
/// device regardless, so without this each press would arrive twice (once from each).
#[derive(Default)]
struct Ignored {
    ignore: Vec<(u16, u16)>,
    only: Option<Vec<(u16, u16)>>,
}

impl Ignored {
    fn from_env() -> Self {
        Self { ignore: sdl_ids("SDL_GAMECONTROLLER_IGNORE_DEVICES").unwrap_or_default(), only: sdl_ids("SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT") }
    }

    fn covers(&self, pad: &Gamepad<'_>) -> bool {
        let (Some(v), Some(p)) = (pad.vendor_id(), pad.product_id()) else { return false };
        match &self.only {
            Some(only) => !only.contains(&(v, p)),
            None => self.ignore.contains(&(v, p)),
        }
    }
}

/// An SDL device list from the environment: `0xAAAA/0xBBBB,0xCCCC/0xDDDD`, or `@file` holding one.
fn sdl_ids(var: &str) -> Option<Vec<(u16, u16)>> {
    let raw = std::env::var(var).ok()?;
    let text = match raw.strip_prefix('@') {
        Some(file) => std::fs::read_to_string(file).ok()?,
        None => raw,
    };
    Some(parse_ids(&text))
}

fn parse_ids(text: &str) -> Vec<(u16, u16)> {
    let hex = |s: &str| u16::from_str_radix(s.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).ok();
    text.split(',')
        .filter_map(|pair| {
            let (v, p) = pair.split_once('/')?;
            Some((hex(v)?, hex(p)?))
        })
        .collect()
}

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

#[cfg(test)]
mod tests {
    use super::parse_ids;

    #[test]
    fn reads_sdl_device_lists() {
        assert_eq!(parse_ids("0x28DE/0x1205,0x045e/0x028E"), [(0x28DE, 0x1205), (0x045E, 0x028E)]);
        assert_eq!(parse_ids(" 0x054C/0x0CE6 , junk, 0x1/"), [(0x054C, 0x0CE6)]);
        assert!(parse_ids("").is_empty());
    }
}
