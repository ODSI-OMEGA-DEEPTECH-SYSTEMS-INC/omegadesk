//! Injeção com `SendInput` no Windows.

#![allow(unsafe_code, reason = "SendInput é uma API Win32")]

use omegadesk_protocol::v1::{KeyEvent, PointerEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSE_EVENT_FLAGS,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
    SendInput, VIRTUAL_KEY,
};

use crate::{InputError, InputInjector, buttons, hid_to_scancode};

/// `XBUTTON1` e `XBUTTON2` do Win32.
const XBUTTON1: i32 = 1;
const XBUTTON2: i32 = 2;

#[derive(Debug, Default)]
pub(crate) struct WindowsInjector {
    pressed_buttons: u32,
}

fn send(inputs: &[INPUT]) -> Result<(), InputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    let size = i32::try_from(size_of::<INPUT>()).map_err(|_| InputError::Rejected)?;
    let sent = unsafe { SendInput(inputs, size) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(InputError::Rejected)
    }
}

fn mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data.cast_unsigned(),
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

impl InputInjector for WindowsInjector {
    fn key(&mut self, event: &KeyEvent) -> Result<(), InputError> {
        let Some(scancode) = hid_to_scancode(event.hid_usage) else {
            return Ok(());
        };
        let mut flags = KEYEVENTF_SCANCODE;
        if scancode.extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if !event.pressed {
            flags |= KEYEVENTF_KEYUP;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: scancode.code,
                    dwFlags: KEYBD_EVENT_FLAGS(flags.0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        send(&[input])
    }

    fn pointer(&mut self, event: &PointerEvent) -> Result<(), InputError> {
        // Coordenadas absolutas 0..=65535 sobre o monitor principal, igual ao protocolo.
        let x = i32::try_from(event.x.min(65_535)).unwrap_or(65_535);
        let y = i32::try_from(event.y.min(65_535)).unwrap_or(65_535);
        let mut inputs = vec![mouse(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE, x, y, 0)];

        let changed = event.buttons ^ self.pressed_buttons;
        let transitions = [
            (buttons::LEFT, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
            (
                buttons::RIGHT,
                MOUSEEVENTF_RIGHTDOWN,
                MOUSEEVENTF_RIGHTUP,
                0,
            ),
            (
                buttons::MIDDLE,
                MOUSEEVENTF_MIDDLEDOWN,
                MOUSEEVENTF_MIDDLEUP,
                0,
            ),
            (buttons::BACK, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON1),
            (
                buttons::FORWARD,
                MOUSEEVENTF_XDOWN,
                MOUSEEVENTF_XUP,
                XBUTTON2,
            ),
        ];
        for (bit, down, up, data) in transitions {
            if changed & bit != 0 {
                let flags = if event.buttons & bit == 0 { up } else { down };
                inputs.push(mouse(flags, 0, 0, data));
            }
        }
        self.pressed_buttons = event.buttons;

        if event.wheel_y != 0 {
            inputs.push(mouse(MOUSEEVENTF_WHEEL, 0, 0, event.wheel_y));
        }
        if event.wheel_x != 0 {
            inputs.push(mouse(MOUSEEVENTF_HWHEEL, 0, 0, event.wheel_x));
        }
        send(&inputs)
    }
}
