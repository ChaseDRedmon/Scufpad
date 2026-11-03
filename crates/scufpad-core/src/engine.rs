use crate::*;
use anyhow::{Result, ensure};

#[derive(Clone, Debug, PartialEq)]
pub struct OutputState {
    pub gamepad_buttons: u64,
    pub keyboard: [bool; 256],
    pub mouse: [bool; 8],
    pub left_x: i32,
    pub left_y: i32,
    pub right_x: i32,
    pub right_y: i32,
    pub left_trigger: i32,
    pub right_trigger: i32,
}
impl Default for OutputState {
    fn default() -> Self {
        Self {
            gamepad_buttons: 0,
            keyboard: [false; 256],
            mouse: [false; 8],
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
            left_trigger: 0,
            right_trigger: 0,
        }
    }
}

impl OutputState {
    pub fn gamepad_passthrough(input: PhysicalState) -> Self {
        let mut output = Self {
            left_x: input.left_x,
            left_y: input.left_y,
            right_x: input.right_x,
            right_y: input.right_y,
            left_trigger: input.left_trigger,
            right_trigger: input.right_trigger,
            ..Default::default()
        };
        for &control in PhysicalControl::ALL {
            if input.buttons & !EXTRA_BUTTONS_MASK & control.bit() != 0
                && let Some(code) = default_gamepad(control as usize)
            {
                output.gamepad_buttons |= 1 << code;
            }
        }
        output
    }
}

struct Owners {
    gamepad: [i32; 21],
    keys: [i32; 256],
    mouse: [i32; 8],
}
impl Default for Owners {
    fn default() -> Self {
        Self {
            gamepad: [0; 21],
            keys: [0; 256],
            mouse: [0; 8],
        }
    }
}

struct MacroInstance {
    definition: Option<usize>,
    step: usize,
    due: u64,
    deadline: u64,
    keys: [bool; 256],
    mouse: [bool; 8],
    gamepad: [bool; 21],
}
impl Default for MacroInstance {
    fn default() -> Self {
        Self {
            definition: None,
            step: 0,
            due: 0,
            deadline: 0,
            keys: [false; 256],
            mouse: [false; 8],
            gamepad: [false; 21],
        }
    }
}

pub struct MappingEngine {
    profile: RemapProfile,
    layers: Vec<[Binding; 31]>,
    held: [Option<usize>; 31],
    owners: Owners,
    layer_owners: [i32; 32],
    layer_toggles: [bool; 32],
    instances: [MacroInstance; 31],
    previous: u64,
    suppressed: u64,
    started: bool,
    left_trigger_held: bool,
    right_trigger_held: bool,
    last_ms: u64,
    macro_cursor: usize,
    pub output: OutputState,
    pub active_layers: u64,
    pub physical_buttons: u64,
    pub next_due_ms: Option<u64>,
}

impl MappingEngine {
    pub fn new(profile: RemapProfile) -> Result<Self> {
        validate_profile(&profile)?;
        let mut engine = Self {
            profile: RemapProfile::default(),
            layers: Vec::new(),
            held: [None; 31],
            owners: Owners::default(),
            layer_owners: [0; 32],
            layer_toggles: [false; 32],
            instances: std::array::from_fn(|_| MacroInstance::default()),
            previous: 0,
            suppressed: 0,
            started: false,
            left_trigger_held: false,
            right_trigger_held: false,
            last_ms: 0,
            macro_cursor: 0,
            output: OutputState::default(),
            active_layers: 1,
            physical_buttons: 0,
            next_due_ms: None,
        };
        engine.apply_profile(profile)?;
        Ok(engine)
    }

    pub fn profile(&self) -> &RemapProfile {
        &self.profile
    }

    pub fn apply_profile(&mut self, profile: RemapProfile) -> Result<()> {
        validate_profile(&profile)?;
        let mut layers = Vec::with_capacity(profile.layers.len() + 1);
        for layer in &profile.layers {
            let mut bindings = std::array::from_fn(|_| Binding {
                kind: BindingKind::Transparent,
                ..Binding::default()
            });
            for (&source, binding) in &layer.bindings {
                bindings[source as usize] = binding.clone();
            }
            layers.push(bindings);
        }
        layers.push(std::array::from_fn(|_| Binding::default()));
        self.cancel_all();
        self.profile = profile;
        self.layers = layers;
        Ok(())
    }

    pub fn cancel_all(&mut self) {
        self.suppressed |= self.physical_buttons;
        self.held.fill(None);
        self.owners = Owners::default();
        self.layer_owners.fill(0);
        self.layer_toggles.fill(false);
        for instance in &mut self.instances {
            *instance = MacroInstance::default();
        }
        self.active_layers = 1;
        self.next_due_ms = None;
        self.output = OutputState::default();
    }

    pub fn tick(&mut self, input: PhysicalState, now: u64) -> Result<()> {
        ensure!(
            !self.started || now >= self.last_ms,
            "The clock must be monotonic."
        );
        self.last_ms = now;
        self.left_trigger_held = self.trigger_held(input.left_trigger, self.left_trigger_held);
        self.right_trigger_held = self.trigger_held(input.right_trigger, self.right_trigger_held);
        self.physical_buttons = input.buttons
            & !(PhysicalControl::LeftTrigger.bit() | PhysicalControl::RightTrigger.bit());
        if self.left_trigger_held {
            self.physical_buttons |= PhysicalControl::LeftTrigger.bit();
        }
        if self.right_trigger_held {
            self.physical_buttons |= PhysicalControl::RightTrigger.bit();
        }
        if !self.started {
            self.suppressed |= self.physical_buttons;
            self.started = true;
        }
        self.suppressed &= self.physical_buttons;
        let current = self.physical_buttons & !self.suppressed;
        let released = self.previous & !current;
        for source in 0..31 {
            if released & (1 << source) != 0 {
                self.release(source);
            }
        }
        self.refresh_layers();
        let mut pressed = current & !self.previous & !HARDWARE_PROFILE_MASK;
        loop {
            let mut changed = false;
            for source in 0..31 {
                let bit = 1 << source;
                if pressed & bit == 0 {
                    continue;
                }
                let layer = self.resolve(source);
                let binding = &self.layers[layer][source];
                if !matches!(
                    binding.kind,
                    BindingKind::MomentaryLayer | BindingKind::ToggleLayer
                ) {
                    continue;
                }
                self.held[source] = Some(layer);
                let target = self
                    .profile
                    .layers
                    .iter()
                    .position(|l| l.id == binding.reference)
                    .unwrap();
                if binding.kind == BindingKind::MomentaryLayer {
                    self.layer_owners[target] += 1;
                } else {
                    self.layer_toggles[target] = !self.layer_toggles[target];
                }
                pressed &= !bit;
                self.refresh_layers();
                changed = true;
            }
            if !changed {
                break;
            }
        }
        for source in 0..31 {
            if pressed & (1 << source) == 0 {
                continue;
            }
            let layer = self.resolve(source);
            self.held[source] = Some(layer);
            let binding = &self.layers[layer][source];
            if binding.kind == BindingKind::Macro {
                let definition = self
                    .profile
                    .macros
                    .iter()
                    .position(|m| m.id == binding.reference)
                    .unwrap();
                let instance = &mut self.instances[source];
                if instance.definition.is_none() {
                    instance.definition = Some(definition);
                    instance.step = 0;
                    instance.due = now;
                    instance.deadline = now.saturating_add(60000);
                }
            } else {
                change_binding(&mut self.owners, source, binding, 1);
            }
        }
        self.previous = current;
        self.run_macros(now);
        self.update_output(input);
        Ok(())
    }

    fn trigger_held(&self, value: i32, held: bool) -> bool {
        if held {
            value > self.profile.trigger_release_threshold
        } else {
            value >= self.profile.trigger_press_threshold
        }
    }

    fn resolve(&self, source: usize) -> usize {
        for layer in (0..self.profile.layers.len()).rev() {
            if self.active_layers & (1 << layer) != 0
                && self.layers[layer][source].kind != BindingKind::Transparent
            {
                return layer;
            }
        }
        self.profile.layers.len()
    }

    fn release(&mut self, source: usize) {
        let Some(layer) = self.held[source].take() else {
            return;
        };
        let binding = &self.layers[layer][source];
        match binding.kind {
            BindingKind::MomentaryLayer => {
                let target = self
                    .profile
                    .layers
                    .iter()
                    .position(|l| l.id == binding.reference)
                    .unwrap();
                self.layer_owners[target] -= 1;
            }
            BindingKind::Macro => {
                let instance = &mut self.instances[source];
                if instance
                    .definition
                    .is_some_and(|i| self.profile.macros[i].repeat_while_held)
                {
                    stop_macro(instance, &mut self.owners);
                }
            }
            _ => change_binding(&mut self.owners, source, binding, -1),
        }
    }

    fn refresh_layers(&mut self) {
        self.active_layers = 1;
        for i in 1..self.profile.layers.len() {
            if self.layer_owners[i] > 0 || self.layer_toggles[i] {
                self.active_layers |= 1 << i;
            }
        }
    }

    fn run_macros(&mut self, now: u64) {
        for instance in &mut self.instances {
            if instance.definition.is_some() && now >= instance.deadline {
                stop_macro(instance, &mut self.owners);
            }
        }
        let mut budget = 64;
        let mut idle = 0;
        while budget > 0 && idle < 31 {
            let instance = &mut self.instances[self.macro_cursor];
            self.macro_cursor = (self.macro_cursor + 1) % 31;
            let Some(index) = instance.definition else {
                idle += 1;
                continue;
            };
            if instance.due > now {
                idle += 1;
                continue;
            }
            idle = 0;
            budget -= 1;
            let definition = &self.profile.macros[index];
            if instance.step == definition.steps.len() {
                release_macro_outputs(instance, &mut self.owners);
                if !definition.repeat_while_held {
                    *instance = MacroInstance::default();
                    continue;
                }
                instance.step = 0;
            }
            let step = &definition.steps[instance.step];
            instance.step += 1;
            match step.kind {
                MacroStepKind::Delay => {
                    instance.due = instance.due.saturating_add(step.delay_ms as u64)
                }
                MacroStepKind::PressKey | MacroStepKind::ReleaseKey => change_macro_owner(
                    &mut instance.keys,
                    &mut self.owners.keys,
                    step.code as usize,
                    step.kind == MacroStepKind::PressKey,
                ),
                MacroStepKind::PressMouse | MacroStepKind::ReleaseMouse => change_macro_owner(
                    &mut instance.mouse,
                    &mut self.owners.mouse,
                    (step.code - 272) as usize,
                    step.kind == MacroStepKind::PressMouse,
                ),
                MacroStepKind::PressGamepad | MacroStepKind::ReleaseGamepad => change_macro_owner(
                    &mut instance.gamepad,
                    &mut self.owners.gamepad,
                    step.code as usize,
                    step.kind == MacroStepKind::PressGamepad,
                ),
            }
        }
        self.next_due_ms = self
            .instances
            .iter()
            .filter_map(|instance| {
                instance.definition.map(|index| {
                    if self.profile.macros[index].repeat_while_held {
                        instance.due.min(instance.deadline)
                    } else {
                        instance.due
                    }
                })
            })
            .min();
    }

    fn update_output(&mut self, input: PhysicalState) {
        self.output.gamepad_buttons = 0;
        for (i, &owners) in self.owners.gamepad.iter().enumerate() {
            if owners > 0 {
                self.output.gamepad_buttons |= 1 << i;
            }
        }
        for (output, &owners) in self.output.keyboard.iter_mut().zip(&self.owners.keys) {
            *output = owners > 0;
        }
        for (output, &owners) in self.output.mouse.iter_mut().zip(&self.owners.mouse) {
            *output = owners > 0;
        }
        self.output.left_x = input.left_x;
        self.output.left_y = input.left_y;
        self.output.right_x = input.right_x;
        self.output.right_y = input.right_y;
        self.output.left_trigger = self.trigger_output(29, input.left_trigger);
        self.output.right_trigger = self.trigger_output(30, input.right_trigger);
    }

    fn trigger_output(&self, source: usize, analog: i32) -> i32 {
        let layer = self.held[source].unwrap_or_else(|| self.resolve(source));
        let native = self.suppressed & (1 << source) == 0
            && self.layers[layer][source].kind == BindingKind::Passthrough;
        let mut digital = self.owners.gamepad[default_gamepad(source).unwrap()];
        if self.held[source]
            .is_some_and(|l| self.layers[l][source].kind == BindingKind::Passthrough)
        {
            digital -= 1;
        }
        if digital > 0 {
            1023
        } else if native {
            analog
        } else {
            0
        }
    }
}

fn default_gamepad(source: usize) -> Option<usize> {
    match source {
        0..=7 => Some(source),
        8 => Some(9),
        9 => Some(10),
        10 => Some(8),
        22 => Some(11),
        21 => Some(12),
        20 => Some(13),
        19 => Some(14),
        25..=30 => Some(source - 10),
        _ => None,
    }
}

fn change_binding(owners: &mut Owners, source: usize, binding: &Binding, change: i32) {
    match binding.kind {
        BindingKind::Passthrough => {
            if let Some(code) = default_gamepad(source) {
                owners.gamepad[code] += change;
            }
        }
        BindingKind::Gamepad => owners.gamepad[binding.code as usize] += change,
        BindingKind::Keys => {
            for &key in &binding.keys {
                owners.keys[key as usize] += change;
            }
        }
        BindingKind::Mouse => owners.mouse[(binding.code - 272) as usize] += change,
        _ => {}
    }
}

fn change_macro_owner(held: &mut [bool], owners: &mut [i32], code: usize, press: bool) {
    if held[code] != press {
        held[code] = press;
        owners[code] += if press { 1 } else { -1 };
    }
}
fn release_macro_outputs(instance: &mut MacroInstance, owners: &mut Owners) {
    for i in 0..256 {
        change_macro_owner(&mut instance.keys, &mut owners.keys, i, false);
    }
    for i in 0..8 {
        change_macro_owner(&mut instance.mouse, &mut owners.mouse, i, false);
    }
    for i in 0..21 {
        change_macro_owner(&mut instance.gamepad, &mut owners.gamepad, i, false);
    }
}
fn stop_macro(instance: &mut MacroInstance, owners: &mut Owners) {
    release_macro_outputs(instance, owners);
    *instance = MacroInstance::default();
}
