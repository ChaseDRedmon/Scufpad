use crate::ipc::Snapshot;
use ksni::blocking::TrayMethods;
use scufpad_core::{BridgeState, ControlCommand, ControlRequest, client_request};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const CADENCE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Presentation {
    summary: &'static str,
    detail: String,
    armed: bool,
    icon: &'static str,
}

impl Presentation {
    fn from_state(state: &BridgeState) -> Self {
        let summary = if !state.connected {
            "Waiting / disconnected"
        } else if !state.armed {
            "Outputs stopped / inactive"
        } else if state.hardware_ready
            && state.status == "Gamepad forwarding; custom mappings active"
        {
            "Forwarding — custom mappings active"
        } else {
            "Forwarding — custom activation not confirmed"
        };
        Self {
            summary,
            detail: state.status.clone(),
            armed: state.armed,
            icon: if state.armed {
                "media-playback-start"
            } else if state.connected {
                "media-playback-pause"
            } else {
                "network-offline"
            },
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    OpenEditor,
    Control(ControlCommand),
}

#[derive(Default)]
struct Priority {
    stop_all: AtomicBool,
    shutdown: AtomicBool,
    generation: AtomicU64,
}

impl Priority {
    fn request(&self, command: ControlCommand) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        match command {
            ControlCommand::Shutdown => self.shutdown.store(true, Ordering::Release),
            ControlCommand::StopAll => self.stop_all.store(true, Ordering::Release),
            _ => unreachable!("Only safety actions bypass the queue"),
        }
    }

    fn take(&self) -> Option<ControlCommand> {
        if self.shutdown.swap(false, Ordering::AcqRel) {
            Some(ControlCommand::Shutdown)
        } else if self.stop_all.swap(false, Ordering::AcqRel) {
            Some(ControlCommand::StopAll)
        } else {
            None
        }
    }

    fn current(&self, generation: u64) -> bool {
        generation == self.generation.load(Ordering::Acquire)
    }

    fn shutdown_failed(&self) {
        // Do not run actions queued while shutdown was in flight, even on failure.
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

struct Tray {
    presentation: Presentation,
    actions: SyncSender<(u64, Action)>,
    priority: Arc<Priority>,
}

impl Tray {
    fn enqueue(&self, action: Action) {
        // Callbacks never wait for IPC, process creation, or a full worker queue.
        match action {
            Action::Control(command @ (ControlCommand::StopAll | ControlCommand::Shutdown)) => {
                self.priority.request(command);
            }
            _ => {
                let generation = self.priority.generation.load(Ordering::Acquire);
                let _ = self.actions.try_send((generation, action));
            }
        }
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "scufpad-bridge".into()
    }

    fn activate(&mut self, _: i32, _: i32) {
        self.enqueue(Action::OpenEditor);
    }

    fn title(&self) -> String {
        format!("Scufpad: {}", self.presentation.summary)
    }

    fn icon_name(&self) -> String {
        self.presentation.icon.into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: self.icon_name(),
            title: self.title(),
            description: self.presentation.detail.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let mut menu = vec![
            StandardItem {
                label: self.presentation.summary.into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: self.presentation.detail.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
        ];
        for (label, action, enabled) in [
            ("Open editor", Action::OpenEditor, true),
            (
                "Resume outputs",
                Action::Control(ControlCommand::Arm),
                !self.presentation.armed,
            ),
            (
                "Stop All outputs",
                Action::Control(ControlCommand::StopAll),
                true,
            ),
            (
                "Stop bridge",
                Action::Control(ControlCommand::Shutdown),
                true,
            ),
        ] {
            menu.push(
                StandardItem {
                    label: label.into(),
                    enabled,
                    activate: Box::new(move |tray: &mut Self| tray.enqueue(action)),
                    ..Default::default()
                }
                .into(),
            );
        }
        menu
    }
}

pub struct Worker(Option<JoinHandle<()>>);

impl Worker {
    pub fn start(snapshot: Arc<RwLock<Snapshot>>, stop: Arc<AtomicBool>, hardware: bool) -> Self {
        if !hardware && std::env::var("SCUFPAD_TEST_TRAY").as_deref() != Ok("1") {
            return Self(None);
        }
        let worker = thread::Builder::new()
            .name("scufpad-tray".into())
            .spawn(move || run(snapshot, stop));
        match worker {
            Ok(worker) => Self(Some(worker)),
            Err(error) => {
                eprintln!("Tray unavailable: {error}");
                Self(None)
            }
        }
    }

    pub fn shutdown(self) {
        if let Some(worker) = self.0
            && worker.join().is_err()
        {
            eprintln!("Tray worker panicked");
        }
    }
}

fn editor_path(bridge: &Path) -> PathBuf {
    if bridge.file_name().is_some_and(|name| name == "Scufpad")
        && let Some(directory) = bridge.parent()
        && directory.file_name().is_some_and(|name| name == "bridge")
        && let Some(application) = directory.parent()
    {
        return application.join("editor/Scufpad.Desktop");
    }
    bridge.with_file_name("scufpad-editor")
}

fn open_editor(child: &mut Option<Child>) -> anyhow::Result<()> {
    if let Some(child) = child.as_mut()
        && child.try_wait()?.is_none()
    {
        return Ok(());
    }
    let bridge = std::env::current_exe()?;
    let executable = editor_path(&bridge);
    *child = Some(
        Command::new(executable)
            .env("SCUFPAD_TRAY_ATTACH_ONLY", "1")
            .env("SCUFPAD_BRIDGE", bridge)
            .stdin(Stdio::null())
            .spawn()?,
    );
    Ok(())
}

fn run(snapshot: Arc<RwLock<Snapshot>>, stop: Arc<AtomicBool>) {
    let initial = match snapshot.read() {
        Ok(snapshot) => Presentation::from_state(&snapshot.state),
        Err(_) => return,
    };
    let (send, receive) = mpsc::sync_channel(4);
    let priority = Arc::new(Priority::default());
    let tray = Tray {
        presentation: initial.clone(),
        actions: send,
        priority: priority.clone(),
    };
    let handle = match tray.assume_sni_available(true).spawn() {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("Tray unavailable (bridge continues): {error}");
            return;
        }
    };
    let mut previous = initial;
    let mut next_update = Instant::now() + CADENCE;
    let mut editor = None;
    'worker: while !stop.load(Ordering::Acquire) && !handle.is_closed() {
        let queued = receive.recv_timeout(next_update.saturating_duration_since(Instant::now()));
        if stop.load(Ordering::Acquire) {
            break;
        }
        // Check again after waking, before any ordinary action can reach IPC.
        while let Some(command) = priority.take() {
            match control(command) {
                Ok(()) if command == ControlCommand::Shutdown => break 'worker,
                Ok(()) => {}
                Err(error) => {
                    if command == ControlCommand::Shutdown {
                        priority.shutdown_failed();
                    }
                    eprintln!("Tray {command:?} failed: {error:#}");
                }
            }
            if stop.load(Ordering::Acquire) {
                break 'worker;
            }
        }
        match queued {
            Ok((generation, action)) if priority.current(generation) => {
                let result = match action {
                    Action::OpenEditor => open_editor(&mut editor),
                    Action::Control(command) => control(command),
                };
                if let Err(error) = result {
                    eprintln!("Tray action failed: {error:#}");
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            _ => {}
        }
        if Instant::now() >= next_update {
            // Project only status fields, never raw buttons/axes or the profile payload.
            let current = snapshot
                .try_read()
                .ok()
                .map(|snapshot| Presentation::from_state(&snapshot.state));
            if let Some(current) = current
                && current != previous
            {
                handle.update(|tray| tray.presentation = current.clone());
                previous = current;
            }
            if let Some(child) = editor.as_mut()
                && matches!(child.try_wait(), Ok(Some(_)))
            {
                editor = None;
            }
            next_update = Instant::now() + CADENCE;
        }
    }
    handle.shutdown().wait();
}

fn control(command: ControlCommand) -> anyhow::Result<()> {
    let response = client_request(&ControlRequest {
        command,
        profile: None,
    })?;
    anyhow::ensure!(
        response.success,
        "{}",
        response.error.as_deref().unwrap_or("Command rejected")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray as _;

    #[test]
    fn editor_path_supports_workspace_and_installed_layouts() {
        assert_eq!(
            editor_path(Path::new("/workspace/target/release/scufpad")),
            Path::new("/workspace/target/release/scufpad-editor")
        );
        assert_eq!(
            editor_path(Path::new("/prefix/share/scufpad/bridge/Scufpad")),
            Path::new("/prefix/share/scufpad/editor/Scufpad.Desktop")
        );
    }

    #[test]
    fn verification_alone_never_claims_custom_activation() {
        let mut state = BridgeState {
            connected: true,
            hardware_ready: true,
            ..Default::default()
        };
        assert_eq!(
            Presentation::from_state(&state).summary,
            "Outputs stopped / inactive"
        );
        state.armed = true;
        assert_eq!(
            Presentation::from_state(&state).summary,
            "Forwarding — custom activation not confirmed"
        );
        state.status = "Gamepad forwarding; custom mappings active".into();
        assert_eq!(
            Presentation::from_state(&state).summary,
            "Forwarding — custom mappings active"
        );
        state.connected = false;
        assert_eq!(
            Presentation::from_state(&state).summary,
            "Waiting / disconnected"
        );
    }

    #[test]
    fn raw_input_does_not_change_presentation_and_callbacks_are_bounded() {
        let mut state = BridgeState::default();
        let presentation = Presentation::from_state(&state);
        state.left_x = 100;
        state.physical_buttons = 17;
        state.recent_presses = 4;
        assert_eq!(presentation, Presentation::from_state(&state));
        let (actions, receive) = mpsc::sync_channel(1);
        let mut tray = Tray {
            presentation,
            actions,
            priority: Arc::new(Priority::default()),
        };
        tray.activate(0, 0);
        tray.activate(0, 0);
        assert!(matches!(receive.try_recv(), Ok((_, Action::OpenEditor))));
        assert!(receive.try_recv().is_err());
        let menu = tray.menu();
        assert_eq!(menu.len(), 7);
        if let ksni::MenuItem::Standard(item) = &menu[4] {
            assert!(item.enabled);
            (item.activate)(&mut tray);
        } else {
            panic!("Resume must be a standard action");
        }
        assert!(matches!(
            receive.try_recv(),
            Ok((_, Action::Control(ControlCommand::Arm)))
        ));
        tray.presentation.armed = true;
        let menu = tray.menu();
        if let ksni::MenuItem::Standard(item) = &menu[4] {
            assert!(!item.enabled);
        } else {
            panic!("Resume must be a standard action");
        }
        for (index, expected) in [(5, ControlCommand::StopAll), (6, ControlCommand::Shutdown)] {
            if let ksni::MenuItem::Standard(item) = &menu[index] {
                assert!(item.enabled);
                (item.activate)(&mut tray);
            } else {
                panic!("Stop must be a standard action");
            }
            assert_eq!(tray.priority.take(), Some(expected));
            assert!(receive.try_recv().is_err());
        }
    }

    #[test]
    fn saturated_queue_cannot_discard_stop_or_shutdown() {
        let (actions, receive) = mpsc::sync_channel(1);
        let tray = Tray {
            presentation: Presentation::from_state(&BridgeState::default()),
            actions,
            priority: Arc::new(Priority::default()),
        };
        tray.enqueue(Action::OpenEditor);
        tray.enqueue(Action::Control(ControlCommand::StopAll));
        tray.enqueue(Action::Control(ControlCommand::Shutdown));
        assert_eq!(tray.priority.take(), Some(ControlCommand::Shutdown));
        assert_eq!(tray.priority.take(), Some(ControlCommand::StopAll));
        assert_eq!(tray.priority.take(), None);
        let (generation, _) = receive.try_recv().unwrap();
        assert!(!tray.priority.current(generation));
    }

    #[test]
    fn stop_invalidates_queued_resume_but_preserves_later_explicit_resume() {
        let (actions, receive) = mpsc::sync_channel(2);
        let tray = Tray {
            presentation: Presentation::from_state(&BridgeState::default()),
            actions,
            priority: Arc::new(Priority::default()),
        };
        tray.enqueue(Action::Control(ControlCommand::Arm));
        tray.enqueue(Action::Control(ControlCommand::StopAll));
        tray.enqueue(Action::Control(ControlCommand::Arm));
        assert_eq!(tray.priority.take(), Some(ControlCommand::StopAll));
        let (old, _) = receive.try_recv().unwrap();
        let (later, _) = receive.try_recv().unwrap();
        assert!(!tray.priority.current(old));
        assert!(tray.priority.current(later));
    }

    #[test]
    fn failed_shutdown_invalidates_actions_queued_during_ipc_without_losing_retry() {
        let priority = Priority::default();
        priority.request(ControlCommand::Shutdown);
        assert_eq!(priority.take(), Some(ControlCommand::Shutdown));
        let during_shutdown = priority.generation.load(Ordering::Acquire);
        priority.request(ControlCommand::Shutdown);
        priority.shutdown_failed();
        assert!(!priority.current(during_shutdown));
        assert_eq!(priority.take(), Some(ControlCommand::Shutdown));
        assert!(priority.current(priority.generation.load(Ordering::Acquire)));
    }
}
