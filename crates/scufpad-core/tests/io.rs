use scufpad_core::*;
use std::{
    cell::RefCell,
    fs::{self, DirBuilder},
    io::{self, Cursor, Read, Write},
    os::unix::fs::{DirBuilderExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fragmented<R>(R);
impl<R: Read> Read for Fragmented<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let length = bytes.len().min(1);
        self.0.read(&mut bytes[..length])
    }
}
fn frame(payload: &[u8]) -> Cursor<Vec<u8>> {
    let mut bytes = (payload.len() as i32).to_le_bytes().to_vec();
    bytes.extend(payload);
    Cursor::new(bytes)
}

#[test]
fn requests_roundtrip_with_profiles_enums_and_little_endian_length() {
    let mut profile = RemapProfile::default();
    profile.layers[0].bindings.insert(
        PhysicalControl::Guide,
        Binding {
            kind: BindingKind::Keys,
            keys: vec![29, 30],
            ..Binding::default()
        },
    );
    let request = ControlRequest {
        command: ControlCommand::ApplyProfile,
        profile: Some(profile),
    };
    let mut bytes = Vec::new();
    write_message(&mut bytes, &request).unwrap();
    assert_eq!(
        i32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
        bytes.len() - 4
    );
    let json = std::str::from_utf8(&bytes[4..]).unwrap();
    assert!(json.contains("\"command\":\"ApplyProfile\""));
    assert!(json.contains("\"Guide\":{\"kind\":\"Keys\""));
    let read: ControlRequest = read_message(&mut Fragmented(Cursor::new(bytes))).unwrap();
    assert_eq!(read, request);
}

#[test]
fn consecutive_responses_survive_fragmented_reads_and_preserve_unsigned_bitfields() {
    let first = ControlResponse {
        success: true,
        profile: Some(RemapProfile::default()),
        state: Some(BridgeState {
            connected: true,
            keyboard3: 1 << 63,
            gamepad_buttons: GamepadControl::Guide.bit(),
            ..BridgeState::default()
        }),
        ..ControlResponse::default()
    };
    let second = ControlResponse {
        success: false,
        error: Some("Rejected".into()),
        ..ControlResponse::default()
    };
    let mut bytes = Vec::new();
    write_message(&mut bytes, &first).unwrap();
    write_message(&mut bytes, &second).unwrap();
    let mut reader = Fragmented(Cursor::new(bytes));
    assert_eq!(read_message::<ControlResponse>(&mut reader).unwrap(), first);
    assert_eq!(
        read_message::<ControlResponse>(&mut reader).unwrap(),
        second
    );
    assert_eq!(reader.0.position() as usize, reader.0.get_ref().len());
}

#[test]
fn invalid_lengths_are_rejected_before_payload_is_read() {
    for length in [-1, 0, MAX_MESSAGE_BYTES as i32 + 1, i32::MAX] {
        let mut reader = Cursor::new(length.to_le_bytes());
        assert!(read_message::<ControlRequest>(&mut reader).is_err());
        assert_eq!(reader.position(), 4);
    }
}

#[test]
fn eof_null_and_malformed_json_are_rejected() {
    for bytes in [vec![], vec![0, 0, 0], vec![2, 0, 0, 0, b'{']] {
        let error =
            read_message::<ControlRequest>(&mut Fragmented(Cursor::new(bytes))).unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
    for payload in [
        b"null".as_slice(),
        b" \nnull\t",
        b"{",
        b"{} garbage",
        b"\xff",
    ] {
        assert!(read_message::<ControlResponse>(&mut frame(payload)).is_err());
    }
    let mut output = Vec::new();
    assert!(write_message(&mut output, &Option::<ControlResponse>::None).is_err());
    assert!(output.is_empty());
}

#[test]
fn oversized_write_emits_nothing_and_maximum_frame_is_accepted() {
    let mut output = Vec::new();
    let response = ControlResponse {
        error: Some("x".repeat(MAX_MESSAGE_BYTES)),
        ..ControlResponse::default()
    };
    assert!(write_message(&mut output, &response).is_err());
    assert!(output.is_empty());
    let mut payload = vec![b' '; MAX_MESSAGE_BYTES];
    let json = br#"{"command":"GetState"}"#;
    payload[..json.len()].copy_from_slice(json);
    let request: ControlRequest = read_message(&mut frame(&payload)).unwrap();
    assert_eq!(request.command, ControlCommand::GetState);
}

struct PartialWriter(Vec<u8>);
impl Write for PartialWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.0.push(bytes[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn framing_handles_partial_writes_and_propagates_write_errors() {
    let request = ControlRequest::default();
    let mut writer = PartialWriter(Vec::new());
    write_message(&mut writer, &request).unwrap();
    assert_eq!(
        read_message::<ControlRequest>(&mut Cursor::new(writer.0)).unwrap(),
        request
    );
    let mut full = [0; 2];
    assert!(write_message(&mut full.as_mut_slice(), &request).is_err());
}

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Temp {
    root: PathBuf,
    files: RefCell<Vec<PathBuf>>,
    directories: RefCell<Vec<PathBuf>>,
}
impl Temp {
    fn new() -> Self {
        let base = if Path::new("/tmp/opencode").is_dir() {
            PathBuf::from("/tmp/opencode")
        } else {
            std::env::temp_dir()
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = base.join(format!(
            "scufpad-core-test-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            root,
            files: RefCell::new(Vec::new()),
            directories: RefCell::new(Vec::new()),
        }
    }
    fn file(&self, name: &str) -> PathBuf {
        assert!(Path::new(name).components().count() == 1 && name != ".." && name != ".");
        let path = self.root.join(name);
        self.files.borrow_mut().push(path.clone());
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        for file in self.files.get_mut().iter().rev() {
            let _ = fs::remove_file(file);
        }
        for directory in self.directories.get_mut().iter().rev() {
            let _ = fs::remove_dir(directory);
        }
        let _ = fs::remove_dir(&self.root);
    }
}

#[test]
fn profiles_are_private_atomic_validated_and_roundtrip_without_home_writes() {
    let temp = Temp::new();
    let path = temp.file("profile.json");
    let mut profile = RemapProfile {
        name: "Original".into(),
        ..RemapProfile::default()
    };
    write_profile(&path, &profile).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(read_profile(&path).unwrap(), profile);
    let original_inode = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&path).unwrap());
    profile.name = "Replacement".into();
    write_profile(&path, &profile).unwrap();
    assert_ne!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&path).unwrap()),
        original_inode
    );
    assert_eq!(read_profile(&path).unwrap(), profile);
    let original_bytes = fs::read(&path).unwrap();
    assert!(
        write_profile(
            &path,
            &RemapProfile {
                version: 2,
                ..RemapProfile::default()
            }
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original_bytes);
    assert_eq!(fs::read_dir(&temp.root).unwrap().count(), 1);
}

#[test]
fn profile_storage_creates_private_directories() {
    let temp = Temp::new();
    let nested = temp.root.join("nested");
    temp.directories.borrow_mut().push(nested.clone());
    let path = nested.join("profile.json");
    temp.files.borrow_mut().push(path.clone());
    write_profile(&path, &RemapProfile::default()).unwrap();
    assert_eq!(
        fs::metadata(&nested).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(read_profile(&path).unwrap(), RemapProfile::default());
}

#[test]
fn profile_reads_are_size_bounded_and_validate_before_return() {
    let temp = Temp::new();
    let path = temp.file("profile.json");
    fs::write(&path, vec![b' '; MAX_MESSAGE_BYTES + 1]).unwrap();
    assert!(
        read_profile(&path)
            .unwrap_err()
            .to_string()
            .contains("1 MiB")
    );
    let mut maximum = vec![b' '; MAX_MESSAGE_BYTES];
    maximum[..2].copy_from_slice(b"{}");
    fs::write(&path, maximum).unwrap();
    assert_eq!(read_profile(&path).unwrap(), RemapProfile::default());
    for value in [
        b"null".as_slice(),
        b"{",
        br#"{"version":2}"#,
        br#"{"layers":null}"#,
    ] {
        fs::write(&path, value).unwrap();
        assert!(read_profile(&path).is_err());
    }
}

#[test]
fn oversized_valid_profile_write_preserves_existing_bytes() {
    let temp = Temp::new();
    let path = temp.file("profile.json");
    write_profile(&path, &RemapProfile::default()).unwrap();
    let original = fs::read(&path).unwrap();
    let oversized = RemapProfile {
        macros: (0..128)
            .map(|i| MacroDefinition {
                id: format!("M{i}"),
                steps: vec![
                    MacroStep {
                        kind: MacroStepKind::Delay,
                        ..MacroStep::default()
                    };
                    256
                ],
                ..MacroDefinition::default()
            })
            .collect(),
        ..RemapProfile::default()
    };
    validate_profile(&oversized).unwrap();
    assert!(serde_json::to_vec(&oversized).unwrap().len() > MAX_MESSAGE_BYTES);
    assert!(write_profile(&path, &oversized).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read_dir(&temp.root).unwrap().count(), 1);
}

#[test]
fn profile_storage_rejects_symlink_targets_parents_and_nonregular_files() {
    let temp = Temp::new();
    let original = temp.file("original.json");
    let link = temp.file("link.json");
    write_profile(&original, &RemapProfile::default()).unwrap();
    let before = fs::read(&original).unwrap();
    symlink(&original, &link).unwrap();
    assert!(read_profile(&link).is_err());
    assert!(write_profile(&link, &RemapProfile::default()).is_err());
    assert_eq!(fs::read(&original).unwrap(), before);
    let parent = temp.file("parent");
    symlink(&temp.root, &parent).unwrap();
    assert!(read_profile(&parent.join("original.json")).is_err());
    assert!(write_profile(&parent.join("new.json"), &RemapProfile::default()).is_err());
    assert!(!temp.root.join("new.json").exists());
    assert!(read_profile(&temp.root).is_err());
    assert!(write_profile(&temp.root, &RemapProfile::default()).is_err());
}

#[test]
fn writable_parent_rejects_updates_and_new_targets_without_mutating_files() {
    let temp = Temp::new();
    let existing = temp.file("profile.json");
    let absent = temp.file("new.json");
    write_profile(&existing, &RemapProfile::default()).unwrap();
    let original = fs::read(&existing).unwrap();
    for mode in [0o720, 0o702] {
        fs::set_permissions(&temp.root, fs::Permissions::from_mode(mode)).unwrap();
        let update = write_profile(
            &existing,
            &RemapProfile {
                name: "Changed".into(),
                ..RemapProfile::default()
            },
        );
        let create = write_profile(&absent, &RemapProfile::default());
        fs::set_permissions(&temp.root, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(update.unwrap_err().to_string().contains("parent directory"));
        assert!(create.unwrap_err().to_string().contains("parent directory"));
        assert_eq!(fs::read(&existing).unwrap(), original);
        assert!(!absent.exists());
        assert_eq!(fs::read_dir(&temp.root).unwrap().count(), 1);
    }
}
