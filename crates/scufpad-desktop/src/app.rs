use crate::{
    edits::*,
    worker::{Action, Event, Status, Worker},
};
use anyhow::{Result, ensure};
use eframe::egui::{self, Color32, RichText};
use scufpad_core::*;
use std::path::PathBuf;

enum Confirm {
    New,
    Load(PathBuf),
    Import,
    Reload,
    Close,
    Stop,
}

pub struct Studio {
    worker: Worker,
    profile: RemapProfile,
    bridge_profile: String,
    initial_profile: Option<RemapProfile>,
    state: Option<BridgeState>,
    starting: bool,
    message: String,
    error: bool,
    selected: PhysicalControl,
    layer: usize,
    rear: bool,
    binding: BindingDraft,
    baseline: BindingDraft,
    profile_name: String,
    layer_name: String,
    local_path: Option<PathBuf>,
    library: Vec<(PathBuf, String)>,
    library_requested: bool,
    library_selection: Option<usize>,
    macro_selection: Option<usize>,
    macro_editor: Option<MacroEditor>,
    dirty: bool,
    pending_apply: bool,
    chosen: bool,
    revision: u64,
    busy: bool,
    confirm: Option<Confirm>,
    allow_close: bool,
    restore_help: bool,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let profile = RemapProfile::default();
        let binding = BindingDraft::load(&profile, 0, PhysicalControl::A);
        let worker = Worker::spawn(cc.egui_ctx.clone());
        Self {
            profile_name: profile.name.clone(),
            layer_name: profile.layers[0].id.clone(),
            worker,
            profile,
            bridge_profile: "not loaded".into(),
            initial_profile: None,
            state: None,
            starting: true,
            message: "Checking native bridge. Gamepad forwarding starts automatically; custom mappings require six verified inputs. Apply edits explicitly."
                .into(),
            error: false,
            selected: PhysicalControl::A,
            layer: 0,
            rear: false,
            baseline: binding.clone(),
            binding,
            local_path: None,
            library: vec![],
            library_requested: false,
            library_selection: None,
            macro_selection: None,
            macro_editor: None,
            dirty: false,
            pending_apply: true,
            chosen: false,
            revision: 0,
            busy: false,
            confirm: None,
            allow_close: false,
            restore_help: false,
        }
    }

    fn fields_dirty(&self) -> bool {
        self.binding != self.baseline
            || self.profile_name != self.profile.name
            || self.layer_name != self.profile.layers[self.layer].id
    }
    fn unsaved(&self) -> bool {
        self.dirty || self.fields_dirty() || self.macro_editor.is_some()
    }
    fn clean(&self) -> Result<()> {
        require_clean(self.fields_dirty())
    }
    fn report(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.message = error.to_string();
            self.error = true;
        }
    }
    fn say(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.error = false;
    }
    fn send(&mut self, action: Action) {
        let workspace_action = matches!(
            action,
            Action::Load { .. }
                | Action::Import { .. }
                | Action::Reload { .. }
                | Action::Save { .. }
                | Action::Export(_)
        );
        match self.worker.send(action) {
            Ok(()) => {
                self.busy = true;
                self.chosen |= workspace_action;
                self.say("Working… The bridge remains independent.");
            }
            Err(error) => {
                self.message = format!("Editor worker unavailable or busy: {error}");
                self.error = true;
            }
        }
    }
    fn refresh_fields(&mut self) {
        self.layer = self.layer.min(self.profile.layers.len() - 1);
        self.profile_name = self.profile.name.clone();
        self.layer_name = self.profile.layers[self.layer].id.clone();
        self.binding = BindingDraft::load(&self.profile, self.layer, self.selected);
        self.baseline = self.binding.clone();
    }
    fn commit(&mut self, candidate: Result<RemapProfile>) {
        match candidate {
            Ok(profile) => {
                self.profile = profile;
                self.dirty = true;
                self.chosen = true;
                self.pending_apply = true;
                self.revision += 1;
                self.refresh_fields();
                self.say("Workspace updated. Apply explicitly to change the bridge.");
            }
            Err(error) => self.report(Err(error)),
        }
    }
    fn load(&mut self, profile: RemapProfile, path: Option<PathBuf>, dirty: bool) {
        match checked(profile) {
            Ok(profile) => {
                self.profile = profile;
                self.local_path = path;
                self.layer = 0;
                self.dirty = dirty;
                self.pending_apply = true;
                self.chosen = true;
                self.revision += 1;
                self.macro_selection = None;
                self.refresh_fields();
            }
            Err(error) => self.report(Err(error)),
        }
    }
    fn select_control(&mut self, control: PhysicalControl) {
        if self.selected == control {
            return;
        }
        if let Err(error) = self.clean() {
            self.report(Err(error));
            return;
        }
        self.selected = control;
        self.refresh_fields();
    }
    fn select_layer(&mut self, layer: usize) {
        if self.layer == layer {
            return;
        }
        if let Err(error) = self.clean() {
            self.report(Err(error));
            return;
        }
        self.layer = layer;
        self.refresh_fields();
    }
    fn request(&mut self, confirm: Confirm, ctx: &egui::Context) {
        if matches!(confirm, Confirm::Stop) || self.unsaved() {
            self.confirm = Some(confirm);
        } else {
            self.execute_confirm(confirm, ctx);
        }
    }
    fn execute_confirm(&mut self, confirm: Confirm, ctx: &egui::Context) {
        match confirm {
            Confirm::New => self.load(RemapProfile::default(), None, true),
            Confirm::Load(path) => self.send(Action::Load {
                path,
                revision: self.revision,
            }),
            Confirm::Import => self.send(Action::Import {
                revision: self.revision,
            }),
            Confirm::Reload => self.send(Action::Reload {
                revision: self.revision,
            }),
            Confirm::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Confirm::Stop => self.send(Action::Command(ControlCommand::Shutdown)),
        }
    }
    fn receive(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            match event {
                Event::Status(Status {
                    state,
                    starting,
                    error,
                }) => {
                    self.state = state;
                    self.starting = starting;
                    if let Some(error) = error
                        && !starting
                        && !self.error
                    {
                        self.message = format!("Bridge stopped / unavailable: {error}");
                    }
                }
                Event::InitialProfile(profile) => {
                    self.bridge_profile = profile.name.clone();
                    self.initial_profile = Some(profile);
                }
                Event::Loaded {
                    profile,
                    path,
                    revision,
                    imported,
                } => {
                    if revision == self.revision {
                        self.load(profile, path, imported);
                    } else {
                        self.message = "Profile result ignored: workspace changed while loading. Your draft was retained.".into();
                        self.error = true;
                    }
                }
                Event::Saved { path, revision } => {
                    if revision == self.revision {
                        self.local_path = Some(path);
                        self.dirty = false;
                    }
                }
                Event::Applied { revision, name } => {
                    self.bridge_profile = name;
                    if revision == self.revision {
                        self.pending_apply = false;
                    }
                }
                Event::Library(items) => {
                    self.library = items;
                    self.library_selection = self
                        .local_path
                        .as_ref()
                        .and_then(|path| self.library.iter().position(|item| &item.0 == path));
                }
                Event::Done(result) => {
                    self.busy = false;
                    match result {
                        Ok(message) => self.say(message),
                        Err(error) => {
                            self.message = error;
                            self.error = true;
                        }
                    }
                }
                Event::Notice(result) => match result {
                    Ok(message) => self.say(message),
                    Err(error) => {
                        self.message = error;
                        self.error = true;
                    }
                },
            }
        }
        let fields_dirty = self.fields_dirty();
        if let Some(profile) = take_initial_profile(
            &mut self.initial_profile,
            self.chosen,
            fields_dirty,
            self.macro_editor.is_some(),
            self.busy,
        ) {
            self.load(profile, None, false);
            self.pending_apply = false;
        }
        if !self.starting && !self.busy && !self.library_requested {
            self.library_requested = true;
            self.send(Action::Library);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.heading("SCUFPAD / CONTROL STUDIO");
        ui.horizontal_wrapped(|ui| {
            let (status, color) = if self.starting {
                ("Bridge: starting / checking…", Color32::YELLOW)
            } else if self.state.is_some() {
                ("Bridge: running", Color32::LIGHT_GREEN)
            } else {
                ("Bridge: stopped / unavailable", Color32::LIGHT_RED)
            };
            ui.colored_label(color, RichText::new(status).strong());
            if ui
                .add_enabled(
                    !self.busy && !self.starting,
                    egui::Button::new("Start bridge / retry"),
                )
                .clicked()
            {
                self.send(Action::Start);
            }
            if ui
                .add_enabled(
                    self.state.is_some() && !self.busy,
                    egui::Button::new("Resume outputs"),
                )
                .clicked()
            {
                self.send(Action::Command(ControlCommand::Arm));
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("STOP ALL OUTPUTS").strong())
                        .fill(Color32::from_rgb(125, 32, 40)),
                )
                .clicked()
            {
                self.worker.stop_outputs();
                self.say("Stop All requested. Waiting for bridge acknowledgement.");
            }
            if ui
                .add_enabled(
                    self.state.is_some() && !self.busy,
                    egui::Button::new("Stop bridge…"),
                )
                .clicked()
            {
                self.confirm = Some(Confirm::Stop);
            }
        });
        ui.colored_label(Color32::from_rgb(255, 209, 138), "Normal gamepad forwarding is automatic. For custom mappings, use an onboard profile with all six paddle/SAX aliases disabled. This editor NEVER programs firmware.");
        if let Some(state) = &self.state {
            ui.label(format!("Controller: {} · Gamepad output: {} · Custom-input verification: {} · Bridge profile: {} · Active layer mask: 0x{:X}",
                if state.connected { "connected" } else { "disconnected" }, if state.armed { "ON" } else { "OFF" },
                if state.hardware_ready { "ready" } else { "setup required for custom mappings only" }, state.profile_name, state.active_layers));
            ui.label(&state.status);
        } else {
            ui.label(
                "Controller and output telemetry unavailable. The local editor remains usable.",
            );
        }
    }

    fn workspace(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Workspace");
        ui.label(format!(
            "{}{}",
            self.profile.name,
            if self.dirty {
                " · UNSAVED"
            } else {
                " · saved / unchanged"
            }
        ));
        if self.fields_dirty() {
            ui.colored_label(Color32::YELLOW, "UNCOMMITTED FORM FIELDS");
        }
        ui.label(if self.pending_apply {
            "Pending Apply"
        } else {
            "Loaded / applied to bridge"
        });
        ui.small(format!("Last bridge profile: {}", self.bridge_profile));
        ui.label("Profile / game name");
        ui.text_edit_singleline(&mut self.profile_name);
        if ui.button("Save name").clicked() {
            let result = (|| {
                require_clean(
                    self.binding != self.baseline
                        || self.layer_name != self.profile.layers[self.layer].id,
                )?;
                let mut next = self.profile.clone();
                next.name = self.profile_name.trim().into();
                checked(next)
            })();
            self.commit(result);
        }
        let library_name = self
            .library_selection
            .and_then(|index| self.library.get(index))
            .map(|item| item.1.as_str())
            .unwrap_or("Select local profile");
        let mut selection = self.library_selection;
        egui::ComboBox::from_id_salt("library")
            .selected_text(library_name)
            .width(208.0)
            .show_ui(ui, |ui| {
                for (index, (_, name)) in self.library.iter().enumerate() {
                    ui.selectable_value(&mut selection, Some(index), name);
                }
            });
        if selection != self.library_selection {
            if self.fields_dirty() {
                self.report(require_clean(true));
            } else {
                self.library_selection = selection;
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Load").clicked() {
                if let Some(index) = self.library_selection {
                    self.request(Confirm::Load(self.library[index].0.clone()), ctx);
                } else {
                    self.report(Err(anyhow::anyhow!("Select a local profile first.")));
                }
            }
            if ui.button("New").clicked() {
                self.request(Confirm::New, ctx);
            }
            if ui.button("Refresh library").clicked() {
                self.send(Action::Library);
            }
        });
        if ui.button("Save locally").clicked() {
            self.save(false);
        }
        ui.horizontal(|ui| {
            if ui.button("Import JSON…").clicked() {
                self.request(Confirm::Import, ctx);
            }
            if ui.button("Export JSON…").clicked() {
                if let Err(error) = self.clean() {
                    self.report(Err(error));
                } else {
                    self.send(Action::Export(self.profile.clone()));
                }
            }
        });
        if ui
            .button(RichText::new("Apply to bridge + save").strong())
            .clicked()
        {
            self.save(true);
        }
        if ui.button("Reload from bridge…").clicked() {
            self.request(Confirm::Reload, ctx);
        }
        ui.separator();
        ui.heading("Layers");
        let layers: Vec<_> = self
            .profile
            .layers
            .iter()
            .map(|layer| layer.id.clone())
            .collect();
        for (index, name) in layers.iter().enumerate() {
            if ui
                .selectable_label(
                    self.layer == index,
                    format!(
                        "{index:02} {name}{}",
                        if index == 0 { " · BASE" } else { "" }
                    ),
                )
                .clicked()
            {
                self.select_layer(index);
            }
        }
        ui.text_edit_singleline(&mut self.layer_name);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Rename").clicked() {
                let result = require_clean(
                    self.binding != self.baseline || self.profile_name != self.profile.name,
                )
                .and_then(|_| rename_layer(&self.profile, self.layer, &self.layer_name));
                self.commit(result);
            }
            if ui.button("+ Layer").clicked() {
                let result = self.clean().and_then(|_| {
                    let mut next = self.profile.clone();
                    next.layers.push(LayerDefinition {
                        id: unique_id("Layer", next.layers.iter().map(|l| l.id.as_str())),
                        ..Default::default()
                    });
                    checked(next)
                });
                if result.is_ok() {
                    self.layer = self.profile.layers.len();
                }
                self.commit(result);
            }
            if ui
                .add_enabled(self.layer != 0, egui::Button::new("Remove"))
                .clicked()
            {
                let result = self.clean().and_then(|_| {
                    let mut next = self.profile.clone();
                    next.layers.remove(self.layer);
                    checked(next)
                });
                if result.is_ok() {
                    self.layer = 0;
                }
                self.commit(result);
            }
        });
        ui.small("Base is permanent. Highest active index wins. Held layers release with the physical button; toggles switch on presses.");
        ui.separator();
        ui.heading("Macros");
        for (index, definition) in self.profile.macros.iter().enumerate() {
            ui.selectable_value(
                &mut self.macro_selection,
                Some(index),
                format!(
                    "{}{}",
                    definition.id,
                    if definition.repeat_while_held {
                        " ↻"
                    } else {
                        ""
                    }
                ),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("+ Macro").clicked() {
                if let Err(error) = self.clean() {
                    self.report(Err(error));
                } else {
                    let definition = MacroDefinition {
                        id: unique_id("Macro", self.profile.macros.iter().map(|m| m.id.as_str())),
                        ..Default::default()
                    };
                    self.macro_editor = Some(MacroEditor::new(None, definition));
                }
            }
            if ui.button("Edit").clicked() {
                if let Err(error) = self.clean() {
                    self.report(Err(error));
                } else if let Some(index) = self.macro_selection {
                    self.macro_editor = Some(MacroEditor::new(
                        Some(index),
                        self.profile.macros[index].clone(),
                    ));
                } else {
                    self.report(Err(anyhow::anyhow!("Select a macro first.")));
                }
            }
            if ui.button("Delete").clicked() {
                let result = self.clean().and_then(|_| {
                    let index = self
                        .macro_selection
                        .ok_or_else(|| anyhow::anyhow!("Select a macro first."))?;
                    let mut next = self.profile.clone();
                    next.macros.remove(index);
                    checked(next)
                });
                if result.is_ok() {
                    self.macro_selection = None;
                }
                self.commit(result);
            }
        });
    }

    fn save(&mut self, apply: bool) {
        if let Err(error) = self.clean().and_then(|_| validate_profile(&self.profile)) {
            self.report(Err(error));
            return;
        }
        self.send(Action::Save {
            profile: self.profile.clone(),
            path: self.local_path.clone(),
            revision: self.revision,
            apply,
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Binding inspector");
        ui.label(RichText::new(label(self.selected)).size(22.0));
        ui.small(format!("Layer: {}", self.profile.layers[self.layer].id));
        if self.selected == PhysicalControl::HardwareProfile {
            ui.colored_label(Color32::YELLOW, "Read-only firmware control. Pressing Profile disarms outputs and clears verification.");
            return;
        }
        ui.label("Action kind");
        egui::ComboBox::from_id_salt("kind")
            .selected_text(format!("{:?}", self.binding.kind))
            .show_ui(ui, |ui| {
                for kind in KINDS {
                    if kind != BindingKind::Transparent || self.layer != 0 {
                        ui.selectable_value(&mut self.binding.kind, kind, format!("{kind:?}"));
                    }
                }
            });
        match self.binding.kind {
            BindingKind::Gamepad => {
                ui.label("Gamepad target");
                egui::ComboBox::from_id_salt("gamepad")
                    .selected_text(format!("{:?}", self.binding.gamepad))
                    .show_ui(ui, |ui| {
                        for control in GamepadControl::ALL {
                            ui.selectable_value(
                                &mut self.binding.gamepad,
                                *control,
                                format!("{control:?}"),
                            );
                        }
                    });
            }
            BindingKind::Keys => {
                ui.label("Keyboard chord · Linux evdev");
                ui.text_edit_singleline(&mut self.binding.chord);
                ui.small("CTRL+C, SHIFT+F12, A–Z, arrows, media keys, or KEY_1 … KEY_255. Separate keys with +.");
            }
            BindingKind::Mouse => {
                egui::ComboBox::from_id_salt("mouse")
                    .selected_text(mouse_label(self.binding.mouse))
                    .show_ui(ui, |ui| {
                        for code in 272..=279 {
                            ui.selectable_value(&mut self.binding.mouse, code, mouse_label(code));
                        }
                    });
            }
            BindingKind::Macro | BindingKind::MomentaryLayer | BindingKind::ToggleLayer => {
                let references: Vec<_> = if self.binding.kind == BindingKind::Macro {
                    self.profile.macros.iter().map(|m| m.id.clone()).collect()
                } else {
                    self.profile
                        .layers
                        .iter()
                        .skip(1)
                        .map(|l| l.id.clone())
                        .collect()
                };
                ui.label(if self.binding.kind == BindingKind::Macro {
                    "Macro reference"
                } else {
                    "Non-base layer reference"
                });
                egui::ComboBox::from_id_salt("reference")
                    .selected_text(if self.binding.reference.is_empty() {
                        "Choose…"
                    } else {
                        &self.binding.reference
                    })
                    .show_ui(ui, |ui| {
                        for reference in references {
                            ui.selectable_value(
                                &mut self.binding.reference,
                                reference.clone(),
                                reference,
                            );
                        }
                    });
            }
            _ => {}
        }
        ui.add_space(10.0);
        if ui.button("Save binding").clicked() {
            let candidate = require_clean(
                self.profile_name != self.profile.name
                    || self.layer_name != self.profile.layers[self.layer].id,
            )
            .and_then(|_| self.binding.binding())
            .and_then(|binding| {
                set_binding(&self.profile, self.layer, self.selected, Some(binding))
            });
            self.commit(candidate);
        }
        if ui.button("Remove override / use default").clicked() {
            let candidate = self
                .clean()
                .and_then(|_| set_binding(&self.profile, self.layer, self.selected, None));
            self.commit(candidate);
        }
        if ui.button("Discard form fields").clicked() {
            self.refresh_fields();
            self.say("Uncommitted fields discarded. Workspace bindings retained.");
        }
        ui.separator();
        ui.small("Passthrough uses standard mapping, including four virtual paddles. G1–G5 and SAX remain unbound until assigned.");
        ui.small("Transparent falls through to lower layers; Base cannot be transparent. Disabled suppresses the input.");
        ui.small("Sticks remain standard axes. Trigger bindings use the bridge press/release thresholds.");
        ui.small("Edits affect only this workspace until Apply. Closing the window never stops the bridge or its macros.");
    }

    fn controller(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Physical controller");
            ui.selectable_value(&mut self.rear, false, "Front");
            ui.selectable_value(&mut self.rear, true, "Rear");
        });
        if let Some(state) = &self.state {
            if !state.connected {
                ui.strong("Connect the controller for automatic gamepad forwarding. Custom mapping setup is separate.");
            } else if let Some(next) = setup_next(state) {
                let number = EXTRAS.iter().position(|c| *c == next).unwrap() + 1;
                ui.colored_label(
                    Color32::YELLOW,
                    RichText::new(format!(
                        "Step {number} / 6 · {}\nPress and release this input ALONE.",
                        label(next)
                    ))
                    .size(18.0),
                );
            } else {
                ui.colored_label(
                    Color32::LIGHT_GREEN,
                    if state.armed {
                        "Six inputs verified. The saved custom profile activates automatically."
                    } else {
                        "Six inputs verified. Resume outputs if you previously stopped them."
                    },
                );
            }
        } else {
            ui.strong("Start / connect the bridge for setup telemetry.");
        }
        self.diagram(ui);
        ui.small("Amber = selected · green = held · blue = recent press · blue border = setup step. Rear view is mirrored; names are positions as held.");
        ui.collapsing(
            "All 29 physical inputs · keyboard-accessible buttons",
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for control in PhysicalControl::ALL {
                        if self.control_button(ui, *control).clicked() {
                            self.select_control(*control);
                        }
                    }
                });
            },
        );
        ui.separator();
        ui.strong("Custom mapping setup / six independent inputs");
        ui.small("These checks are not required for ordinary gamepad use.");
        ui.small("Disable onboard aliases first. Follow the single named step without other extras, standard buttons or triggers. Only the bridge verifies coverage; no firmware writes occur.");
        ui.horizontal_wrapped(|ui| {
            for control in EXTRAS {
                let verified = self
                    .state
                    .as_ref()
                    .is_some_and(|s| s.connected && s.verified_extra_controls & control.bit() != 0);
                ui.colored_label(
                    if verified {
                        Color32::LIGHT_GREEN
                    } else {
                        Color32::GRAY
                    },
                    format!("{} {}", if verified { "●" } else { "○" }, label(control)),
                );
            }
        });
        if ui
            .button("Original onboard bindings / restoration help")
            .clicked()
        {
            self.restore_help = true;
        }
        ui.small("Hardware Profile or detected aliases suspend custom mappings, not normal gamepad forwarding. Disconnect releases outputs. Custom inputs must verify again after reconnect.");
        ui.separator();
        ui.strong("PHYSICAL AXES · not post-remap output axes");
        if let Some(state) = &self.state {
            ui.monospace(format!(
                "LX {:6}   LY {:6}\nRX {:6}   RY {:6}\nLT {:4} / 1023   RT {:4} / 1023",
                state.left_x,
                state.left_y,
                state.right_x,
                state.right_y,
                state.left_trigger,
                state.right_trigger
            ));
            ui.separator();
            ui.strong("OUTPUT MONITOR · bridge snapshot");
            let gamepad = GamepadControl::ALL
                .iter()
                .filter(|c| state.gamepad_buttons & c.bit() != 0)
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(format!("Gamepad 0x{:X}: {gamepad}", state.gamepad_buttons));
            let words = [
                state.keyboard0,
                state.keyboard1,
                state.keyboard2,
                state.keyboard3,
            ];
            let keys: Vec<_> = (1..=255)
                .filter(|code| words[*code as usize / 64] & (1u64 << (*code as usize % 64)) != 0)
                .collect();
            ui.label(format!("Keyboard: {}", format_chord(&keys)));
            ui.monospace(format!(
                "K0 {:016X} K1 {:016X}\nK2 {:016X} K3 {:016X}",
                words[0], words[1], words[2], words[3]
            ));
            let mouse = (0..8)
                .filter(|i| state.mouse_buttons & (1 << i) != 0)
                .map(|i| mouse_label(272 + i))
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(format!("Mouse 0x{:02X}: {mouse}", state.mouse_buttons));
        } else {
            ui.monospace("LX — LY — RX — RY — LT — RT —");
            ui.label("Output telemetry unavailable. Previous values cleared.");
        }
    }

    fn control_fill(&self, control: PhysicalControl) -> Color32 {
        if self
            .state
            .as_ref()
            .is_some_and(|s| s.physical_buttons & control.bit() != 0)
        {
            Color32::from_rgb(49, 105, 78)
        } else if self
            .state
            .as_ref()
            .is_some_and(|s| s.recent_presses & control.bit() != 0)
        {
            Color32::from_rgb(50, 94, 136)
        } else if control == self.selected {
            Color32::from_rgb(156, 112, 46)
        } else {
            Color32::from_rgb(53, 61, 74)
        }
    }
    fn control_widget(&self, control: PhysicalControl) -> egui::Button<'static> {
        let setup = self.state.as_ref().and_then(setup_next) == Some(control);
        let stroke = if setup {
            egui::Stroke::new(3.0_f32, Color32::LIGHT_BLUE)
        } else if control == self.selected {
            egui::Stroke::new(2.0_f32, Color32::from_rgb(255, 208, 128))
        } else {
            egui::Stroke::NONE
        };
        egui::Button::new(label(control))
            .fill(self.control_fill(control))
            .stroke(stroke)
    }
    fn control_button(&self, ui: &mut egui::Ui, control: PhysicalControl) -> egui::Response {
        ui.add(self.control_widget(control)).on_hover_text(format!(
            "{} · {:?}{}",
            label(control),
            self.profile.layers[self.layer]
                .bindings
                .get(&control)
                .map(|b| b.kind)
                .unwrap_or(if self.layer == 0 {
                    BindingKind::Passthrough
                } else {
                    BindingKind::Transparent
                }),
            if control == PhysicalControl::HardwareProfile {
                " · firmware only"
            } else {
                ""
            }
        ))
    }
    fn diagram(&mut self, ui: &mut egui::Ui) {
        use PhysicalControl::*;
        let scale = (ui.available_width() / 580.0).min(1.25);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(580.0 * scale, 390.0 * scale),
            egui::Sense::hover(),
        );
        let at = |x: f32, y: f32| rect.min + egui::vec2(x * scale, y * scale);
        let outline = [
            (130., 85.),
            (90., 75.),
            (60., 130.),
            (22., 304.),
            (35., 360.),
            (75., 365.),
            (166., 265.),
            (414., 265.),
            (505., 365.),
            (545., 360.),
            (558., 304.),
            (520., 130.),
            (490., 75.),
            (450., 85.),
            (390., 105.),
            (190., 105.),
        ];
        // Convex pieces avoid triangulating the controller's concave outline.
        ui.painter().rect_filled(
            egui::Rect::from_min_max(at(75., 95.), at(505., 270.)),
            35.0 * scale,
            Color32::from_rgb(35, 42, 52),
        );
        ui.painter().line(
            outline
                .into_iter()
                .chain(std::iter::once((130., 85.)))
                .map(|(x, y)| at(x, y))
                .collect(),
            egui::Stroke::new(2.0_f32, Color32::from_rgb(82, 96, 113)),
        );
        for (x, y) in [(200., 280.), (380., 280.)] {
            ui.painter()
                .circle_filled(at(x, y), 42.0 * scale, Color32::from_rgb(25, 30, 38));
        }
        let mut positions = vec![
            (LeftTrigger, 100., 32., 72.),
            (RightTrigger, 480., 32., 72.),
            (LeftBumper, 100., 80., 72.),
            (RightBumper, 480., 80., 72.),
            (SaxLeft, 40., 135., 76.),
            (SaxRight, 540., 135., 76.),
        ];
        if self.rear {
            positions.extend([
                (RearOuterLeft, 100., 215., 122.),
                (RearInnerLeft, 215., 270., 122.),
                (RearInnerRight, 365., 270., 122.),
                (RearOuterRight, 480., 215., 122.),
            ]);
        } else {
            positions.extend([
                (A, 465., 235., 40.),
                (B, 509., 195., 40.),
                (X, 421., 195., 40.),
                (Y, 465., 155., 40.),
                (DpadUp, 145., 153., 72.),
                (DpadLeft, 85., 195., 84.),
                (DpadRight, 205., 195., 84.),
                (DpadDown, 145., 237., 86.),
                (LeftStickClick, 200., 280., 100.),
                (RightStickClick, 380., 280., 100.),
                (View, 245., 155., 65.),
                (Menu, 335., 155., 65.),
                (Guide, 290., 280., 64.),
                (HardwareProfile, 290., 316., 222.),
                (G1, 200., 350., 40.),
                (G2, 245., 350., 40.),
                (G3, 290., 350., 40.),
                (G4, 335., 350., 40.),
                (G5, 380., 350., 40.),
            ]);
        }
        for (control, x, y, width) in positions {
            let x = if self.rear { 580.0 - x } else { x };
            let button_rect =
                egui::Rect::from_center_size(at(x, y), egui::vec2(width * scale, 32.0 * scale));
            if ui.put(button_rect, self.control_widget(control)).clicked() {
                self.select_control(control);
            }
        }
        ui.painter().text(
            at(290., 381.),
            egui::Align2::CENTER_CENTER,
            if self.rear {
                "REAR VIEW · mirrored · names as held"
            } else {
                "FRONT VIEW · physical inputs"
            },
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
        ui.advance_cursor_after_rect(rect);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(confirm) = &self.confirm {
            let stop = matches!(confirm, Confirm::Stop);
            let mut accept = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirmation")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading(if stop { "Stop bridge?" } else { "Discard unsaved workspace / fields?" });
                ui.label(if stop { "This explicitly shuts down the independent daemon and releases all outputs. Closing the editor alone never does this." }
                    else { "Unsaved bindings, names, macros and invalid form fields will be discarded. Save fields, then save locally or export to retain them. The running bridge is not changed." });
                ui.horizontal(|ui| {
                    cancel = ui.button("Cancel").clicked();
                    accept = ui.button(if stop { "Stop bridge" } else { "Discard / continue" }).clicked();
                });
                if ui.add(egui::Button::new("STOP ALL OUTPUTS").fill(Color32::from_rgb(125, 32, 40))).clicked() {
                    self.worker.stop_outputs();
                }
            });
            if accept {
                let confirm = self.confirm.take().unwrap();
                self.execute_confirm(confirm, ctx);
            } else if cancel {
                self.confirm = None;
            }
        }
        if self.restore_help {
            egui::Window::new("Original onboard bindings / restoration").open(&mut self.restore_help).resizable(false).show(ctx, |ui| {
                ui.set_max_width(500.0);
                ui.label("Onboard bindings depend on the selected hardware slot. Record that slot's actual bindings before changing anything; do not substitute another slot's restoration map.");
                ui.hyperlink_to("SCUF official support / Envision manuals", "https://customercare.scufgaming.com/");
                ui.label("Use the manufacturer's documented procedure, one control at a time. This editor never sends firmware writes or changes onboard profiles.");
                ui.label("Restoring aliases makes that profile unsuitable for independent host remapping. Hardware Profile returns to normal gamepad forwarding; verify all six inputs again before custom mappings resume.");
            });
        }
        if self.confirm.is_none()
            && let Some(mut editor) = self.macro_editor.take()
        {
            match editor.show(ctx, &self.profile) {
                MacroOutcome::Keep => self.macro_editor = Some(editor),
                MacroOutcome::Saved(profile) => self.commit(Ok(profile)),
                MacroOutcome::Discard => {
                    self.say("Macro draft discarded. Existing workspace retained.")
                }
            }
        }
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive();
        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            if self.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.say("Finish the current file or bridge operation before closing. The daemon is independent.");
            } else if self.unsaved() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm = Some(Confirm::Close);
            }
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("message").show(ctx, |ui| {
            ui.colored_label(
                if self.error {
                    Color32::LIGHT_RED
                } else {
                    Color32::LIGHT_GRAY
                },
                &self.message,
            );
        });
        let editable = !self.busy && self.confirm.is_none() && self.macro_editor.is_none();
        egui::SidePanel::left("workspace")
            .exact_width(236.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_enabled_ui(editable, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("workspace-scroll")
                        .show(ui, |ui| self.workspace(ui, ctx));
                });
            });
        egui::SidePanel::right("inspector")
            .exact_width(284.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_enabled_ui(editable, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("inspector-scroll")
                        .show(ui, |ui| self.inspector(ui));
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(editable, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("controller-scroll")
                    .show(ui, |ui| self.controller(ui));
            });
        });
        self.dialogs(ctx);
    }
}

fn mouse_label(code: i32) -> String {
    let name = [
        "Left", "Right", "Middle", "Side", "Extra", "Forward", "Back", "Task",
    ]
    .get((code - 272) as usize)
    .copied()
    .unwrap_or("Invalid");
    format!("{name} · {code}")
}

enum MacroOutcome {
    Keep,
    Saved(RemapProfile),
    Discard,
}
struct MacroEditor {
    index: Option<usize>,
    definition: MacroDefinition,
    selected: Option<usize>,
    draft: StepDraft,
    baseline: StepDraft,
    error: String,
    confirm_discard: bool,
}
impl MacroEditor {
    fn new(index: Option<usize>, definition: MacroDefinition) -> Self {
        let selected = (!definition.steps.is_empty()).then_some(0);
        let fallback = MacroStep {
            kind: MacroStepKind::Delay,
            code: 0,
            delay_ms: 50,
        };
        let draft = StepDraft::load(definition.steps.first().unwrap_or(&fallback));
        Self {
            index,
            definition,
            selected,
            baseline: draft.clone(),
            draft,
            error: String::new(),
            confirm_discard: false,
        }
    }
    fn dirty_step(&self) -> bool {
        self.selected.is_some() && self.draft != self.baseline
    }
    fn select(&mut self, selected: Option<usize>) {
        self.selected = selected;
        if let Some(index) = selected {
            self.draft = StepDraft::load(&self.definition.steps[index]);
            self.baseline = self.draft.clone();
        }
    }
    fn save_step(&mut self) -> Result<()> {
        let index = self
            .selected
            .ok_or_else(|| anyhow::anyhow!("Select a macro step first."))?;
        let step = self.draft.step()?;
        let mut steps = self.definition.steps.clone();
        steps[index] = step;
        ensure!(
            steps.iter().map(|s| i64::from(s.delay_ms)).sum::<i64>() <= 60000,
            "Total macro delay cannot exceed 60 seconds."
        );
        self.definition.steps = steps;
        self.select(Some(index));
        Ok(())
    }
    fn move_step(&mut self, delta: isize) -> Result<()> {
        require_clean(self.dirty_step())?;
        let index = self
            .selected
            .ok_or_else(|| anyhow::anyhow!("Select a macro step first."))?;
        let target = index as isize + delta;
        if target >= 0 && (target as usize) < self.definition.steps.len() {
            self.definition.steps.swap(index, target as usize);
            self.select(Some(target as usize));
        }
        Ok(())
    }
    fn show(&mut self, ctx: &egui::Context, profile: &RemapProfile) -> MacroOutcome {
        let mut outcome = MacroOutcome::Keep;
        egui::Window::new("Macro composer · finite sequence").id(egui::Id::new("macro-editor")).collapsible(false).resizable(true).default_width(700.0).show(ctx, |ui| {
            ui.set_max_height(640.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("Macro name"); ui.text_edit_singleline(&mut self.definition.id);
                ui.checkbox(&mut self.definition.repeat_while_held, "Repeat while physical control is held");
                ui.small("At most 256 steps and 60 seconds total delay. Repeats require a positive delay. Host macros are not OS real-time scheduling.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Step").clicked() {
                        let result = require_clean(self.dirty_step()).and_then(|_| {
                            ensure!(self.definition.steps.len() < 256, "A macro supports at most 256 steps.");
                            self.definition.steps.push(MacroStep { kind: MacroStepKind::Delay, code: 0, delay_ms: 50 });
                            self.select(Some(self.definition.steps.len() - 1)); Ok(())
                        });
                        if let Err(error) = result { self.error = error.to_string(); }
                    }
                    if ui.button("Up").clicked() && let Err(error) = self.move_step(-1) { self.error = error.to_string(); }
                    if ui.button("Down").clicked() && let Err(error) = self.move_step(1) { self.error = error.to_string(); }
                    if ui.button("Remove step").clicked() {
                        let result = require_clean(self.dirty_step()).and_then(|_| {
                            let index = self.selected.ok_or_else(|| anyhow::anyhow!("Select a step first."))?;
                            self.definition.steps.remove(index);
                            self.select(if self.definition.steps.is_empty() { None } else { Some(index.min(self.definition.steps.len() - 1)) }); Ok(())
                        });
                        if let Err(error) = result { self.error = error.to_string(); }
                    }
                });
                egui::ScrollArea::vertical().id_salt("macro-rows").max_height(190.0).show(ui, |ui| {
                    for index in 0..self.definition.steps.len() {
                        let step = &self.definition.steps[index];
                        let text = format!("{:03}  {:?}  {}", index + 1, step.kind, if step.kind == MacroStepKind::Delay { format!("{} ms", step.delay_ms) } else { StepDraft::load(step).code });
                        if ui.selectable_label(self.selected == Some(index), text).clicked() && self.selected != Some(index) {
                            if self.dirty_step() { self.error = "Save or discard step fields before selecting another row. Invalid fields retained.".into(); }
                            else { self.select(Some(index)); }
                        }
                    }
                });
                ui.separator();
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    ui.label("Selected step kind");
                    egui::ComboBox::from_id_salt("step-kind").selected_text(format!("{:?}", self.draft.kind)).show_ui(ui, |ui| {
                        for kind in STEP_KINDS { ui.selectable_value(&mut self.draft.kind, kind, format!("{kind:?}")); }
                    });
                    if self.draft.kind == MacroStepKind::Delay { ui.label("Delay (milliseconds)"); ui.text_edit_singleline(&mut self.draft.delay); }
                    else { ui.label("Code / Linux key name / gamepad name"); ui.text_edit_singleline(&mut self.draft.code); }
                    ui.small("Key: CTRL, C, KEY_200 (Linux 1–255). Mouse: 272–279. Gamepad: A, LeftBumper, Paddle1, etc., or 0–20.");
                    ui.horizontal(|ui| {
                        if ui.button("Save step").clicked() { match self.save_step() { Ok(()) => self.error.clear(), Err(error) => self.error = error.to_string() } }
                        if ui.button("Discard step fields").clicked() { self.select(self.selected); self.error.clear(); }
                    });
                });
                if !self.error.is_empty() { ui.colored_label(Color32::LIGHT_RED, &self.error); }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save macro").clicked() {
                        let candidate = require_clean(self.dirty_step()).and_then(|_| {
                            let mut definition = self.definition.clone(); definition.id = definition.id.trim().into();
                            save_macro(profile, self.index, definition)
                        });
                        match candidate { Ok(profile) => outcome = MacroOutcome::Saved(profile), Err(error) => self.error = error.to_string() }
                    }
                    if ui.button("Cancel / discard macro…").clicked() { self.confirm_discard = true; }
                });
                if self.confirm_discard {
                    ui.colored_label(Color32::YELLOW, "Discard all macro draft changes, including invalid fields?");
                    ui.horizontal(|ui| {
                        if ui.button("Keep editing").clicked() { self.confirm_discard = false; }
                        if ui.button("Discard macro draft").clicked() { outcome = MacroOutcome::Discard; }
                    });
                }
            });
        });
        outcome
    }
}

pub fn check_macro_drafts() -> Result<()> {
    let definition = MacroDefinition {
        id: "Draft check".into(),
        repeat_while_held: false,
        steps: vec![
            MacroStep {
                kind: MacroStepKind::Delay,
                code: 0,
                delay_ms: 10,
            },
            MacroStep {
                kind: MacroStepKind::Delay,
                code: 0,
                delay_ms: 20,
            },
        ],
    };
    let mut editor = MacroEditor::new(None, definition.clone());
    editor.move_step(1)?;
    ensure!(
        editor.selected == Some(1) && editor.definition.steps[0].delay_ms == 20,
        "Step reorder failed."
    );
    editor.draft.delay = "invalid".into();
    ensure!(
        editor.move_step(-1).is_err(),
        "Reorder discarded an invalid step draft."
    );
    ensure!(editor.save_step().is_err(), "Invalid delay saved.");
    ensure!(
        editor.selected == Some(1)
            && editor.draft.delay == "invalid"
            && editor.definition.steps[1].delay_ms == 10,
        "Rejected step edit dropped fields or changed its saved value."
    );
    editor.draft.delay = "50".into();
    editor.save_step()?;
    ensure!(
        !editor.dirty_step() && editor.definition.steps[1].delay_ms == 50,
        "Save step did not establish a clean baseline."
    );
    ensure!(
        definition.steps[0].delay_ms == 10,
        "Macro draft edits mutated the original definition."
    );
    editor.move_step(-1)?;
    ensure!(
        editor.selected == Some(0) && editor.definition.steps[0].delay_ms == 50,
        "Moving a saved step failed."
    );
    Ok(())
}
