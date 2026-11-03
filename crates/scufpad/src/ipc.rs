use anyhow::{Context, Result, bail, ensure};
use scufpad_core::{
    BridgeState, ControlCommand, ControlRequest, ControlResponse, RemapProfile, read_message,
    write_message,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(3);
const CONNECTION_WORKERS: usize = 4;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireRequest {
    command: ControlCommand,
    #[serde(default)]
    profile: Option<RemapProfile>,
}

struct DeadlineStream<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}
impl DeadlineStream<'_> {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Control I/O deadline expired"))
    }
}
impl Read for DeadlineStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(buffer)
    }
}
impl Write for DeadlineStream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

fn probe_endpoint(path: &Path) -> io::Result<()> {
    let descriptor = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    for (out, &byte) in address.sun_path.iter_mut().zip(bytes) {
        *out = byte as libc::c_char;
    }
    let length = (size_of::<libc::sa_family_t>() + bytes.len() + 1) as libc::socklen_t;
    let result = unsafe {
        libc::connect(
            descriptor.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length,
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Clone)]
pub struct Snapshot {
    pub state: BridgeState,
    pub profile: Arc<RemapProfile>,
}

pub struct Pending {
    pub request: ControlRequest,
    pub deadline: Instant,
    pub execution: Arc<AtomicU8>,
    pub reply: SyncSender<ControlResponse>,
}

impl Pending {
    pub fn start(&self) -> bool {
        Instant::now() < self.deadline
            && self
                .execution
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
}

pub fn failure(error: impl ToString) -> ControlResponse {
    ControlResponse {
        error: Some(error.to_string()),
        ..Default::default()
    }
}

fn owned_socket(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.file_type().is_socket() && metadata.uid() == unsafe { libc::geteuid() },
        "Refusing foreign or non-socket endpoint"
    );
    Ok(metadata)
}

fn same_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

pub struct Endpoint {
    pub listener: UnixListener,
    path: PathBuf,
    identity: fs::Metadata,
    _lock: File,
}

impl Endpoint {
    pub fn bind(path: PathBuf) -> Result<Self> {
        let lock_path = PathBuf::from(format!("{}.lock", path.display()));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&lock_path)
            .context("Open singleton lock")?;
        let metadata = lock.metadata()?;
        ensure!(
            metadata.is_file()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o777 == 0o600
                && metadata.nlink() == 1,
            "Singleton lock must be an owned private regular file"
        );
        ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Daemon singleton lock is held"
        );
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let identity = owned_socket(&path)?;
                match probe_endpoint(&path) {
                    Ok(()) => bail!("An active daemon already owns this endpoint"),
                    Err(error) if error.raw_os_error() == Some(libc::ECONNREFUSED) => {
                        ensure!(
                            same_inode(&identity, &owned_socket(&path)?),
                            "Endpoint changed during stale check"
                        );
                        fs::remove_file(&path)?;
                    }
                    Err(error) => {
                        return Err(error).context("Existing endpoint is not safely stale");
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        // Startup runs before threads or signal handlers, so changing umask is confined here.
        let previous_umask = unsafe { libc::umask(0o177) };
        let bound = UnixListener::bind(&path);
        unsafe {
            libc::umask(previous_umask);
        }
        let listener = bound?;
        let identity = owned_socket(&path)?;
        let endpoint = Self {
            listener,
            path,
            identity,
            _lock: lock,
        };
        fs::set_permissions(&endpoint.path, fs::Permissions::from_mode(0o600))?;
        endpoint.listener.set_nonblocking(true)?;
        Ok(endpoint)
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        if let Ok(metadata) = owned_socket(&self.path)
            && same_inode(&metadata, &self.identity)
        {
            let _ = fs::remove_file(&self.path);
        }
        // Keep the private lock inode: unlinking a flock file permits split-lock races.
        // Closing the File releases only this process's lock, including during unwinding.
    }
}

fn same_user(stream: &UnixStream) -> Result<()> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    ensure!(
        result == 0
            && length as usize == size_of::<libc::ucred>()
            && credentials.uid == unsafe { libc::geteuid() },
        "Control peer UID rejected"
    );
    Ok(())
}

fn submit(request: ControlRequest, commands: &SyncSender<Pending>) -> ControlResponse {
    let (reply, receive) = mpsc::sync_channel(1);
    let execution = Arc::new(AtomicU8::new(0));
    let pending = Pending {
        request,
        deadline: Instant::now() + DEADLINE,
        execution: execution.clone(),
        reply,
    };
    if commands.try_send(pending).is_err() {
        return failure("Control queue full or daemon stopped");
    }
    match receive.recv_timeout(DEADLINE) {
        Ok(response) => response,
        Err(_) => {
            if execution
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                failure("Command deadline expired; queued command cancelled")
            } else {
                failure("Command deadline expired after execution began; query daemon state")
            }
        }
    }
}

fn respond(
    request: ControlRequest,
    commands: &SyncSender<Pending>,
    snapshot: &RwLock<Snapshot>,
) -> ControlResponse {
    if request.command != ControlCommand::ApplyProfile && request.profile.is_some() {
        return failure("Profile is only valid with ApplyProfile");
    }
    match request.command {
        ControlCommand::GetState | ControlCommand::GetProfile => {
            let current = match snapshot.read() {
                Ok(current) => current.clone(),
                Err(_) => return failure("State unavailable"),
            };
            ControlResponse {
                success: true,
                state: Some(current.state),
                profile: (request.command == ControlCommand::GetProfile)
                    .then(|| (*current.profile).clone()),
                error: None,
            }
        }
        _ => submit(request, commands),
    }
}

pub fn serve(
    endpoint: Endpoint,
    commands: SyncSender<Pending>,
    snapshot: Arc<RwLock<Snapshot>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    thread::scope(|scope| {
        let (send, receive) = mpsc::sync_channel::<UnixStream>(CONNECTION_WORKERS);
        let receive = Arc::new(Mutex::new(receive));
        let mut workers = Vec::with_capacity(CONNECTION_WORKERS);
        let result = (|| -> Result<()> {
            for index in 0..CONNECTION_WORKERS {
                let receive = receive.clone();
                let commands = &commands;
                let snapshot = &snapshot;
                let stop = &stop;
                workers.push(
                    thread::Builder::new()
                        .name(format!("scufpad-control-{index}"))
                        .spawn_scoped(scope, move || {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    loop {
                                        let connection =
                                            receive.lock().expect("Control queue poisoned").recv();
                                        let Ok(stream) = connection else { break };
                                        if stop.load(Ordering::Acquire) {
                                            break;
                                        }
                                        if let Err(error) =
                                            serve_connection(stream, commands, snapshot, stop)
                                        {
                                            eprintln!("Control connection failed: {error}");
                                        }
                                    }
                                }));
                            if let Err(panic) = result {
                                stop.store(true, Ordering::Release);
                                std::panic::resume_unwind(panic);
                            }
                        })?,
                );
            }

            let mut readiness = libc::pollfd {
                fd: endpoint.listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            while !stop.load(Ordering::Acquire) {
                let ready = unsafe { libc::poll(&mut readiness, 1, 100) };
                if ready < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error.into());
                }
                ensure!(
                    readiness.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) == 0,
                    "Control listener poll failed"
                );
                if ready == 0 {
                    continue;
                }
                while !stop.load(Ordering::Acquire) {
                    match endpoint.listener.accept() {
                        Ok((stream, _)) => match send.try_send(stream) {
                            Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                            Err(mpsc::TrySendError::Disconnected(_)) => {
                                bail!("Control workers disconnected")
                            }
                        },
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            Ok(())
        })();
        stop.store(true, Ordering::Release);
        drop(send);
        let mut panicked = false;
        for worker in workers {
            panicked |= worker.join().is_err();
        }
        ensure!(!panicked, "Control worker panicked");
        result
    })
}

fn serve_connection(
    mut stream: UnixStream,
    commands: &SyncSender<Pending>,
    snapshot: &RwLock<Snapshot>,
    stop: &AtomicBool,
) -> Result<()> {
    same_user(&stream)?;
    let mut input = DeadlineStream {
        stream: &mut stream,
        deadline: Instant::now() + DEADLINE,
    };
    let request = read_message::<WireRequest>(&mut input);
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let (response, shutdown) = match request {
        Ok(wire) => {
            let request = ControlRequest {
                command: wire.command,
                profile: wire.profile,
            };
            let shutdown = request.command == ControlCommand::Shutdown;
            let response = respond(request, commands, snapshot);
            let shutdown = shutdown && response.success;
            (response, shutdown)
        }
        Err(error) => (failure(format!("Invalid control request: {error}")), false),
    };
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut output = DeadlineStream {
        stream: &mut stream,
        deadline: Instant::now() + DEADLINE,
    };
    let result = write_message(&mut output, &response);
    // The input owner has already neutralized. Send the acknowledgement before stopping.
    if shutdown {
        stop.store(true, Ordering::Release);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn connection_pool_rejects_overflow_and_drains_queued_clients() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        fixture.track(fixture.root.join("control.lock"));
        let endpoint = Endpoint::bind(path.clone()).unwrap();
        let snapshot = Arc::new(RwLock::new(Snapshot {
            state: BridgeState::default(),
            profile: Arc::new(RemapProfile::default()),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (send, receive) = mpsc::sync_channel(32);
        let server_stop = stop.clone();
        let server = thread::spawn(move || serve(endpoint, send, snapshot, server_stop));
        let mut waiting = Vec::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let connect = |command| {
                let mut stream = UnixStream::connect(&path).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                write_message(
                    &mut stream,
                    &ControlRequest {
                        command,
                        profile: None,
                    },
                )
                .unwrap();
                stream
            };
            let mut active = Vec::new();
            for _ in 0..CONNECTION_WORKERS {
                active.push(connect(ControlCommand::StopAll));
                waiting.push(receive.recv_timeout(Duration::from_secs(1)).unwrap());
            }
            let mut queued = (0..CONNECTION_WORKERS)
                .map(|_| connect(ControlCommand::GetState))
                .collect::<Vec<_>>();
            let mut overflow = UnixStream::connect(&path).unwrap();
            overflow
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let rejected = match write_message(&mut overflow, &ControlRequest::default()) {
                Ok(()) => match overflow.read(&mut [0]) {
                    Ok(0) => true,
                    Err(error) => matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
                    ),
                    _ => false,
                },
                Err(error) => error.downcast_ref::<io::Error>().is_some_and(|error| {
                    matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
                    )
                }),
            };
            assert!(
                rejected,
                "an over-capacity connection was not closed promptly"
            );
            for pending in waiting.drain(..) {
                assert!(pending.start());
                pending
                    .reply
                    .send(ControlResponse {
                        success: true,
                        ..Default::default()
                    })
                    .unwrap();
            }
            for stream in active.iter_mut().chain(&mut queued) {
                assert!(read_message::<ControlResponse>(stream).unwrap().success);
            }
        }));
        stop.store(true, Ordering::Release);
        for pending in waiting {
            let _ = pending.reply.send(failure("Test cleanup"));
        }
        server.join().unwrap().unwrap();
        assert!(!path.exists());
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }

    #[test]
    fn endpoint_is_private_singleton_and_cleanup_is_owned() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        let endpoint = Endpoint::bind(path.clone()).unwrap();
        fixture.track(path.with_file_name("control.lock"));
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert!(Endpoint::bind(path.clone()).is_err());
        let client = UnixStream::connect(&path).unwrap();
        same_user(&client).unwrap();
        drop(endpoint);
        assert!(!path.exists());
    }

    #[test]
    fn stale_socket_is_reclaimed_but_regular_files_and_symlinks_are_not() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        drop(UnixListener::bind(&path).unwrap());
        fixture.track(fixture.root.join("control.lock"));
        let endpoint = Endpoint::bind(path.clone()).unwrap();
        drop(endpoint);
        fs::write(&path, b"keep").unwrap();
        fixture.track(path.clone());
        assert!(Endpoint::bind(path.clone()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        fs::remove_file(&path).unwrap();
        symlink(fixture.root.join("missing"), &path).unwrap();
        assert!(Endpoint::bind(path).is_err());
    }

    #[test]
    fn active_socket_is_never_replaced_even_without_our_lock() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        let listener = UnixListener::bind(&path).unwrap();
        fixture.track(path.clone());
        fixture.track(fixture.root.join("control.lock"));
        let inode = fs::metadata(&path).unwrap().ino();
        assert!(Endpoint::bind(path.clone()).is_err());
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        drop(listener);
    }

    #[test]
    fn expired_and_cancelled_commands_cannot_start() {
        let (reply, _) = mpsc::sync_channel(1);
        let mut pending = Pending {
            request: ControlRequest::default(),
            deadline: Instant::now() - Duration::from_secs(1),
            execution: Arc::new(AtomicU8::new(0)),
            reply,
        };
        assert!(!pending.start());
        pending.deadline = Instant::now() + DEADLINE;
        pending.execution.store(2, Ordering::Release);
        assert!(!pending.start());
    }

    #[test]
    fn wire_rejects_missing_unknown_and_invalid_commands_and_lengths() {
        for json in [
            "{}",
            "null",
            "{\"command\":\"Missing\"}",
            "{\"command\":\"GetState\",\"extra\":true}",
        ] {
            let mut bytes = (json.len() as i32).to_le_bytes().to_vec();
            bytes.extend_from_slice(json.as_bytes());
            assert!(read_message::<WireRequest>(&mut bytes.as_slice()).is_err());
        }
        for length in [-1i32, 0, 1_048_577] {
            assert!(read_message::<WireRequest>(&mut length.to_le_bytes().as_slice()).is_err());
        }
    }

    #[test]
    fn absolute_deadline_does_not_restart_on_partial_reads() {
        let (mut peer, mut stream) = UnixStream::pair().unwrap();
        peer.write_all(b"a").unwrap();
        let mut deadline = DeadlineStream {
            stream: &mut stream,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        let mut byte = [0];
        deadline.read_exact(&mut byte).unwrap();
        deadline.deadline = Instant::now() - Duration::from_millis(1);
        peer.write_all(b"b").unwrap();
        assert_eq!(
            deadline.read(&mut byte).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn lock_symlinks_and_nonprivate_files_are_not_followed_or_modified() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        let target = fixture.file("target", b"keep");
        let lock_path = fixture.root.join("control.lock");
        symlink(&target, &lock_path).unwrap();
        fixture.track(lock_path.clone());
        assert!(Endpoint::bind(path.clone()).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        fs::remove_file(&lock_path).unwrap();
        fs::write(&lock_path, b"private-required").unwrap();
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Endpoint::bind(path).is_err());
        assert_eq!(fs::read(lock_path).unwrap(), b"private-required");
    }

    #[test]
    fn full_command_queue_rejects_submission_without_waiting() {
        let (commands, _receiver) = mpsc::sync_channel(1);
        let (reply, _reply_receiver) = mpsc::sync_channel(1);
        commands
            .try_send(Pending {
                request: ControlRequest::default(),
                deadline: Instant::now() + DEADLINE,
                execution: Arc::new(AtomicU8::new(0)),
                reply,
            })
            .unwrap_or_else(|_| panic!("empty queue rejected submission"));
        let response = submit(ControlRequest::default(), &commands);
        assert!(!response.success);
        assert!(response.error.unwrap().contains("queue full"));
    }
}
