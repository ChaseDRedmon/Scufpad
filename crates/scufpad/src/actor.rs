use crate::{
    discovery,
    gate::HardwareGate,
    hardware::Session,
    ipc::{Pending, Snapshot, failure},
    output::Outputs,
};
use anyhow::{Result, anyhow, bail};
use scufpad_core::{
    BridgeState, ControlCommand, ControlResponse, MappingEngine, OutputState, PhysicalControl,
    PhysicalState, RemapProfile, config_dir, decode_usb_report, read_profile, write_profile,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    thread,
    time::{Duration, Instant},
};

pub fn load_profile(path: &Path) -> RemapProfile {
    if !path.exists() {
        return RemapProfile::default();
    }
    read_profile(path)
        .and_then(|profile| {
            MappingEngine::new(profile.clone())?;
            Ok(profile)
        })
        .unwrap_or_else(|error| {
            eprintln!(
                "Stored profile rejected; using Default without overwriting original: {error}"
            );
            RemapProfile::default()
        })
}

pub struct Actor {
    engine: MappingEngine,
    profile: Arc<RemapProfile>,
    profile_path: PathBuf,
    gate: HardwareGate,
    physical: PhysicalState,
    recent_until: [u64; 31],
    connected: bool,
    want_output: bool,
    remapping: bool,
    outputs: Option<Outputs>,
    status: String,
    hardware: bool,
}

impl Actor {
    pub fn new(hardware: bool) -> Result<Self> {
        Self::with_profile_path(hardware, config_dir().join("active-profile.json"))
    }

    fn with_profile_path(hardware: bool, profile_path: PathBuf) -> Result<Self> {
        let profile = load_profile(&profile_path);
        let engine = MappingEngine::new(profile)?;
        let profile = Arc::new(engine.profile().clone());
        Ok(Self {
            engine,
            profile,
            profile_path,
            gate: HardwareGate::default(),
            physical: PhysicalState::default(),
            recent_until: [0; 31],
            connected: false,
            want_output: hardware,
            remapping: false,
            outputs: None,
            status: if hardware {
                "Waiting for controller"
            } else {
                "No-hardware mode: discovery, input and uinput disabled"
            }
            .into(),
            hardware,
        })
    }

    fn stop_outputs(&mut self) {
        self.outputs = None;
        self.remapping = false;
        self.engine.cancel_all();
    }

    fn connected(&mut self) {
        self.stop_outputs();
        self.connected = true;
        self.physical = PhysicalState::default();
        self.gate.reset();
        self.recent_until.fill(0);
        self.status = "Controller connected; gamepad forwarding starts automatically. Verify six paddles/SAX for custom mappings.".into();
    }

    fn disconnected(&mut self, reason: impl ToString) {
        self.stop_outputs();
        self.connected = false;
        self.gate.reset();
        self.physical = PhysicalState::default();
        self.recent_until.fill(0);
        self.status = reason.to_string();
        // Stop All survives reconnection; custom-input verification does not.
    }

    fn frame(&mut self, mut physical: PhysicalState, now: u64) -> Result<()> {
        self.gate.observe(physical);
        for (control, value) in [
            (PhysicalControl::LeftTrigger, physical.left_trigger),
            (PhysicalControl::RightTrigger, physical.right_trigger),
        ] {
            let pressed = if self.physical.buttons & control.bit() != 0 {
                value > self.profile.trigger_release_threshold
            } else {
                value >= self.profile.trigger_press_threshold
            };
            if pressed {
                physical.buttons |= control.bit();
            }
        }
        let rising = physical.buttons & !self.physical.buttons;
        for (bit, until) in self.recent_until.iter_mut().enumerate() {
            if rising & (1 << bit) != 0 {
                *until = now.saturating_add(100);
            }
        }
        self.physical = physical;
        self.tick(now)
    }

    fn tick(&mut self, now: u64) -> Result<()> {
        if !self.hardware || !self.connected || !self.want_output {
            return Ok(());
        }
        let was_remapping = self.remapping;
        let create = self.outputs.is_none();
        self.update_output(now)?;
        if create {
            self.outputs = Some(Outputs::create()?);
        }
        if let Some(outputs) = &mut self.outputs {
            outputs.set_remapping(self.remapping)?;
            outputs.emit(&self.engine.output)?;
        }
        if create || was_remapping != self.remapping {
            self.status = if self.remapping {
                "Gamepad forwarding; custom mappings active"
            } else {
                "Gamepad forwarding; verify all six paddles/SAX for custom mappings"
            }
            .into();
        }
        Ok(())
    }

    fn update_output(&mut self, now: u64) -> Result<()> {
        let remapping = self.gate.ready();
        if remapping && !self.remapping {
            self.engine.tick(self.physical, now)?;
            self.engine.cancel_all();
        } else if !remapping && self.remapping {
            self.engine.cancel_all();
        }
        if remapping {
            self.engine.tick(self.physical, now)?;
        } else {
            self.engine.output = OutputState::gamepad_passthrough(self.physical);
        }
        self.remapping = remapping;
        Ok(())
    }

    pub fn snapshot(&self, now: u64) -> Snapshot {
        let output = &self.engine.output;
        let key_word = |start: usize| {
            (0..64).fold(0u64, |bits, i| {
                bits | (u64::from(output.keyboard[start + i]) << i)
            })
        };
        Snapshot {
            profile: self.profile.clone(),
            state: BridgeState {
                connected: self.connected,
                armed: self.outputs.is_some(),
                hardware_ready: self.gate.ready(),
                verified_extra_controls: self.gate.verified,
                physical_buttons: self.physical.buttons,
                recent_presses: self
                    .recent_until
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (i, &until)| bits | (u64::from(until > now) << i)),
                gamepad_buttons: output.gamepad_buttons,
                active_layers: self.engine.active_layers,
                keyboard0: key_word(0),
                keyboard1: key_word(64),
                keyboard2: key_word(128),
                keyboard3: key_word(192),
                mouse_buttons: output
                    .mouse
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (i, &pressed)| bits | (u8::from(pressed) << i)),
                left_x: self.physical.left_x,
                left_y: self.physical.left_y,
                right_x: self.physical.right_x,
                right_y: self.physical.right_y,
                left_trigger: self.physical.left_trigger,
                right_trigger: self.physical.right_trigger,
                profile_name: self.profile.name.clone(),
                status: self.status.clone(),
            },
        }
    }

    fn publish(&self, snapshot: &RwLock<Snapshot>, now: u64) {
        if let Ok(mut current) = snapshot.try_write() {
            *current = self.snapshot(now);
        }
    }

    fn command(
        &mut self,
        command: ControlCommand,
        profile: Option<RemapProfile>,
        now: u64,
    ) -> Result<ControlResponse> {
        match command {
            ControlCommand::Arm => {
                self.want_output = true;
                self.status = if self.hardware {
                    "Outputs requested; custom mappings wait for six-input verification"
                } else {
                    "Arm requested; hardware disabled by no-hardware mode"
                }
                .into();
                if let Err(error) = self.tick(now) {
                    self.disconnected(&error);
                    return Err(error);
                }
            }
            ControlCommand::ApplyProfile => {
                let replacement = MappingEngine::new(
                    profile.ok_or_else(|| anyhow!("ApplyProfile requires profile"))?,
                )?;
                self.engine = replacement;
                self.remapping = false;
                self.profile = Arc::new(self.engine.profile().clone());
                for (control, value) in [
                    (PhysicalControl::LeftTrigger, self.physical.left_trigger),
                    (PhysicalControl::RightTrigger, self.physical.right_trigger),
                ] {
                    self.physical.buttons &= !control.bit();
                    if value >= self.profile.trigger_press_threshold {
                        self.physical.buttons |= control.bit();
                    }
                }
                if self.want_output && self.connected {
                    self.engine.output = OutputState::gamepad_passthrough(self.physical);
                }
                if let Some(outputs) = &mut self.outputs {
                    outputs.set_remapping(false)?;
                    outputs.emit(&self.engine.output)?;
                }
                self.status = if !self.want_output {
                    "Profile active; outputs remain stopped; resume explicitly"
                } else if !self.connected {
                    "Profile active; waiting for controller"
                } else if self.gate.ready() {
                    "Profile active; verified custom mappings resume with outputs"
                } else {
                    "Profile active; custom mappings require six-input verification"
                }
                .into();
                if let Err(error) = write_profile(&self.profile_path, &self.profile) {
                    self.status = format!("Profile active but not saved: {error}");
                    return Ok(failure(&self.status));
                }
            }
            ControlCommand::StopAll | ControlCommand::Shutdown => {
                self.want_output = false;
                self.stop_outputs();
                self.status = "Stopped: arm to resume".into();
            }
            _ => bail!("Unsupported queued command"),
        }
        Ok(ControlResponse {
            success: true,
            ..Default::default()
        })
    }

    fn drain(
        &mut self,
        commands: &Receiver<Pending>,
        snapshot: &RwLock<Snapshot>,
        now: u64,
    ) -> bool {
        for _ in 0..32 {
            let Ok(pending) = commands.try_recv() else {
                break;
            };
            if !pending.start() {
                let _ = pending
                    .reply
                    .send(failure("Command cancelled before execution"));
                continue;
            }
            let response = match self.command(pending.request.command, pending.request.profile, now)
            {
                Ok(response) => response,
                Err(error) => {
                    self.want_output = false;
                    self.stop_outputs();
                    self.status = error.to_string();
                    failure(error)
                }
            };
            let shutdown = pending.request.command == ControlCommand::Shutdown && response.success;
            self.publish(snapshot, now);
            let _ = pending.reply.send(response);
            if shutdown {
                return true;
            }
        }
        false
    }

    pub fn run(
        &mut self,
        commands: Receiver<Pending>,
        snapshot: Arc<RwLock<Snapshot>>,
        stop: Arc<AtomicBool>,
        reconnect: bool,
    ) -> Result<()> {
        let clock = Instant::now();
        let now_ms = || clock.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let (mut next_discovery, mut last_valid) = (0, 0);
        let mut session: Option<Session> = None;
        let mut shutdown_requested = false;
        while !stop.load(Ordering::Acquire) {
            let now = now_ms();
            if session.is_some() && now.saturating_sub(last_valid) > 500 {
                self.disconnected("Controller input stale");
                session = None;
                if !reconnect {
                    break;
                }
            }
            if !shutdown_requested {
                shutdown_requested = self.drain(&commands, &snapshot, now);
            }
            if session.is_some() && !self.connected {
                session = None;
                if !reconnect {
                    break;
                }
            }
            if session.is_none() {
                if self.hardware && now >= next_discovery {
                    next_discovery = now.saturating_add(1000);
                    let opened = discovery::discover(Path::new("/sys/class"), Path::new("/dev"))
                        .and_then(|devices| {
                            devices.map(|devices| Session::open(&devices)).transpose()
                        });
                    match opened {
                        Ok(Some(opened)) => {
                            session = Some(opened);
                            self.connected();
                            last_valid = now;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.status = format!("Read-only: discovery/open failed: {error}")
                        }
                    }
                }
                self.publish(&snapshot, now);
                if session.is_none() {
                    thread::sleep(Duration::from_millis(4));
                    continue;
                }
            }
            let result = (|| -> Result<()> {
                let session = session.as_mut().expect("connected session");
                let timeout = if self.outputs.is_some() {
                    self.engine
                        .next_due_ms
                        .map_or(4, |due| due.saturating_sub(now).min(4) as i32)
                } else {
                    4
                };
                if session.poll(timeout)? {
                    let mut bytes = [0u8; 64];
                    for _ in 0..64 {
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        let Some(length) = session.read(&mut bytes)? else {
                            break;
                        };
                        if bytes[0] == 0x58 {
                            if length != 64 {
                                bail!("Malformed report 58");
                            }
                            continue;
                        }
                        let physical = decode_usb_report(&bytes[..length])
                            .ok_or_else(|| anyhow!("Malformed controller report"))?;
                        last_valid = now_ms();
                        self.frame(physical, last_valid)?;
                    }
                }
                let now = now_ms();
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                if now.saturating_sub(last_valid) > 500 {
                    bail!("Controller input stale");
                }
                self.tick(now)?;
                Ok(())
            })();
            if let Err(error) = result {
                self.disconnected(error);
                session = None;
                self.publish(&snapshot, now_ms());
                if !reconnect {
                    break;
                }
            }
            self.publish(&snapshot, now_ms());
        }
        self.stop_outputs();
        drop(session);
        self.connected = false;
        self.publish(&snapshot, now_ms());
        for pending in commands.try_iter() {
            let _ = pending.reply.send(failure("Daemon stopped"));
        }
        Ok(())
    }
}

impl Drop for Actor {
    fn drop(&mut self) {
        self.stop_outputs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scufpad_core::{
        Binding, BindingKind, GamepadControl, LayerDefinition, MacroDefinition, MacroStep,
        MacroStepKind,
    };

    #[test]
    #[ignore = "creates a temporary virtual controller; requires SCUFPAD_ROUNDTRIP_ACCEPT=1 with games closed"]
    fn roundtrip_uinput_driver() -> Result<()> {
        use std::io::{BufRead, Write};

        anyhow::ensure!(
            std::env::var("SCUFPAD_ROUNDTRIP_ACCEPT").as_deref() == Ok("1"),
            "Roundtrip driver requires SCUFPAD_ROUNDTRIP_ACCEPT=1 and games closed"
        );
        anyhow::ensure!(
            std::env::var("SCUFPAD_NO_HARDWARE").as_deref() != Ok("1"),
            "Roundtrip driver cannot run with SCUFPAD_NO_HARDWARE=1"
        );
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json"))?;
        actor.connected();
        actor.hardware = true;
        actor.want_output = true;
        actor.outputs = Some(Outputs::create()?);
        let sysname = actor
            .outputs
            .as_ref()
            .ok_or_else(|| anyhow!("Roundtrip output missing"))?
            .test_sysname()?;
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        writeln!(output, "SCUFPAD_ROUNDTRIP READY {sysname}")?;
        output.flush()?;
        let mut now = 0u64;
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                break;
            }
            let command = line.strip_suffix('\n').unwrap_or(&line);
            anyhow::ensure!(
                matches!(command, "PRESS" | "RELEASE" | "QUIT"),
                "Invalid roundtrip command: expected PRESS, RELEASE or QUIT"
            );
            let mut report = [0u8; 16];
            report[0] = 6;
            report[11] = 0x80;
            if command == "PRESS" {
                report[12] = PhysicalControl::A.bit() as u8;
            }
            now = now
                .checked_add(1)
                .ok_or_else(|| anyhow!("Roundtrip timestamp overflow"))?;
            actor.frame(
                decode_usb_report(&report)
                    .ok_or_else(|| anyhow!("Invalid roundtrip USB report"))?,
                now,
            )?;
            if command == "QUIT" {
                actor.stop_outputs();
                writeln!(output, "SCUFPAD_ROUNDTRIP DONE")?;
                output.flush()?;
                break;
            }
            writeln!(output, "SCUFPAD_ROUNDTRIP ACK {command}")?;
            output.flush()?;
        }
        Ok(())
    }

    fn routing_actor(fixture: &crate::test_support::Fixture) -> Actor {
        let mut profile = RemapProfile::default();
        profile.layers[0].bindings = [
            (
                PhysicalControl::A,
                Binding {
                    kind: BindingKind::Keys,
                    keys: vec![29],
                    ..Default::default()
                },
            ),
            (
                PhysicalControl::RearOuterRight,
                Binding {
                    kind: BindingKind::Macro,
                    reference: "HeldKey".into(),
                    ..Default::default()
                },
            ),
            (
                PhysicalControl::SaxLeft,
                Binding {
                    kind: BindingKind::Mouse,
                    code: 272,
                    ..Default::default()
                },
            ),
            (
                PhysicalControl::G1,
                Binding {
                    kind: BindingKind::MomentaryLayer,
                    reference: "Shift".into(),
                    ..Default::default()
                },
            ),
        ]
        .into();
        profile.layers.push(LayerDefinition {
            id: "Shift".into(),
            ..Default::default()
        });
        profile.macros.push(MacroDefinition {
            id: "HeldKey".into(),
            steps: vec![
                MacroStep {
                    kind: MacroStepKind::PressKey,
                    code: 46,
                    delay_ms: 0,
                },
                MacroStep {
                    kind: MacroStepKind::Delay,
                    code: 0,
                    delay_ms: 50,
                },
                MacroStep {
                    kind: MacroStepKind::ReleaseKey,
                    code: 46,
                    delay_ms: 0,
                },
            ],
            ..Default::default()
        });
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        actor.engine = MappingEngine::new(profile.clone()).unwrap();
        actor.profile = Arc::new(profile);
        actor.want_output = true;
        actor.connected();
        actor
    }

    fn route(actor: &mut Actor, buttons: u64, now: u64) {
        assert!(!actor.hardware);
        actor
            .frame(
                PhysicalState {
                    buttons,
                    ..Default::default()
                },
                now,
            )
            .unwrap();
        actor.update_output(now).unwrap();
        assert!(actor.outputs.is_none());
    }

    fn verify_custom_inputs(actor: &mut Actor, now: &mut u64) {
        route(actor, 0, *now);
        for bit in 19..25 {
            *now += 1;
            route(actor, 1 << bit, *now);
            assert!(!actor.gate.ready());
            assert!(!actor.engine.output.keyboard.iter().any(|&pressed| pressed));
            *now += 1;
            route(actor, 0, *now);
        }
        assert!(actor.gate.ready());
        assert!(actor.remapping);
    }

    #[test]
    fn startup_requests_forwarding_only_when_hardware_is_allowed() {
        let fixture = crate::test_support::Fixture::new();
        let mut enabled =
            Actor::with_profile_path(true, fixture.root.join("missing.json")).unwrap();
        assert!(enabled.want_output);
        assert!(!enabled.remapping && !enabled.gate.ready());
        assert!(enabled.outputs.is_none());
        enabled.command(ControlCommand::StopAll, None, 0).unwrap();
        enabled.connected();
        assert!(!enabled.want_output);
        let disabled = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        assert!(!disabled.want_output);
        assert!(disabled.outputs.is_none());
    }

    #[test]
    fn normal_inputs_forward_before_verification_and_custom_profile_activates_afterward() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = routing_actor(&fixture);
        route(
            &mut actor,
            PhysicalControl::A.bit() | PhysicalControl::RearOuterRight.bit(),
            0,
        );
        assert!(!actor.gate.ready() && !actor.remapping);
        assert_eq!(actor.engine.output.gamepad_buttons, GamepadControl::A.bit());
        assert!(!actor.engine.output.keyboard.iter().any(|&pressed| pressed));
        assert_eq!(actor.engine.next_due_ms, None);
        let mut now = 1;
        verify_custom_inputs(&mut actor, &mut now);
        now += 1;
        route(&mut actor, PhysicalControl::A.bit(), now);
        assert!(actor.engine.output.keyboard[29]);
        actor.command(ControlCommand::StopAll, None, now).unwrap();
        actor.command(ControlCommand::Arm, None, now).unwrap();
        actor.update_output(now).unwrap();
        assert!(
            !actor.engine.output.keyboard[29],
            "resume must not replay a held binding"
        );
        now += 1;
        route(&mut actor, 0, now);
        now += 1;
        route(&mut actor, PhysicalControl::A.bit(), now);
        assert!(actor.engine.output.keyboard[29]);
    }

    #[test]
    fn verification_loss_cancels_custom_owners_without_stopping_normal_forwarding() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = routing_actor(&fixture);
        let mut now = 0;
        verify_custom_inputs(&mut actor, &mut now);
        route(&mut actor, PhysicalControl::RearOuterRight.bit(), now + 1);
        assert!(actor.engine.output.keyboard[46]);
        route(&mut actor, 0, now + 2);
        route(&mut actor, PhysicalControl::SaxLeft.bit(), now + 3);
        assert!(actor.engine.output.mouse[0]);
        assert!(actor.engine.output.keyboard[46]);
        route(&mut actor, 0, now + 4);
        route(&mut actor, PhysicalControl::G1.bit(), now + 5);
        assert_eq!(actor.engine.active_layers, 3);
        assert!(actor.engine.next_due_ms.is_some());
        route(
            &mut actor,
            PhysicalControl::RearOuterRight.bit() | PhysicalControl::A.bit(),
            now + 6,
        );
        assert!(!actor.remapping && !actor.gate.ready());
        assert!(actor.want_output);
        assert_eq!(actor.engine.output.gamepad_buttons, GamepadControl::A.bit());
        assert!(!actor.engine.output.keyboard.iter().any(|&pressed| pressed));
        assert!(!actor.engine.output.mouse.iter().any(|&pressed| pressed));
        assert_eq!(actor.engine.active_layers, 1);
        assert_eq!(actor.engine.next_due_ms, None);
        route(&mut actor, PhysicalControl::HardwareProfile.bit(), now + 7);
        assert_eq!(actor.gate.verified, 0);
        assert!(actor.want_output);
        route(&mut actor, PhysicalControl::A.bit(), now + 8);
        assert_eq!(actor.engine.output.gamepad_buttons, GamepadControl::A.bit());
        route(&mut actor, 0, now + 1000);
        assert!(!actor.engine.output.keyboard.iter().any(|&pressed| pressed));
    }

    #[test]
    fn profile_apply_preserves_running_and_stopped_intent_without_replaying_held_keys() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = routing_actor(&fixture);
        actor.profile_path = fixture.root.join("active-profile.json");
        fixture.track(actor.profile_path.clone());
        let mut now = 0;
        verify_custom_inputs(&mut actor, &mut now);
        route(&mut actor, PhysicalControl::A.bit(), now + 1);
        assert!(actor.engine.output.keyboard[29]);
        let mut replacement = (*actor.profile).clone();
        replacement.name = "Replacement".into();
        replacement.layers[0].bindings.insert(
            PhysicalControl::A,
            Binding {
                kind: BindingKind::Keys,
                keys: vec![30],
                ..Default::default()
            },
        );
        assert!(
            actor
                .command(
                    ControlCommand::ApplyProfile,
                    Some(replacement.clone()),
                    now + 2
                )
                .unwrap()
                .success
        );
        assert!(actor.want_output);
        assert!(!actor.remapping);
        assert_eq!(
            actor.status,
            "Profile active; verified custom mappings resume with outputs"
        );
        assert_eq!(actor.engine.output.gamepad_buttons, GamepadControl::A.bit());
        assert!(!actor.engine.output.keyboard.iter().any(|&pressed| pressed));
        actor.update_output(now + 2).unwrap();
        assert!(!actor.engine.output.keyboard[30]);
        route(&mut actor, 0, now + 3);
        route(&mut actor, PhysicalControl::A.bit(), now + 4);
        assert!(actor.engine.output.keyboard[30]);
        assert_eq!(
            read_profile(&actor.profile_path).unwrap().name,
            "Replacement"
        );
        actor
            .command(ControlCommand::StopAll, None, now + 5)
            .unwrap();
        assert!(
            actor
                .command(ControlCommand::ApplyProfile, Some(replacement), now + 6)
                .unwrap()
                .success
        );
        assert!(!actor.want_output);
        assert_eq!(
            actor.status,
            "Profile active; outputs remain stopped; resume explicitly"
        );
        assert_eq!(actor.engine.output, OutputState::default());
        actor.disconnected("fixture reconnect");
        actor.connected();
        actor
            .frame(
                PhysicalState {
                    buttons: PhysicalControl::A.bit(),
                    ..Default::default()
                },
                now + 7,
            )
            .unwrap();
        assert!(!actor.want_output);
        assert_eq!(actor.engine.output, OutputState::default());
        actor.command(ControlCommand::Arm, None, now + 8).unwrap();
        assert!(actor.want_output);
        actor.update_output(now + 8).unwrap();
        assert_eq!(actor.engine.output.gamepad_buttons, GamepadControl::A.bit());
    }

    #[test]
    fn profile_status_distinguishes_disconnected_and_unverified_controller() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = routing_actor(&fixture);
        actor.profile_path = fixture.root.join("active-profile.json");
        fixture.track(actor.profile_path.clone());
        let profile = (*actor.profile).clone();
        actor.disconnected("fixture disconnect");
        actor
            .command(ControlCommand::ApplyProfile, Some(profile.clone()), 0)
            .unwrap();
        assert_eq!(actor.status, "Profile active; waiting for controller");
        actor.connected();
        actor
            .command(ControlCommand::ApplyProfile, Some(profile), 1)
            .unwrap();
        assert_eq!(
            actor.status,
            "Profile active; custom mappings require six-input verification"
        );
    }

    #[test]
    fn accepted_shutdown_leaves_later_arm_commands_unstarted() {
        use scufpad_core::ControlRequest;
        use std::sync::{atomic::AtomicU8, mpsc};
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        actor.want_output = true;
        let snapshot = RwLock::new(actor.snapshot(0));
        let (send, receive) = mpsc::sync_channel(2);
        let mut replies = Vec::new();
        let mut execution = Vec::new();
        for command in [ControlCommand::Shutdown, ControlCommand::Arm] {
            let (reply, response) = mpsc::sync_channel(1);
            let started = Arc::new(AtomicU8::new(0));
            send.try_send(Pending {
                request: ControlRequest {
                    command,
                    profile: None,
                },
                deadline: Instant::now() + Duration::from_secs(3),
                execution: started.clone(),
                reply,
            })
            .unwrap_or_else(|_| panic!("empty queue rejected command"));
            replies.push(response);
            execution.push(started);
        }
        assert!(actor.drain(&receive, &snapshot, 0));
        assert!(!actor.want_output);
        assert!(replies[0].try_recv().unwrap().success);
        assert_eq!(execution[0].load(Ordering::Acquire), 1);
        assert_eq!(execution[1].load(Ordering::Acquire), 0);
        assert!(replies[1].try_recv().is_err());
        assert_eq!(
            receive.try_recv().unwrap().request.command,
            ControlCommand::Arm
        );
    }
    #[test]
    fn no_hardware_arm_is_intent_only_stop_all_keeps_daemon_state() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        actor.command(ControlCommand::Arm, None, 0).unwrap();
        assert!(actor.want_output);
        assert!(!actor.snapshot(0).state.armed);
        actor.command(ControlCommand::StopAll, None, 1).unwrap();
        assert!(!actor.want_output);
        assert!(!actor.snapshot(1).state.connected);
    }

    #[test]
    fn reconnect_and_profile_press_keep_forwarding_intent_but_clear_custom_verification() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        actor.want_output = true;
        actor.connected();
        for bit in 19..25 {
            actor
                .frame(
                    PhysicalState {
                        buttons: 1 << bit,
                        ..Default::default()
                    },
                    0,
                )
                .unwrap();
            actor.frame(PhysicalState::default(), 1).unwrap();
        }
        assert!(actor.gate.ready());
        actor.disconnected("test disconnect");
        assert!(actor.want_output);
        actor.connected();
        assert!(!actor.gate.ready());
        actor
            .frame(
                PhysicalState {
                    buttons: PhysicalControl::HardwareProfile.bit(),
                    ..Default::default()
                },
                2,
            )
            .unwrap();
        assert!(actor.want_output);
        assert_eq!(actor.gate.verified, 0);
    }

    #[test]
    fn invalid_persisted_profile_falls_back_without_overwrite() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.file("profile.json", b"not json");
        assert_eq!(load_profile(&path).name, "Default");
        assert_eq!(std::fs::read(path).unwrap(), b"not json");
    }

    #[test]
    fn unsaved_active_profile_reports_failure_without_losing_old_file() {
        let fixture = crate::test_support::Fixture::new();
        let old = fixture.file("not-directory", b"old");
        let mut actor = Actor::with_profile_path(false, old.join("active-profile.json")).unwrap();
        let profile = RemapProfile {
            name: "Unsaved".into(),
            ..Default::default()
        };
        let response = actor
            .command(ControlCommand::ApplyProfile, Some(profile), 0)
            .unwrap();
        assert!(!response.success);
        assert!(response.error.unwrap().contains("active but not saved"));
        assert_eq!(actor.profile.name, "Unsaved");
        assert_eq!(std::fs::read(old).unwrap(), b"old");
    }

    #[test]
    fn trigger_hysteresis_and_recent_press_telemetry() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        for (time, value, pressed) in [(0, 512, true), (1, 490, true), (2, 480, false)] {
            actor
                .frame(
                    PhysicalState {
                        left_trigger: value,
                        ..Default::default()
                    },
                    time,
                )
                .unwrap();
            assert_eq!(
                actor.physical.buttons & PhysicalControl::LeftTrigger.bit() != 0,
                pressed
            );
        }
        assert_ne!(actor.snapshot(99).state.recent_presses, 0);
        assert_eq!(actor.snapshot(100).state.recent_presses, 0);
    }

    #[test]
    fn zero_release_threshold_clears_trigger_and_detects_next_press() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        Arc::make_mut(&mut actor.profile).trigger_release_threshold = 0;
        let trigger = PhysicalControl::RightTrigger.bit();
        for (time, value, pressed) in [(0, 512, true), (101, 0, false), (102, 512, true)] {
            actor
                .frame(
                    PhysicalState {
                        right_trigger: value,
                        ..Default::default()
                    },
                    time,
                )
                .unwrap();
            assert_eq!(
                actor.snapshot(time).state.physical_buttons & trigger != 0,
                pressed
            );
        }
        assert_ne!(actor.snapshot(102).state.recent_presses & trigger, 0);
        assert_eq!(actor.snapshot(202).state.recent_presses & trigger, 0);
    }

    #[test]
    fn applying_profile_restarts_trigger_hysteresis_like_the_new_engine() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("active-profile.json");
        fixture.track(path.clone());
        let mut actor = Actor::with_profile_path(false, path).unwrap();
        let trigger = PhysicalControl::LeftTrigger.bit();
        let frame = |value| PhysicalState {
            left_trigger: value,
            ..Default::default()
        };
        actor.frame(frame(512), 0).unwrap();
        actor.frame(frame(490), 101).unwrap();
        assert_ne!(actor.physical.buttons & trigger, 0);
        actor
            .command(
                ControlCommand::ApplyProfile,
                Some((*actor.profile).clone()),
                102,
            )
            .unwrap();
        assert_eq!(actor.physical.buttons & trigger, 0);
        actor.frame(frame(490), 103).unwrap();
        assert_eq!(actor.physical.buttons & trigger, 0);
        actor.frame(frame(512), 104).unwrap();
        assert_ne!(actor.snapshot(104).state.recent_presses & trigger, 0);
        let both = PhysicalControl::LeftTrigger.bit() | PhysicalControl::RightTrigger.bit();
        let held = PhysicalState {
            left_trigger: 512,
            right_trigger: 512,
            ..Default::default()
        };
        actor.frame(held, 205).unwrap();
        actor
            .command(
                ControlCommand::ApplyProfile,
                Some((*actor.profile).clone()),
                305,
            )
            .unwrap();
        assert_eq!(actor.snapshot(305).state.physical_buttons & both, both);
        assert_eq!(actor.snapshot(305).state.recent_presses & both, 0);
        actor.frame(held, 306).unwrap();
        assert_eq!(actor.snapshot(306).state.recent_presses & both, 0);
    }

    #[test]
    fn native_report_fixtures_require_release_before_readiness() {
        let fixture = crate::test_support::Fixture::new();
        let mut actor = Actor::with_profile_path(false, fixture.root.join("missing.json")).unwrap();
        let mut report = [0u8; 16];
        report[0] = 6;
        report[11] = 0x80;
        for bit in 19..25 {
            report[12..16].copy_from_slice(&(1u32 << bit).to_le_bytes());
            actor.frame(decode_usb_report(&report).unwrap(), 0).unwrap();
            assert!(!actor.gate.ready());
            report[12..16].fill(0);
            actor.frame(decode_usb_report(&report).unwrap(), 1).unwrap();
        }
        assert!(actor.gate.ready());
        report[14] = 1;
        actor.want_output = true;
        actor.frame(decode_usb_report(&report).unwrap(), 2).unwrap();
        assert!(actor.want_output);
        assert!(!actor.gate.ready());
    }

    #[test]
    fn isolated_no_hardware_control_integration_acknowledges_shutdown() {
        use scufpad_core::{ControlRequest, read_message, write_message};
        use std::{io::Write, os::unix::net::UnixStream, sync::mpsc};
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.root.join("control");
        let profile_path = fixture.root.join("active-profile.json");
        fixture.track(profile_path.clone());
        fixture.track(fixture.root.join("control.lock"));
        let endpoint = crate::ipc::Endpoint::bind(path.clone()).unwrap();
        let mut actor = Actor::with_profile_path(false, profile_path.clone()).unwrap();
        let snapshot = Arc::new(RwLock::new(actor.snapshot(0)));
        let stop = Arc::new(AtomicBool::new(false));
        let (send, receive) = mpsc::sync_channel(32);
        let server_stop = stop.clone();
        let server_snapshot = snapshot.clone();
        let server =
            thread::spawn(move || crate::ipc::serve(endpoint, send, server_snapshot, server_stop));
        let actor_stop = stop.clone();
        let owner = thread::spawn(move || actor.run(receive, snapshot, actor_stop, false));
        let request = |command, profile| {
            let mut stream = UnixStream::connect(&path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            write_message(&mut stream, &ControlRequest { command, profile }).unwrap();
            read_message::<ControlResponse>(&mut stream).unwrap()
        };
        let result = std::panic::catch_unwind(|| {
            let state = request(ControlCommand::GetState, None).state.unwrap();
            assert!(!state.armed && !state.connected && !state.hardware_ready);
            assert!(request(ControlCommand::Arm, None).success);
            assert!(!request(ControlCommand::GetState, None).state.unwrap().armed);
            let profile = RemapProfile {
                name: "Fixture".into(),
                ..Default::default()
            };
            assert!(request(ControlCommand::ApplyProfile, Some(profile)).success);
            assert_eq!(read_profile(&profile_path).unwrap().name, "Fixture");
            assert_eq!(
                request(ControlCommand::GetProfile, None)
                    .profile
                    .unwrap()
                    .name,
                "Fixture"
            );
            let mut stalled = UnixStream::connect(&path).unwrap();
            stalled.write_all(&[1]).unwrap();
            let started = Instant::now();
            assert!(request(ControlCommand::StopAll, None).success);
            assert!(started.elapsed() < Duration::from_secs(1));
            assert!(request(ControlCommand::GetState, None).success);
            let started = Instant::now();
            assert!(request(ControlCommand::Shutdown, None).success);
            assert!(started.elapsed() < Duration::from_secs(1));
            drop(stalled);
        });
        stop.store(true, Ordering::Release);
        owner.join().unwrap().unwrap();
        server.join().unwrap().unwrap();
        assert!(!path.exists());
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }
}
