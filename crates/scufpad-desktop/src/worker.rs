use crate::edits::{launch_plan, start_once};
use anyhow::{Context, Result, bail, ensure};
use scufpad_core::*;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub enum Action {
    Start,
    Command(ControlCommand),
    Reload {
        revision: u64,
    },
    Load {
        path: PathBuf,
        revision: u64,
    },
    Import {
        revision: u64,
    },
    Save {
        profile: RemapProfile,
        path: Option<PathBuf>,
        revision: u64,
        apply: bool,
    },
    Export(RemapProfile),
    Library,
}

pub enum Event {
    Status(Status),
    InitialProfile(RemapProfile),
    Loaded {
        profile: RemapProfile,
        path: Option<PathBuf>,
        revision: u64,
        imported: bool,
    },
    Saved {
        path: PathBuf,
        revision: u64,
    },
    Applied {
        revision: u64,
        name: String,
    },
    Library(Vec<(PathBuf, String)>),
    Notice(Result<String, String>),
    Done(Result<String, String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub state: Option<BridgeState>,
    pub starting: bool,
    pub error: Option<String>,
}

pub struct Worker {
    actions: SyncSender<(Action, u64)>,
    pub events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    stop_all: Arc<AtomicBool>,
    stop_generation: Arc<AtomicU64>,
    wake: thread::Thread,
}
impl Worker {
    pub fn spawn(ctx: eframe::egui::Context) -> Self {
        let (actions, rx) = mpsc::sync_channel(8);
        let (tx, events) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop_all = Arc::new(AtomicBool::new(false));
        let stop_generation = Arc::new(AtomicU64::new(0));
        let canceled = cancel.clone();
        let emergency = stop_all.clone();
        let generation = stop_generation.clone();
        let thread = thread::Builder::new()
            .name("scufpad-editor-io".into())
            .spawn(move || {
                run(rx, tx, canceled, emergency, generation, ctx);
            })
            .expect("Could not create editor I/O worker");
        Self {
            actions,
            events,
            cancel,
            stop_all,
            stop_generation,
            wake: thread.thread().clone(),
        }
    }
    pub fn send(&self, action: Action) -> Result<(), String> {
        self.actions
            .try_send((action, self.stop_generation.load(Ordering::Acquire)))
            .map_err(|error| error.to_string())?;
        self.wake.unpark();
        Ok(())
    }
    pub fn stop_outputs(&self) {
        self.stop_generation.fetch_add(1, Ordering::AcqRel);
        self.stop_all.store(true, Ordering::Release);
        self.wake.unpark();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.wake.unpark();
    }
}

fn rpc(command: ControlCommand, profile: Option<RemapProfile>) -> Result<ControlResponse> {
    let response = client_request(&ControlRequest { command, profile })?;
    ensure!(
        response.success,
        "{}",
        response
            .error
            .as_deref()
            .unwrap_or("Bridge rejected request.")
    );
    Ok(response)
}

fn resolve_bridge() -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let executable = |path: &Path| {
        path.is_file()
            && path
                .metadata()
                .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    };
    if let Some(explicit) = std::env::var_os("SCUFPAD_BRIDGE").filter(|s| !s.is_empty()) {
        let path = PathBuf::from(explicit);
        ensure!(
            executable(&path),
            "SCUFPAD_BRIDGE is not an executable file: {}",
            path.display()
        );
        return Ok(path);
    }
    let sibling = std::env::current_exe()?.with_file_name("scufpad");
    if executable(&sibling) {
        return Ok(sibling);
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join("scufpad");
        if executable(&path) {
            return Ok(path);
        }
    }
    bail!(
        "Native bridge not installed. Set SCUFPAD_BRIDGE, place scufpad beside the editor, or install scufpad on PATH. No build or .NET fallback is used."
    )
}

fn start(children: &mut Vec<Child>, canceled: &AtomicBool) -> Result<bool> {
    start_once(
        || {
            client_request(&ControlRequest {
                command: ControlCommand::GetState,
                profile: None,
            })
        },
        || {
            ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
            let plan = launch_plan(&resolve_bridge()?)?;
            ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
            let child = Command::new(&plan.executable)
                .args(&plan.args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| format!("Could not start {}", plan.executable.display()))?;
            children.push(child);
            Ok(())
        },
    )
}

fn emit(tx: &SyncSender<Event>, ctx: &eframe::egui::Context, event: Event) -> bool {
    let sent = tx.send(event).is_ok();
    ctx.request_repaint();
    sent
}

fn publish_status(
    tx: &SyncSender<Event>,
    ctx: &eframe::egui::Context,
    status: Status,
    previous: &mut Option<Status>,
) -> bool {
    if previous.as_ref() == Some(&status) {
        return true;
    }
    match tx.try_send(Event::Status(status.clone())) {
        Ok(()) => {
            *previous = Some(status);
            ctx.request_repaint();
            true
        }
        Err(mpsc::TrySendError::Full(_)) => true,
        Err(mpsc::TrySendError::Disconnected(_)) => false,
    }
}

fn new_profile_path() -> Result<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = profile_dir().join(format!("profile-{}-{stamp}.json", std::process::id()));
    ensure!(
        !path.try_exists()? && std::fs::symlink_metadata(&path).is_err(),
        "Generated profile already exists; retry Save."
    );
    Ok(path)
}

fn library() -> Result<Vec<(PathBuf, String)>> {
    let directory = profile_dir();
    if !directory.try_exists()? {
        return Ok(vec![]);
    }
    let mut profiles = vec![];
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = match read_profile(&path) {
                Ok(profile) => profile.name,
                Err(error) => format!(
                    "Unreadable: {} — {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            };
            profiles.push((path, name));
        }
    }
    profiles.sort_by_key(|profile| profile.1.to_lowercase());
    Ok(profiles)
}

fn handle(
    action: Action,
    tx: &SyncSender<Event>,
    ctx: &eframe::egui::Context,
    canceled: &AtomicBool,
) -> Result<String> {
    ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
    match action {
        Action::Command(command) => {
            rpc(command, None)?;
            Ok(match command {
                ControlCommand::Arm => {
                    "Outputs requested. Normal gamepad forwarding needs no setup; custom mappings require six verified inputs."
                }
                ControlCommand::StopAll => "All outputs stopped. Bridge remains alive.",
                ControlCommand::Shutdown => {
                    "Bridge shutdown acknowledged. It will not restart automatically."
                }
                _ => "Bridge acknowledged request.",
            }
            .into())
        }
        Action::Reload { revision } => {
            let profile = rpc(ControlCommand::GetProfile, None)?
                .profile
                .context("Bridge returned no profile.")?;
            validate_profile(&profile)?;
            emit(
                tx,
                ctx,
                Event::Loaded {
                    profile,
                    path: None,
                    revision,
                    imported: false,
                },
            );
            Ok("Bridge profile loaded. Workspace edits were not applied.".into())
        }
        Action::Load { path, revision } => {
            let profile = read_profile(&path)?;
            emit(
                tx,
                ctx,
                Event::Loaded {
                    profile,
                    path: Some(path),
                    revision,
                    imported: false,
                },
            );
            Ok("Local profile loaded; apply explicitly when ready.".into())
        }
        Action::Import { revision } => {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Import Scufpad profile")
                .add_filter("JSON profile", &["json"])
                .pick_file()
            else {
                return Ok("Import canceled.".into());
            };
            ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
            let profile = read_profile(&path)?;
            emit(
                tx,
                ctx,
                Event::Loaded {
                    profile,
                    path: None,
                    revision,
                    imported: true,
                },
            );
            Ok("Imported and validated. Save locally to retain this workspace.".into())
        }
        Action::Save {
            profile,
            path,
            revision,
            apply,
        } => {
            validate_profile(&profile)?;
            if apply {
                rpc(ControlCommand::ApplyProfile, Some(profile.clone()))?;
                emit(
                    tx,
                    ctx,
                    Event::Applied {
                        revision,
                        name: profile.name.clone(),
                    },
                );
            }
            let path = (|| -> Result<PathBuf> {
                let path = match path {
                    Some(path) => path,
                    None => new_profile_path()?,
                };
                write_profile(&path, &profile)?;
                Ok(path)
            })()
            .map_err(|error| {
                if apply {
                    anyhow::anyhow!("Bridge applied profile, but local save failed: {error}")
                } else {
                    error
                }
            })?;
            emit(tx, ctx, Event::Saved { path, revision });
            if let Ok(items) = library() {
                emit(tx, ctx, Event::Library(items));
            }
            Ok(if apply {
                "Bridge acknowledged profile. Local private copy saved."
            } else {
                "Local private copy saved. Bridge was not changed."
            }
            .into())
        }
        Action::Export(profile) => {
            validate_profile(&profile)?;
            let Some(path) = rfd::FileDialog::new()
                .set_title("Export Scufpad profile")
                .add_filter("JSON profile", &["json"])
                .set_file_name("scufpad-profile.json")
                .save_file()
            else {
                return Ok("Export canceled.".into());
            };
            ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
            if std::fs::symlink_metadata(&path).is_ok()
                && rfd::MessageDialog::new()
                    .set_title("Replace exported profile?")
                    .set_description(format!("Replace {}?", path.display()))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show()
                    != rfd::MessageDialogResult::Yes
            {
                return Ok("Export canceled; existing file retained.".into());
            }
            ensure!(!canceled.load(Ordering::Acquire), "Editor is closing.");
            write_profile(&path, &profile)?;
            Ok(format!("Private JSON exported to {}", path.display()))
        }
        Action::Library => {
            emit(tx, ctx, Event::Library(library()?));
            Ok("Local library refreshed.".into())
        }
        Action::Start => unreachable!("Start is handled by the worker lifecycle"),
    }
}

pub fn stale_arm(command: ControlCommand, queued_generation: u64, stop_generation: u64) -> bool {
    command == ControlCommand::Arm && queued_generation != stop_generation
}

fn run(
    rx: Receiver<(Action, u64)>,
    tx: SyncSender<Event>,
    canceled: Arc<AtomicBool>,
    emergency: Arc<AtomicBool>,
    stop_generation: Arc<AtomicU64>,
    ctx: eframe::egui::Context,
) {
    let mut children = vec![];
    let attach_only = std::env::var_os("SCUFPAD_TRAY_ATTACH_ONLY").is_some();
    let mut starting_until = (!attach_only).then(|| Instant::now() + Duration::from_secs(8));
    let mut initial_profile_sent = false;
    let _ = emit(
        &tx,
        &ctx,
        Event::Status(Status {
            state: None,
            starting: !attach_only,
            error: None,
        }),
    );
    if !attach_only && let Err(error) = start(&mut children, &canceled) {
        starting_until = None;
        emit(&tx, &ctx, Event::Notice(Err(error.to_string())));
    }
    // Only startup and an explicit Start action can launch. Polling never launches.
    let mut next_poll = Instant::now();
    let mut previous_status = None;
    while !canceled.load(Ordering::Acquire) {
        if emergency.swap(false, Ordering::AcqRel) {
            let result = rpc(ControlCommand::StopAll, None)
                .map(|_| "All outputs stopped. Bridge remains alive.".into())
                .map_err(|e| e.to_string());
            if !emit(&tx, &ctx, Event::Notice(result)) {
                break;
            }
        }
        let action = match rx.try_recv() {
            Ok(action) => Some(action),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => break,
        };
        if let Some((action, queued_generation)) = action {
            if canceled.load(Ordering::Acquire) {
                break;
            }
            if let Action::Command(command) = &action
                && stale_arm(
                    *command,
                    queued_generation,
                    stop_generation.load(Ordering::Acquire),
                )
            {
                emit(
                    &tx,
                    &ctx,
                    Event::Done(Err(
                        "Pending Enable was canceled by Stop All. Enable again only when ready."
                            .into(),
                    )),
                );
                continue;
            }
            if matches!(
                action,
                Action::Import { .. }
                    | Action::Export(_)
                    | Action::Library
                    | Action::Load { .. }
                    | Action::Save { .. }
            ) {
                // File scans and native pickers can be slow. Keep telemetry and Stop All
                // on the I/O worker while the single pending file operation completes.
                let file_tx = tx.clone();
                let file_ctx = ctx.clone();
                let file_canceled = canceled.clone();
                if let Err(error) = thread::Builder::new()
                    .name("scufpad-editor-file-io".into())
                    .spawn(move || {
                        let result = handle(action, &file_tx, &file_ctx, &file_canceled)
                            .map_err(|e| e.to_string());
                        emit(&file_tx, &file_ctx, Event::Done(result));
                    })
                {
                    emit(
                        &tx,
                        &ctx,
                        Event::Done(Err(format!("Could not start file operation: {error}"))),
                    );
                }
                continue;
            }
            let result = if matches!(action, Action::Start) {
                previous_status = None;
                starting_until = Some(Instant::now() + Duration::from_secs(8));
                emit(
                    &tx,
                    &ctx,
                    Event::Status(Status {
                        state: None,
                        starting: true,
                        error: None,
                    }),
                );
                start(&mut children, &canceled).map(|launched| {
                    if launched {
                        "Native bridge started with automatic gamepad forwarding; waiting for IPC."
                    } else {
                        "Attached to existing bridge; no process launched."
                    }
                    .into()
                })
            } else {
                if matches!(action, Action::Command(ControlCommand::Shutdown)) {
                    starting_until = None;
                }
                handle(action, &tx, &ctx, &canceled)
            };
            if result.is_err() {
                starting_until = None;
            }
            if !emit(&tx, &ctx, Event::Done(result.map_err(|e| e.to_string()))) {
                break;
            }
            next_poll = Instant::now();
        }
        if Instant::now() >= next_poll && !canceled.load(Ordering::Acquire) {
            let (state, error) = match rpc(ControlCommand::GetState, None)
                .and_then(|r| r.state.context("Bridge returned no state."))
            {
                Ok(state) => {
                    starting_until = None;
                    (Some(state), None)
                }
                Err(error) => (None, Some(error.to_string())),
            };
            let starting = starting_until.is_some_and(|deadline| Instant::now() < deadline);
            if !publish_status(
                &tx,
                &ctx,
                Status {
                    state: state.clone(),
                    starting,
                    error,
                },
                &mut previous_status,
            ) {
                break;
            }
            if state.is_some() && !initial_profile_sent {
                match rpc(ControlCommand::GetProfile, None)
                    .and_then(|r| r.profile.context("Bridge returned no profile."))
                    .and_then(crate::edits::checked)
                {
                    Ok(profile) => {
                        initial_profile_sent = true;
                        if !emit(&tx, &ctx, Event::InitialProfile(profile)) {
                            break;
                        }
                    }
                    Err(error) => {
                        initial_profile_sent = true;
                        emit(
                            &tx,
                            &ctx,
                            Event::Notice(Err(format!(
                                "Bridge is running, but profile load failed: {error}"
                            ))),
                        );
                    }
                }
            }
            for child in &mut children {
                let _ = child.try_wait();
            }
            next_poll = Instant::now() + Duration::from_millis(50);
        }
        thread::park_timeout(next_poll.saturating_duration_since(Instant::now()));
    }
    // Dropping Child handles neither kills nor waits for the independent daemon.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_change_driven_and_full_queues_retry_delivery() {
        let ctx = eframe::egui::Context::default();
        let (tx, rx) = mpsc::sync_channel(1);
        let initial = Status {
            state: Some(BridgeState::default()),
            starting: false,
            error: None,
        };
        let mut previous = None;
        assert!(publish_status(&tx, &ctx, initial.clone(), &mut previous));
        assert!(matches!(rx.try_recv(), Ok(Event::Status(status)) if status == initial));
        assert!(publish_status(&tx, &ctx, initial.clone(), &mut previous));
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));

        for changed in [
            Status {
                starting: true,
                ..initial.clone()
            },
            Status {
                error: Some("Unavailable".into()),
                ..initial.clone()
            },
            Status {
                state: Some(BridgeState {
                    keyboard0: 1,
                    ..Default::default()
                }),
                ..initial.clone()
            },
            initial.clone(),
        ] {
            tx.try_send(Event::Notice(Ok("Reliable event".into())))
                .unwrap_or_else(|_| panic!("queue not empty"));
            let before = previous.clone();
            assert!(publish_status(&tx, &ctx, changed.clone(), &mut previous));
            assert_eq!(previous, before);
            assert!(matches!(rx.try_recv(), Ok(Event::Notice(_))));
            assert!(publish_status(&tx, &ctx, changed.clone(), &mut previous));
            assert!(matches!(rx.try_recv(), Ok(Event::Status(status)) if status == changed));
        }
        previous = None;
        assert!(publish_status(&tx, &ctx, initial.clone(), &mut previous));
        assert!(matches!(rx.try_recv(), Ok(Event::Status(status)) if status == initial));
        drop(rx);
        assert!(!publish_status(
            &tx,
            &ctx,
            Status {
                starting: true,
                ..initial
            },
            &mut previous
        ));
    }

    #[test]
    fn actions_emergency_stop_and_drop_wake_a_parked_worker() {
        let (actions, rx) = mpsc::sync_channel(8);
        let (_tx, events) = mpsc::sync_channel(32);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop_all = Arc::new(AtomicBool::new(false));
        let stop_generation = Arc::new(AtomicU64::new(0));
        let (ready, readiness) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        let canceled = cancel.clone();
        let emergency = stop_all.clone();
        let generation = stop_generation.clone();
        let thread = thread::spawn(move || {
            for stage in 0..3 {
                ready.send(stage).unwrap();
                thread::park_timeout(Duration::from_secs(2));
                let observed = match stage {
                    0 => matches!(rx.try_recv(), Ok((Action::Library, 0))),
                    1 => {
                        emergency.load(Ordering::Acquire) && generation.load(Ordering::Acquire) == 1
                    }
                    _ => canceled.load(Ordering::Acquire),
                };
                done.send(observed).unwrap();
            }
        });
        let worker = Worker {
            actions,
            events,
            cancel,
            stop_all,
            stop_generation,
            wake: thread.thread().clone(),
        };
        assert_eq!(readiness.recv_timeout(Duration::from_secs(1)).unwrap(), 0);
        assert!(worker.send(Action::Library).is_ok());
        assert!(completed.recv_timeout(Duration::from_secs(1)).unwrap());
        assert_eq!(readiness.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
        worker.stop_outputs();
        assert!(completed.recv_timeout(Duration::from_secs(1)).unwrap());
        assert_eq!(readiness.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
        drop(worker);
        assert!(completed.recv_timeout(Duration::from_secs(1)).unwrap());
        thread.join().unwrap();
    }
}
