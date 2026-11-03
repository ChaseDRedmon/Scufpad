use crate::discovery::{Devices, valid_descriptor, verify_node};
use anyhow::{Result, bail, ensure};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

pub fn ioctl_value(file: &File, request: libc::c_ulong, value: libc::c_int) -> Result<()> {
    let result = unsafe { libc::ioctl(file.as_raw_fd(), request, value) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn ioctl_ptr<T>(file: &File, request: libc::c_ulong, value: &mut T) -> Result<()> {
    let result = unsafe { libc::ioctl(file.as_raw_fd(), request, value as *mut T) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

struct Grab(File);
impl Drop for Grab {
    fn drop(&mut self) {
        let _ = ioctl_value(&self.0, 0x40044590, 0);
    }
}

pub struct Session {
    raw: File,
    _grabs: Vec<Grab>,
}

impl Session {
    pub fn open(devices: &Devices) -> Result<Self> {
        let open = |path: &std::path::Path| -> io::Result<File> {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open(path)
        };
        let raw = open(&devices.hidraw)?;
        verify_node(&raw, &devices.raw_sysfs, &devices.usb)?;
        #[repr(C)]
        #[derive(Default)]
        struct RawInfo {
            bus: u32,
            vendor: u16,
            product: u16,
        }
        let mut info = RawInfo::default();
        ioctl_ptr(&raw, 0x80084803, &mut info)?;
        ensure!(
            info.bus == 3 && info.vendor == 0x1b1c && info.product == 0x3a05,
            "Hidraw FD identity rejected"
        );
        let mut size = 0i32;
        ioctl_ptr(&raw, 0x80044801, &mut size)?;
        ensure!((1..=4096).contains(&size), "Invalid HID descriptor size");
        #[repr(C)]
        struct Descriptor {
            size: u32,
            bytes: [u8; 4096],
        }
        let mut descriptor = Descriptor {
            size: size as u32,
            bytes: [0; 4096],
        };
        ioctl_ptr(&raw, 0x90044802, &mut descriptor)?;
        ensure!(
            descriptor.size as usize <= descriptor.bytes.len()
                && valid_descriptor(&descriptor.bytes[..descriptor.size as usize]),
            "Hidraw FD descriptor rejected"
        );
        let mut grabs = Vec::new();
        for (path, sysfs) in devices.events.iter().zip(&devices.event_sysfs) {
            let file = open(path)?;
            verify_node(&file, sysfs, &devices.usb)?;
            let mut id = [0u16; 4];
            ioctl_ptr(&file, 0x80084502, &mut id)?;
            ensure!(
                id[0] == 3 && id[1] == 0x1b1c && id[2] == 0x3a05,
                "Evdev FD identity rejected"
            );
            ioctl_value(&file, 0x40044590, 1)?;
            grabs.push(Grab(file));
        }
        ensure!(grabs.len() == 6, "All six owned grabs are required");
        Ok(Self { raw, _grabs: grabs })
    }

    pub fn poll(&self, timeout_ms: i32) -> Result<bool> {
        let mut poll = libc::pollfd {
            fd: self.raw.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, timeout_ms) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(false);
            }
            return Err(error.into());
        }
        ensure!(
            poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) == 0,
            "Controller poll failed"
        );
        Ok(poll.revents & libc::POLLIN != 0)
    }

    pub fn read(&mut self, bytes: &mut [u8; 64]) -> Result<Option<usize>> {
        for _ in 0..9 {
            match self.raw.read(bytes) {
                Ok(0) => bail!("Controller disconnected"),
                Ok(n) => return Ok(Some(n)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        bail!("Controller read repeatedly interrupted")
    }
}
