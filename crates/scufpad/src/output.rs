use crate::hardware::{ioctl_ptr, ioctl_value};
use anyhow::{Result, ensure};
use scufpad_core::{GamepadControl as G, OutputState};
use std::{
    fs::{File, OpenOptions},
    io,
    mem::size_of,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Event {
    time: libc::timeval,
    kind: u16,
    code: u16,
    value: i32,
}

impl Event {
    fn new(kind: u16, code: u16, value: i32) -> Self {
        Self {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            kind,
            code,
            value,
        }
    }
}

fn write_events(bytes: &[u8], mut write: impl FnMut(&[u8]) -> io::Result<usize>) -> Result<()> {
    let (mut offset, mut interruptions) = (0, 0);
    while offset < bytes.len() {
        match write(&bytes[offset..]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted && interruptions < 8 => {
                interruptions += 1
            }
            Err(error) => return Err(error.into()),
            Ok(n) => {
                ensure!(
                    n > 0 && n <= bytes.len() - offset && n % size_of::<Event>() == 0,
                    "Uncertain partial uinput frame"
                );
                offset += n;
                interruptions = 0;
            }
        }
    }
    Ok(())
}

fn verify_uinput(file: &File, expected: &str) -> Result<()> {
    let metadata = file.metadata()?;
    ensure!(
        metadata.file_type().is_char_device(),
        "uinput FD is not a character device"
    );
    let identity = format!(
        "{}:{}",
        libc::major(metadata.rdev()),
        libc::minor(metadata.rdev())
    );
    ensure!(
        identity == expected.trim(),
        "uinput FD identity does not match kernel sysfs"
    );
    Ok(())
}

struct Device {
    file: File,
    controls: Vec<(u16, u16)>,
    previous: Vec<i32>,
    events: Vec<Event>,
    created: bool,
    uncertain: bool,
}

impl Device {
    fn create(
        name: &[u8],
        id: [u16; 4],
        keys: &[u16],
        axes: &[(u16, i32, i32)],
        mouse: bool,
    ) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open("/dev/uinput")?;
        verify_uinput(
            &file,
            &std::fs::read_to_string("/sys/class/misc/uinput/dev")?,
        )?;
        let mut version = 0u32;
        ioctl_ptr(&file, 0x8004552d, &mut version)?;
        ensure!(
            version >= 5,
            "uinput setup API version 5 or later is required"
        );
        let mut device = Self {
            file,
            controls: Vec::new(),
            previous: Vec::new(),
            events: Vec::new(),
            created: false,
            uncertain: false,
        };
        ioctl_value(&device.file, 0x40045564, 0)?;
        ioctl_value(&device.file, 0x40045564, 1)?;
        for &key in keys {
            ioctl_value(&device.file, 0x40045565, key as i32)?;
            device.controls.push((1, key));
        }
        if !axes.is_empty() {
            ioctl_value(&device.file, 0x40045564, 3)?;
        }
        for &(code, min, max) in axes {
            ioctl_value(&device.file, 0x40045567, code as i32)?;
            #[repr(C)]
            struct AbsSetup {
                code: u16,
                padding: u16,
                info: [i32; 6],
            }
            let stick = matches!(code, 0 | 1 | 3 | 4);
            let mut setup = AbsSetup {
                code,
                padding: 0,
                info: [
                    0,
                    min,
                    max,
                    if stick { 16 } else { 0 },
                    if stick { 128 } else { 0 },
                    0,
                ],
            };
            ioctl_ptr(&device.file, 0x401c5504, &mut setup)?;
            device.controls.push((3, code));
        }
        if mouse {
            ioctl_value(&device.file, 0x40045564, 2)?;
            for code in 0..2 {
                ioctl_value(&device.file, 0x40045566, code)?;
            }
        }
        #[repr(C)]
        struct Setup {
            id: [u16; 4],
            name: [u8; 80],
            ff_effects: u32,
        }
        let mut setup = Setup {
            id,
            name: [0; 80],
            ff_effects: 0,
        };
        ensure!(
            name.len() < setup.name.len(),
            "Virtual device name too long"
        );
        setup.name[..name.len()].copy_from_slice(name);
        ioctl_ptr(&device.file, 0x405c5503, &mut setup)?;
        ioctl_value(&device.file, 0x5501, 0)?;
        device.created = true;
        device.previous.resize(device.controls.len(), 0);
        device.events.reserve(device.controls.len() + 1);
        Ok(device)
    }

    fn emit(&mut self, values: &[i32], force: bool) -> Result<()> {
        ensure!(
            values.len() == self.controls.len(),
            "Wrong virtual state size"
        );
        self.events.clear();
        for (i, &(kind, code)) in self.controls.iter().enumerate() {
            if force || self.uncertain || values[i] != self.previous[i] {
                self.events.push(Event::new(kind, code, values[i]));
            }
        }
        if self.events.is_empty() {
            return Ok(());
        }
        self.events.push(Event::new(0, 0, 0));
        self.uncertain = true;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                self.events.as_ptr().cast::<u8>(),
                self.events.len() * size_of::<Event>(),
            )
        };
        write_events(bytes, |bytes| {
            let n =
                unsafe { libc::write(self.file.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        })?;
        self.previous.copy_from_slice(values);
        self.uncertain = false;
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if self.created {
            // Always release the full capability snapshot, including uncertain writes.
            let zeros = [0i32; 256];
            let length = self.controls.len();
            if length <= zeros.len()
                && let Err(error) = self.emit(&zeros[..length], true)
            {
                eprintln!("Neutralization failed: {error}");
            }
            let _ = ioctl_value(&self.file, 0x5502, 0);
        }
    }
}

const BUTTONS: [(G, u16); 15] = [
    (G::A, 0x130),
    (G::B, 0x131),
    (G::X, 0x133), // BTN_X / BTN_NORTH: match xpad for the Microsoft device identity.
    (G::Y, 0x134), // BTN_Y / BTN_WEST.
    (G::LeftBumper, 0x136),
    (G::RightBumper, 0x137),
    (G::View, 0x13a),
    (G::Menu, 0x13b),
    (G::Guide, 0x13c),
    (G::LeftStickClick, 0x13d),
    (G::RightStickClick, 0x13e),
    (G::Paddle1, 0x2c0),
    (G::Paddle2, 0x2c1),
    (G::Paddle3, 0x2c2),
    (G::Paddle4, 0x2c3),
];
const AXES: [(u16, i32, i32); 8] = [
    (0, -32768, 32767),
    (1, -32768, 32767),
    (3, -32768, 32767),
    (4, -32768, 32767),
    (2, 0, 1023),
    (5, 0, 1023),
    (16, -1, 1),
    (17, -1, 1),
];
const GAMEPAD_ID: [u16; 4] = [3, 0x045e, 0x0b12, 1];
const GAMEPAD_NAME: &[u8] = b"Xbox Elite 2 Virtual Controller";

fn gamepad_values(state: &OutputState) -> [i32; 23] {
    let mut values = [0; 23];
    let has = |control: G| i32::from(state.gamepad_buttons & control.bit() != 0);
    for (i, &(control, _)) in BUTTONS.iter().enumerate() {
        values[i] = has(control);
    }
    values[15..].copy_from_slice(&[
        state.left_x,
        state.left_y,
        state.right_x,
        state.right_y,
        state.left_trigger,
        state.right_trigger,
        has(G::DpadRight) - has(G::DpadLeft),
        has(G::DpadDown) - has(G::DpadUp),
    ]);
    values
}

fn filter_axes(values: &mut [i32; 23], previous: &[i32]) {
    for offset in [15, 17] {
        let x = i64::from(values[offset]);
        let y = i64::from(values[offset + 1]);
        if x * x + y * y < 3500 * 3500 {
            values[offset] = 0;
            values[offset + 1] = 0;
        } else {
            for axis in offset..offset + 2 {
                if (values[axis] - previous[axis]).abs() < 300 {
                    values[axis] = previous[axis];
                }
            }
        }
    }
    for axis in [19, 20] {
        if values[axis] < 10 {
            values[axis] = 0;
        } else if (values[axis] - previous[axis]).abs() < 20 {
            values[axis] = previous[axis];
        }
    }
}

pub struct Outputs {
    gamepad: Device,
    remapping: Option<(Device, Device)>,
}
impl Outputs {
    #[cfg(test)]
    pub fn test_sysname(&self) -> Result<String> {
        let mut bytes = [0xffu8; 128];
        ioctl_ptr(&self.gamepad.file, 0x8080552c, &mut bytes)?;
        test_sysname_from_bytes(&bytes)
    }

    pub fn create() -> Result<Self> {
        let keys: Vec<_> = BUTTONS.iter().map(|&(_, code)| code).collect();
        let gamepad = Device::create(GAMEPAD_NAME, GAMEPAD_ID, &keys, &AXES, false)?;
        Ok(Self {
            gamepad,
            remapping: None,
        })
    }

    pub fn set_remapping(&mut self, enabled: bool) -> Result<()> {
        if !enabled {
            self.remapping = None;
            return Ok(());
        }
        if self.remapping.is_some() {
            return Ok(());
        }
        let keyboard = Device::create(
            b"Scufpad Virtual Keyboard",
            [6, 0, 0, 1],
            &(1..256).collect::<Vec<u16>>(),
            &[],
            false,
        )?;
        let mouse = Device::create(
            b"Scufpad Virtual Mouse",
            [6, 0, 0, 1],
            &(272..280).collect::<Vec<u16>>(),
            &[],
            true,
        )?;
        self.remapping = Some((keyboard, mouse));
        Ok(())
    }

    pub fn emit(&mut self, state: &OutputState) -> Result<()> {
        let mut values = gamepad_values(state);
        filter_axes(&mut values, &self.gamepad.previous);
        self.gamepad.emit(&values, false)?;
        if let Some((keyboard, mouse)) = &mut self.remapping {
            let mut keys = [0; 255];
            for (value, &pressed) in keys.iter_mut().zip(&state.keyboard[1..]) {
                *value = i32::from(pressed);
            }
            keyboard.emit(&keys, false)?;
            let buttons = state.mouse.map(i32::from);
            mouse.emit(&buttons, false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
fn test_sysname_from_bytes(bytes: &[u8; 128]) -> Result<String> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .ok_or_else(|| anyhow::anyhow!("uinput sysname has no bounded NUL terminator"))?;
    let name = &bytes[..end];
    ensure!(
        name.starts_with(b"input") && name.len() > 5 && name[5..].iter().all(u8::is_ascii_digit),
        "Invalid uinput sysname: expected input followed by ASCII digits"
    );
    Ok(std::str::from_utf8(name)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysname_requires_input_digits_and_bounded_nul() {
        for name in [b"input0".as_slice(), b"input123456"] {
            let mut bytes = [0xff; 128];
            bytes[..name.len()].copy_from_slice(name);
            bytes[name.len()] = 0;
            assert_eq!(test_sysname_from_bytes(&bytes).unwrap().as_bytes(), name);
        }
        for name in [
            b"".as_slice(),
            b"input",
            b"input-1",
            b"input1/other",
            b"event1",
            b"input\xff",
        ] {
            let mut bytes = [0; 128];
            bytes[..name.len()].copy_from_slice(name);
            assert!(test_sysname_from_bytes(&bytes).is_err());
        }
        let mut bytes = [b'1'; 128];
        bytes[..5].copy_from_slice(b"input");
        assert!(test_sysname_from_bytes(&bytes).is_err());
    }

    #[test]
    fn disabling_custom_outputs_neutralizes_auxiliary_devices_and_keeps_gamepad_identity() {
        let fixture = crate::test_support::Fixture::new();
        let gamepad_path = fixture.file("gamepad-events", &[]);
        let keyboard_path = fixture.file("keyboard-events", &[]);
        let mouse_path = fixture.file("mouse-events", &[]);
        let make_device = |path: &std::path::Path, control, pressed, created| Device {
            file: OpenOptions::new().write(true).open(path).unwrap(),
            controls: vec![control],
            previous: vec![pressed],
            events: Vec::with_capacity(2),
            created,
            uncertain: false,
        };
        let gamepad = make_device(&gamepad_path, (1, 0x130), 0, false);
        let descriptor = gamepad.file.as_raw_fd();
        let mut outputs = Outputs {
            gamepad,
            remapping: Some((
                make_device(&keyboard_path, (1, 46), 1, true),
                make_device(&mouse_path, (1, 272), 1, true),
            )),
        };
        outputs.set_remapping(true).unwrap();
        assert_eq!(std::fs::metadata(&keyboard_path).unwrap().len(), 0);
        outputs.set_remapping(false).unwrap();
        assert!(outputs.remapping.is_none());
        assert_eq!(outputs.gamepad.file.as_raw_fd(), descriptor);
        assert_eq!(std::fs::metadata(&gamepad_path).unwrap().len(), 0);
        for path in [&keyboard_path, &mouse_path] {
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(bytes.len(), size_of::<Event>() * 2);
            for record in bytes.as_chunks::<{ size_of::<Event>() }>().0 {
                let offset = std::mem::offset_of!(Event, value);
                assert_eq!(
                    i32::from_ne_bytes(record[offset..offset + 4].try_into().unwrap()),
                    0
                );
            }
        }
        outputs.set_remapping(false).unwrap();
        assert_eq!(outputs.gamepad.file.as_raw_fd(), descriptor);
        assert_eq!(
            std::fs::metadata(&keyboard_path).unwrap().len(),
            (size_of::<Event>() * 2) as u64
        );
    }

    #[test]
    fn uinput_identity_rejects_regular_files_and_other_character_devices() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.file("not-uinput", b"untouched");
        assert!(verify_uinput(&File::open(&path).unwrap(), "10:223").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"untouched");
        assert!(verify_uinput(&File::open("/dev/null").unwrap(), "10:223").is_err());
    }
    #[test]
    fn writes_retry_eintr_and_aligned_partial_events() {
        let bytes = vec![0; size_of::<Event>() * 3];
        let mut calls = 0;
        write_events(&bytes, |remaining| {
            calls += 1;
            match calls {
                1 => Err(io::Error::from(io::ErrorKind::Interrupted)),
                2 => Ok(size_of::<Event>()),
                _ => Ok(remaining.len()),
            }
        })
        .unwrap();
        assert_eq!(calls, 3);
    }

    #[test]
    fn uncertain_writes_reject_zero_misaligned_and_impossible_counts() {
        let bytes = vec![0; size_of::<Event>()];
        for count in [0, 1, bytes.len() + size_of::<Event>()] {
            assert!(write_events(&bytes, |_| Ok(count)).is_err());
        }
        assert!(write_events(&bytes, |_| Err(io::ErrorKind::Interrupted.into())).is_err());
    }

    #[test]
    fn native_event_layout_and_gamepad_mapping() {
        assert_eq!(GAMEPAD_ID, [3, 0x045e, 0x0b12, 1]);
        assert_eq!(GAMEPAD_NAME, b"Xbox Elite 2 Virtual Controller");
        assert_eq!(size_of::<Event>(), size_of::<libc::timeval>() + 8);
        let state = OutputState {
            gamepad_buttons: G::X.bit() | G::DpadLeft.bit() | G::DpadDown.bit() | G::Paddle4.bit(),
            left_trigger: 1023,
            ..Default::default()
        };
        let values = gamepad_values(&state);
        assert_eq!(values[2], 1);
        assert_eq!(values[14], 1);
        assert_eq!(&values[19..], &[1023, 0, -1, 1]);
        assert_eq!(BUTTONS[2].1, 0x133);
    }

    #[test]
    fn xbox_face_buttons_use_kernel_btn_x_and_btn_y_codes() {
        for (control, expected_code) in [(G::X, 0x133), (G::Y, 0x134)] {
            let state = OutputState {
                gamepad_buttons: control.bit(),
                ..Default::default()
            };
            let values = gamepad_values(&state);
            let pressed: Vec<_> = BUTTONS
                .iter()
                .zip(values)
                .filter_map(|(&(_, code), value)| (value == 1).then_some(code))
                .collect();
            assert_eq!(pressed, [expected_code], "{control:?}");
        }
    }

    #[test]
    fn production_deadzone_and_jitter_filter_match_existing_daemon() {
        let mut state = OutputState {
            left_x: 3499,
            right_x: 3500,
            left_trigger: 9,
            right_trigger: 20,
            ..Default::default()
        };
        let mut values = gamepad_values(&state);
        filter_axes(&mut values, &[0; 23]);
        assert_eq!(&values[15..21], &[0, 0, 3500, 0, 0, 20]);
        let previous = values;
        state.right_x = 3799;
        state.right_trigger = 39;
        let mut values = gamepad_values(&state);
        filter_axes(&mut values, &previous);
        assert_eq!(values[17], 3500);
        assert_eq!(values[20], 20);
        state.right_x = 0;
        state.right_trigger = 0;
        let mut values = gamepad_values(&state);
        filter_axes(&mut values, &previous);
        assert_eq!(values[17], 0);
        assert_eq!(values[20], 0);
    }

    #[test]
    fn writer_works_with_regular_fixture_file() {
        use std::io::Write;
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.file("events", &[]);
        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        let bytes = vec![0; size_of::<Event>() * 2];
        write_events(&bytes, |part| file.write(part)).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn unchanged_states_do_not_write_and_uncertain_state_forces_neutral_snapshot() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.file("events", &[]);
        let mut device = Device {
            file: OpenOptions::new().write(true).open(&path).unwrap(),
            controls: vec![(1, 30)],
            previous: vec![0],
            events: Vec::with_capacity(2),
            created: false,
            uncertain: false,
        };
        device.emit(&[0], false).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        device.emit(&[1], false).unwrap();
        let length = std::fs::metadata(&path).unwrap().len();
        device.emit(&[1], false).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), length);
        device.file = File::open(&path).unwrap();
        assert!(device.emit(&[0], false).is_err());
        assert!(device.uncertain);
        assert_eq!(device.previous, [1]);
        device.file = OpenOptions::new().append(true).open(&path).unwrap();
        device.emit(&[0], false).unwrap();
        assert!(!device.uncertain);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), length * 2);
    }
}
