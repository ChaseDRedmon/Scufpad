use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Devices {
    pub hidraw: PathBuf,
    pub events: Vec<PathBuf>,
    pub usb: PathBuf,
    pub raw_sysfs: PathBuf,
    pub event_sysfs: Vec<PathBuf>,
}

pub fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= maximum, "Oversized sysfs value");
    Ok(bytes)
}

fn text(path: &Path) -> Result<String> {
    Ok(String::from_utf8(bounded_read(path, 256)?)?
        .trim()
        .to_owned())
}

pub fn usb_parent(path: &Path) -> Option<PathBuf> {
    let real = path.canonicalize().ok()?;
    real.ancestors()
        .find(|p| p.join("idVendor").exists() && p.join("idProduct").exists())
        .map(Path::to_path_buf)
}

fn is_controller(usb: &Path) -> bool {
    text(&usb.join("idVendor")).ok().as_deref() == Some("1b1c")
        && text(&usb.join("idProduct")).ok().as_deref() == Some("3a05")
}

fn interface03(path: &Path, usb: &Path) -> bool {
    for parent in path.ancestors().take_while(|p| *p != usb) {
        let number = parent.join("bInterfaceNumber");
        if number.exists() {
            return text(&number).ok().as_deref() == Some("03");
        }
    }
    false
}

// Parse HID short items, rather than accepting report-ID bytes inside unrelated data.
pub fn valid_descriptor(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > 4096 {
        return false;
    }
    let (mut pos, mut size, mut count, mut id, mut input_bits) = (0, 0u32, 0u32, 0u32, 0u32);
    let mut stack = Vec::new();
    while pos < bytes.len() {
        let prefix = bytes[pos];
        pos += 1;
        if prefix == 0xfe {
            return false;
        }
        let length = match prefix & 3 {
            3 => 4,
            n => n as usize,
        };
        if pos + length > bytes.len() {
            return false;
        }
        let mut value = 0u32;
        for i in 0..length {
            value |= (bytes[pos + i] as u32) << (8 * i);
        }
        pos += length;
        match prefix & 0xfc {
            0x74 => size = value,
            0x94 => count = value,
            0x84 => {
                if !(1..=255).contains(&value) {
                    return false;
                }
                id = value;
            }
            0xa4 => {
                if stack.len() >= 32 {
                    return false;
                }
                stack.push((size, count, id));
            }
            0xb4 => {
                let Some(saved) = stack.pop() else {
                    return false;
                };
                (size, count, id) = saved;
            }
            0x80 if id == 6 => {
                let Some(bits) = size
                    .checked_mul(count)
                    .and_then(|n| input_bits.checked_add(n))
                else {
                    return false;
                };
                input_bits = bits;
            }
            _ => {}
        }
    }
    input_bits == 120 && stack.is_empty()
}

pub fn discover(sys_class: &Path, dev: &Path) -> Result<Option<Devices>> {
    let raw_class = sys_class.join("hidraw");
    if !raw_class.exists() {
        return Ok(None);
    }
    let mut raws = fs::read_dir(raw_class)?.collect::<std::io::Result<Vec<_>>>()?;
    raws.sort_by_key(|entry| entry.file_name());
    for entry in raws {
        if !entry.file_name().to_string_lossy().starts_with("hidraw") {
            continue;
        }
        let raw = entry.path().canonicalize()?;
        let Some(usb) = usb_parent(&raw) else {
            continue;
        };
        if !is_controller(&usb) || !interface03(&raw, &usb) {
            continue;
        }
        let descriptor = bounded_read(&raw.join("device/report_descriptor"), 4096)
            .context("Read interface 03 descriptor")?;
        if !valid_descriptor(&descriptor) {
            continue;
        }
        let mut events = Vec::new();
        let mut event_sysfs = Vec::new();
        for event in fs::read_dir(sys_class.join("input"))? {
            let event = event?;
            if !event.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            let path = event.path().canonicalize()?;
            if usb_parent(&path).as_ref() == Some(&usb) {
                events.push(dev.join("input").join(event.file_name()));
                event_sysfs.push(path);
            }
        }
        // The verified USB product exposes six evdev interfaces, including consumer aliases.
        ensure!(
            events.len() == 6,
            "Expected all six owned evdev interfaces; found {}",
            events.len()
        );
        return Ok(Some(Devices {
            hidraw: dev.join(entry.file_name()),
            events,
            usb,
            raw_sysfs: raw,
            event_sysfs,
        }));
    }
    Ok(None)
}

pub fn verify_node(fd: &File, sysfs: &Path, usb: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    use std::os::{fd::AsRawFd, unix::fs::FileTypeExt};
    let metadata = fd.metadata()?;
    ensure!(
        metadata.file_type().is_char_device(),
        "Input FD is not a character device"
    );
    ensure!(
        usb_parent(sysfs).as_deref() == Some(usb) && is_controller(usb),
        "USB identity changed"
    );
    let expected = text(&sysfs.join("dev"))?;
    let actual = format!(
        "{}:{}",
        libc::major(metadata.rdev()),
        libc::minor(metadata.rdev())
    );
    if expected != actual {
        bail!("Input FD does not match discovered sysfs node");
    }
    ensure!(fd.as_raw_fd() >= 0, "Invalid input FD");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_requires_exact_input_size_and_real_items() {
        assert!(valid_descriptor(&[0x85, 6, 0x75, 8, 0x95, 15, 0x81, 2]));
        assert!(!valid_descriptor(&[0x85, 6]));
        assert!(!valid_descriptor(&[0x85, 6, 0x75, 8, 0x95, 16, 0x81, 2]));
        assert!(!valid_descriptor(&[0x86, 0x85, 6]));
        assert!(!valid_descriptor(&[0x85, 6, 0x75, 8, 0x95, 15, 0x81]));
        assert!(!valid_descriptor(&vec![0; 4097]));
    }

    #[test]
    fn nearest_usb_parent_never_matches_a_hub_ancestor() {
        let fixture = crate::test_support::Fixture::new();
        let hub = fixture.directory("hub");
        let child = fixture.directory("hub/child");
        fixture.file("hub/idVendor", b"1b1c");
        fixture.file("hub/idProduct", b"3a05");
        fixture.file("hub/child/idVendor", b"1234");
        fixture.file("hub/child/idProduct", b"5678");
        let interface = fixture.directory("hub/child/interface");
        assert_eq!(usb_parent(&interface), Some(child.clone()));
        assert!(!is_controller(&child));
        assert!(is_controller(&hub));
    }

    #[test]
    fn reads_actual_content_and_rejects_oversize() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.file("descriptor", &[0x85, 6, 0x75, 8, 0x95, 15, 0x81, 2]);
        assert!(valid_descriptor(&bounded_read(&path, 4096).unwrap()));
        assert!(bounded_read(&path, 7).is_err());
    }

    #[test]
    fn sysfs_fixture_groups_all_owned_interfaces_and_excludes_sibling_usb() {
        use std::os::unix::fs::symlink;
        let fixture = crate::test_support::Fixture::new();
        let class = fixture.directory("class");
        fixture.directory("class/input");
        fixture.directory("class/hidraw");
        let usb = fixture.directory("hub/usb");
        for parent in ["hub", "hub/usb", "hub/other"] {
            fixture.file(&format!("{parent}/idVendor"), b"1b1c\n");
            fixture.file(&format!("{parent}/idProduct"), b"3a05\n");
        }
        let interface = fixture.file("hub/usb/interface03/bInterfaceNumber", b"03\n");
        let hid = fixture.directory("hub/usb/interface03/hid");
        fixture.file(
            "hub/usb/interface03/hid/report_descriptor",
            &[0x85, 6, 0x75, 8, 0x95, 15, 0x81, 2],
        );
        let raw = fixture.directory("hub/usb/interface03/hid/hidraw/hidraw0");
        let device = raw.join("device");
        symlink(&hid, &device).unwrap();
        fixture.track(device);
        let class_raw = class.join("hidraw/hidraw0");
        symlink(&raw, &class_raw).unwrap();
        fixture.track(class_raw);
        for i in 0..7 {
            let relative = if i < 6 {
                format!("hub/usb/interface{i}/event{i}")
            } else {
                "hub/other/event6".into()
            };
            let event = fixture.directory(&relative);
            let link = class.join(format!("input/event{i}"));
            symlink(event, &link).unwrap();
            fixture.track(link);
        }
        let dev = fixture.root.join("fake-dev");
        let found = discover(&class, &dev).unwrap().unwrap();
        assert_eq!(found.usb, usb);
        assert_eq!(found.events.len(), 6);
        assert!(!found.events.contains(&dev.join("input/event6")));
        assert_eq!(found.hidraw, dev.join("hidraw0"));
        fs::write(interface, b"04").unwrap();
        assert!(discover(&class, &dev).unwrap().is_none());
    }
}
