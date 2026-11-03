use crate::{ControlRequest, ControlResponse, RemapProfile, validate_profile};
use anyhow::{Context, Result, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    ffi::{CStr, CString, OsStr},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
            net::UnixStream,
        },
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

pub fn read_message<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = i32::from_le_bytes(header);
    ensure!(
        (1..=MAX_MESSAGE_BYTES as i32).contains(&length),
        "Message length must be 1–{MAX_MESSAGE_BYTES} bytes."
    );
    let mut payload = vec![0; length as usize];
    stream.read_exact(&mut payload)?;
    ensure!(payload.trim_ascii() != b"null", "A message cannot be null.");
    Ok(serde_json::from_slice(&payload)?)
}

pub fn write_message<T: Serialize>(stream: &mut impl Write, message: &T) -> Result<()> {
    let payload = serde_json::to_vec(message)?;
    ensure!(
        !payload.is_empty() && payload.len() <= MAX_MESSAGE_BYTES,
        "Message exceeds {MAX_MESSAGE_BYTES} bytes."
    );
    ensure!(payload != b"null", "A message cannot be null.");
    stream.write_all(&(payload.len() as i32).to_le_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()?;
    Ok(())
}

fn account() -> Result<(String, PathBuf)> {
    let mut length = 16384;
    loop {
        let mut buffer = vec![0u8; length];
        let mut pwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let error = unsafe {
            libc::getpwuid_r(
                libc::getuid(),
                pwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if error == libc::ERANGE && length < MAX_MESSAGE_BYTES {
            length *= 2;
            continue;
        }
        ensure!(
            error == 0 && !result.is_null(),
            "Cannot resolve current user account."
        );
        let pwd = unsafe { pwd.assume_init() };
        ensure!(
            !pwd.pw_name.is_null() && !pwd.pw_dir.is_null(),
            "Account has no name or home directory."
        );
        let name = unsafe { CStr::from_ptr(pwd.pw_name) }.to_str()?.to_owned();
        let home = PathBuf::from(OsStr::from_bytes(
            unsafe { CStr::from_ptr(pwd.pw_dir) }.to_bytes(),
        ));
        return Ok((name, home));
    }
}

fn valid_pipe_name(name: &str) -> bool {
    (1..=48).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub fn socket_path() -> Result<PathBuf> {
    let name = match std::env::var("SCUFPAD_PIPE_NAME")
        .ok()
        .filter(|name| valid_pipe_name(name))
    {
        Some(name) => name,
        None => format!("scufpad-{}", account()?.0),
    };
    Ok(std::env::temp_dir().join(format!("CoreFxPipe_{name}")))
}

pub fn client_request(request: &ControlRequest) -> Result<ControlResponse> {
    let mut stream = connect_bounded(&socket_path()?, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    ensure!(
        length as usize == size_of::<libc::ucred>(),
        "Invalid peer credentials."
    );
    ensure!(
        unsafe { credentials.assume_init().uid == libc::getuid() },
        "The bridge belongs to another user."
    );
    write_message(&mut stream, request)?;
    read_message(&mut stream)
}

fn connect_bounded(path: &Path, timeout: Duration) -> Result<UnixStream> {
    let path = path.as_os_str().as_bytes();
    let mut address = unsafe { std::mem::zeroed::<libc::sockaddr_un>() };
    ensure!(
        !path.is_empty() && path.len() < address.sun_path.len() && !path.contains(&0),
        "Unix socket path must be nonempty, contain no NUL, and fit sockaddr_un."
    );
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (target, &byte) in address.sun_path.iter_mut().zip(path) {
        *target = byte as libc::c_char;
    }
    let socket = owned_fd(unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    })?;
    let deadline = Instant::now() + timeout;
    let result = unsafe {
        libc::connect(
            socket.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            (std::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1) as libc::socklen_t,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        // A full AF_UNIX backlog returns EAGAIN; fail immediately rather than retrying indefinitely.
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error.into());
        }
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Unix socket connect timed out.",
                )
                .into());
            }
            let mut poll = libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let milliseconds = remaining
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128) as i32;
            let result = unsafe { libc::poll(&mut poll, 1, milliseconds) };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            if result == 0 {
                continue;
            }
            let mut error = 0i32;
            let mut length = size_of_val(&error) as libc::socklen_t;
            let result = unsafe {
                libc::getsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_ERROR,
                    (&mut error as *mut i32).cast(),
                    &mut length,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            ensure!(
                length as usize == size_of_val(&error),
                "Invalid socket error response."
            );
            if error != 0 {
                return Err(std::io::Error::from_raw_os_error(error).into());
            }
            ensure!(
                poll.revents & libc::POLLOUT != 0,
                "Unix socket did not become writable."
            );
            break;
        }
    }
    let stream = UnixStream::from(socket);
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn xdg_directory(value: Option<PathBuf>, home: &Path, fallback: &str) -> PathBuf {
    value
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(fallback))
}
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| account().ok().map(|a| a.1))
        .unwrap_or_else(|| PathBuf::from("/"))
}
pub fn config_dir() -> PathBuf {
    std::env::var_os("SCUFPAD_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            xdg_directory(
                std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                &home_dir(),
                ".config",
            )
            .join("Scufpad")
        })
}
pub fn profile_dir() -> PathBuf {
    xdg_directory(
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        &home_dir(),
        ".local/share",
    )
    .join("Scufpad/profiles")
}

fn c_name(name: &OsStr) -> Result<CString> {
    Ok(CString::new(name.as_bytes())?)
}
fn owned_fd(fd: i32) -> Result<OwnedFd> {
    if fd < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

// Walk using directory descriptors so parent renames cannot redirect profile writes.
fn profile_parent(path: &Path, create: bool) -> Result<(OwnedFd, CString)> {
    let full = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let filename = c_name(full.file_name().context("Profile path has no filename.")?)?;
    let mut directory = owned_fd(unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    })?;
    for component in full
        .parent()
        .context("Profile has no parent directory.")?
        .components()
    {
        let name = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(name) => c_name(name)?,
            _ => bail!("Profile paths cannot contain parent traversal."),
        };
        if create {
            let result = unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
            if result != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        directory = owned_fd(unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })?;
    }
    Ok((directory, filename))
}

fn check_target(directory: &OwnedFd, name: &CStr) -> Result<bool> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            return Ok(false);
        }
        return Err(error.into());
    }
    let stat = unsafe { stat.assume_init() };
    ensure!(
        stat.st_mode & libc::S_IFMT == libc::S_IFREG,
        "Profile target must be a regular file, not a symlink."
    );
    ensure!(
        stat.st_uid == unsafe { libc::getuid() },
        "Profile target belongs to another user."
    );
    Ok(true)
}

fn commit_profile(directory: &OwnedFd, temporary: &CStr, name: &CStr, replace: bool) -> Result<()> {
    // ponytail: existing-file replacement assumes one same-UID writer; add a cooperating file lock for concurrent writers.
    let result = unsafe {
        if replace {
            libc::renameat(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                directory.as_raw_fd(),
                name.as_ptr(),
            )
        } else {
            libc::renameat2(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        }
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn read_profile(path: &Path) -> Result<RemapProfile> {
    let (directory, name) = profile_parent(path, false)?;
    let fd = owned_fd(unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    })?;
    let file = File::from(fd);
    ensure!(
        file.metadata()?.is_file(),
        "Profile must be a regular file."
    );
    let mut bytes = Vec::new();
    file.take(MAX_MESSAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_MESSAGE_BYTES,
        "Profile exceeds 1 MiB limit."
    );
    let profile = serde_json::from_slice(&bytes)?;
    validate_profile(&profile)?;
    Ok(profile)
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn write_profile(path: &Path, profile: &RemapProfile) -> Result<()> {
    validate_profile(profile)?;
    let bytes = serde_json::to_vec(profile)?;
    ensure!(
        bytes.len() <= MAX_MESSAGE_BYTES,
        "Profile exceeds 1 MiB limit."
    );
    let (directory, name) = profile_parent(path, true)?;
    let metadata = File::from(directory.try_clone()?).metadata()?;
    ensure!(
        metadata.uid() == unsafe { libc::getuid() } && metadata.permissions().mode() & 0o022 == 0,
        "Profile parent directory must belong to the current user and not be group/world writable."
    );
    let replace = check_target(&directory, &name)?;
    let (temporary, fd) = loop {
        let temporary = CString::new(format!(
            ".scufpad-{}-{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))?;
        if temporary == name {
            continue;
        }
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd >= 0 {
            break (temporary, owned_fd(fd)?);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EEXIST) {
            return Err(error.into());
        }
    };
    let result = (|| -> Result<()> {
        let mut file = File::from(fd);
        ensure!(
            file.metadata()?.permissions().mode() & 0o777 == 0o600,
            "Filesystem cannot guarantee owner-only profile permissions."
        );
        file.write_all(&bytes)?;
        file.sync_all()?;
        check_target(&directory, &name)?;
        commit_profile(&directory, &temporary, &name, replace)
    })();
    // Only remove our single, exclusively-created temporary file.
    unsafe {
        libc::unlinkat(directory.as_raw_fd(), temporary.as_ptr(), 0);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_directory() -> PathBuf {
        use std::os::unix::fs::DirBuilderExt;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
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
            "scufpad-io-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        root
    }

    #[test]
    fn names_and_xdg_paths_are_pure_and_bounded() {
        for name in ["my_test.pipe-1", "a"] {
            assert!(valid_pipe_name(name));
        }
        for name in ["", "../bad/path", "with space", "é", &"a".repeat(49)] {
            assert!(!valid_pipe_name(name));
        }
        assert_eq!(
            xdg_directory(Some("relative".into()), Path::new("/home/test"), ".config"),
            Path::new("/home/test/.config")
        );
        assert_eq!(
            xdg_directory(Some("/data".into()), Path::new("/home/test"), ".config"),
            Path::new("/data")
        );
    }

    #[test]
    fn export_target_cannot_collide_with_its_temporary_filename() {
        let root = private_directory();
        let name = format!(
            ".scufpad-{}-{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.load(Ordering::Relaxed)
        );
        let path = root.join(name);
        let result = write_profile(&path, &RemapProfile::default());
        let saved = read_profile(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&root);
        result.unwrap();
        assert_eq!(saved.unwrap(), RemapProfile::default());
    }

    #[test]
    fn new_target_commit_does_not_replace_a_concurrently_created_file() {
        let root = private_directory();
        let target = root.join("profile.json");
        let temporary = root.join("temporary.json");
        let (directory, name) = profile_parent(&target, false).unwrap();
        let replace = check_target(&directory, &name).unwrap();
        assert!(!replace);
        std::fs::write(&temporary, b"replacement").unwrap();
        std::fs::write(&target, b"concurrent creation").unwrap();
        let result = commit_profile(&directory, c"temporary.json", &name, replace);
        let saved = std::fs::read(&target);
        let pending = std::fs::read(&temporary);
        std::fs::remove_file(&temporary).unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::remove_dir(&root).unwrap();
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(libc::EEXIST)
        );
        assert_eq!(saved.unwrap(), b"concurrent creation");
        assert_eq!(pending.unwrap(), b"replacement");
    }

    #[test]
    fn bounded_connect_uses_private_socket_and_returns_a_blocking_cloexec_stream() {
        let root = private_directory();
        let path = root.join("control.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let connected = connect_bounded(&path, Duration::from_secs(1));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
        let stream = connected.unwrap();
        let status = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFL) };
        let descriptor = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFD) };
        assert!(status >= 0 && status & libc::O_NONBLOCK == 0);
        assert!(descriptor >= 0 && descriptor & libc::FD_CLOEXEC != 0);
        drop(listener);
    }

    #[test]
    fn bounded_connect_rejects_invalid_paths_and_a_full_backlog_without_blocking() {
        for path in [
            PathBuf::new(),
            PathBuf::from("x".repeat(108)),
            PathBuf::from(OsStr::from_bytes(b"bad\0path")),
        ] {
            assert!(connect_bounded(&path, Duration::from_millis(50)).is_err());
        }
        let root = private_directory();
        let path = root.join("control.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
        let mut pending = Vec::new();
        let started = Instant::now();
        let mut failure = None;
        for _ in 0..4 {
            match connect_bounded(&path, Duration::from_millis(50)) {
                Ok(stream) => pending.push(stream),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let elapsed = started.elapsed();
        drop(pending);
        drop(listener);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
        let error = failure.expect("A zero-length Unix backlog must fill within four attempts.");
        let error = error.downcast_ref::<std::io::Error>().unwrap();
        assert!(
            error.raw_os_error() == Some(libc::EAGAIN)
                || error.kind() == std::io::ErrorKind::TimedOut
        );
        assert!(elapsed < Duration::from_secs(3));
    }
}
