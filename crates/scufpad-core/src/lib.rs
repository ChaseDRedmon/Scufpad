mod contracts;
mod engine;
mod io;
mod keys;
mod validation;

pub use contracts::*;
pub use engine::{MappingEngine, OutputState};
pub use io::*;
pub use keys::{format_chord, parse_chord};
pub use validation::validate_profile;

pub const EXTRA_BUTTONS_MASK: u64 = 0x3f << 19;
pub const NORMAL_BUTTONS_MASK: u64 = (1 << 11) - 1;
pub const HARDWARE_PROFILE_MASK: u64 = 1 << 16;

pub fn decode_usb_report(report: &[u8]) -> Option<PhysicalState> {
    if report.len() != 16 || report[0] != 6 || report[11] >> 4 > 8 {
        return None;
    }
    let mut buttons = u32::from_le_bytes(report[12..16].try_into().ok()?) as u64;
    if buttons & !(((1 << 17) - 1) | EXTRA_BUTTONS_MASK) != 0 {
        return None;
    }
    let hat = report[11] >> 4;
    for (active, control) in [
        (matches!(hat, 0 | 1 | 7), PhysicalControl::DpadUp),
        (matches!(hat, 1..=3), PhysicalControl::DpadRight),
        (matches!(hat, 3..=5), PhysicalControl::DpadDown),
        (matches!(hat, 5..=7), PhysicalControl::DpadLeft),
    ] {
        if active {
            buttons |= control.bit();
        }
    }
    let axis = |i| i16::from_le_bytes([report[i], report[i + 1]]) as i32;
    Some(PhysicalState {
        buttons,
        left_x: axis(1),
        left_y: axis(3),
        right_x: axis(5),
        right_y: axis(7),
        left_trigger: report[9] as i32 | ((report[10] as i32 & 3) << 8),
        right_trigger: (report[10] as i32 >> 2) | ((report[11] as i32 & 15) << 6),
    })
}
