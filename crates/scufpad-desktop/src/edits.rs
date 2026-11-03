use anyhow::{Result, bail, ensure};
use scufpad_core::*;
use std::path::{Path, PathBuf};

pub const EXTRAS: [PhysicalControl; 6] = [
    PhysicalControl::RearOuterLeft,
    PhysicalControl::RearInnerLeft,
    PhysicalControl::RearInnerRight,
    PhysicalControl::RearOuterRight,
    PhysicalControl::SaxLeft,
    PhysicalControl::SaxRight,
];
pub const KINDS: [BindingKind; 9] = [
    BindingKind::Passthrough,
    BindingKind::Disabled,
    BindingKind::Gamepad,
    BindingKind::Keys,
    BindingKind::Mouse,
    BindingKind::Macro,
    BindingKind::Transparent,
    BindingKind::MomentaryLayer,
    BindingKind::ToggleLayer,
];
pub const STEP_KINDS: [MacroStepKind; 7] = [
    MacroStepKind::PressKey,
    MacroStepKind::ReleaseKey,
    MacroStepKind::PressMouse,
    MacroStepKind::ReleaseMouse,
    MacroStepKind::PressGamepad,
    MacroStepKind::ReleaseGamepad,
    MacroStepKind::Delay,
];

pub fn checked(profile: RemapProfile) -> Result<RemapProfile> {
    validate_profile(&profile)?;
    Ok(profile)
}

pub fn set_binding(
    profile: &RemapProfile,
    layer: usize,
    control: PhysicalControl,
    binding: Option<Binding>,
) -> Result<RemapProfile> {
    ensure!(
        control != PhysicalControl::HardwareProfile,
        "Hardware Profile is firmware-only and read-only."
    );
    let mut next = profile.clone();
    let bindings = &mut next
        .layers
        .get_mut(layer)
        .ok_or_else(|| anyhow::anyhow!("Select a layer."))?
        .bindings;
    if let Some(binding) = binding {
        bindings.insert(control, binding);
    } else {
        bindings.remove(&control);
    }
    checked(next)
}

pub fn rename_layer(profile: &RemapProfile, index: usize, name: &str) -> Result<RemapProfile> {
    let mut next = profile.clone();
    let old = next.layers[index].id.clone();
    let name = name.trim();
    next.layers[index].id = name.into();
    if next.base_layer == old {
        next.base_layer = name.into();
    }
    for layer in &mut next.layers {
        for binding in layer.bindings.values_mut() {
            if matches!(
                binding.kind,
                BindingKind::MomentaryLayer | BindingKind::ToggleLayer
            ) && binding.reference == old
            {
                binding.reference = name.into();
            }
        }
    }
    checked(next)
}

pub fn save_macro(
    profile: &RemapProfile,
    index: Option<usize>,
    definition: MacroDefinition,
) -> Result<RemapProfile> {
    let mut next = profile.clone();
    if let Some(index) = index {
        let old = next.macros[index].id.clone();
        for layer in &mut next.layers {
            for binding in layer.bindings.values_mut() {
                if binding.kind == BindingKind::Macro && binding.reference == old {
                    binding.reference = definition.id.clone();
                }
            }
        }
        next.macros[index] = definition;
    } else {
        next.macros.push(definition);
    }
    checked(next)
}

pub fn unique_id<'a>(prefix: &str, ids: impl Iterator<Item = &'a str>) -> String {
    let ids: Vec<_> = ids.collect();
    (1..)
        .map(|n| format!("{prefix} {n}"))
        .find(|name| !ids.contains(&name.as_str()))
        .unwrap()
}

pub fn setup_next(state: &BridgeState) -> Option<PhysicalControl> {
    state
        .connected
        .then(|| {
            EXTRAS
                .into_iter()
                .find(|c| state.verified_extra_controls & c.bit() == 0)
        })
        .flatten()
}

pub fn label(control: PhysicalControl) -> String {
    match control {
        PhysicalControl::RearOuterLeft => "Outer-left rear",
        PhysicalControl::RearInnerLeft => "Inner-left rear",
        PhysicalControl::RearInnerRight => "Inner-right rear",
        PhysicalControl::RearOuterRight => "Outer-right rear",
        PhysicalControl::SaxLeft => "SAX left",
        PhysicalControl::SaxRight => "SAX right",
        PhysicalControl::LeftBumper => "LB",
        PhysicalControl::RightBumper => "RB",
        PhysicalControl::LeftTrigger => "LT",
        PhysicalControl::RightTrigger => "RT",
        PhysicalControl::LeftStickClick => "Left stick",
        PhysicalControl::RightStickClick => "Right stick",
        PhysicalControl::DpadUp => "D-pad up",
        PhysicalControl::DpadDown => "D-pad down",
        PhysicalControl::DpadLeft => "D-pad left",
        PhysicalControl::DpadRight => "D-pad right",
        PhysicalControl::HardwareProfile => "Hardware Profile (read-only)",
        _ => return format!("{control:?}"),
    }
    .into()
}

#[derive(Clone, Debug, PartialEq)]
pub struct BindingDraft {
    pub kind: BindingKind,
    pub gamepad: GamepadControl,
    pub chord: String,
    pub mouse: i32,
    pub reference: String,
}
impl BindingDraft {
    pub fn load(profile: &RemapProfile, layer: usize, control: PhysicalControl) -> Self {
        let fallback = Binding {
            kind: if layer == 0 {
                BindingKind::Passthrough
            } else {
                BindingKind::Transparent
            },
            ..Default::default()
        };
        let binding = profile.layers[layer]
            .bindings
            .get(&control)
            .unwrap_or(&fallback);
        Self {
            kind: binding.kind,
            gamepad: GamepadControl::ALL
                .iter()
                .copied()
                .find(|c| *c as i32 == binding.code)
                .unwrap_or(GamepadControl::A),
            chord: format_chord(&binding.keys),
            mouse: if (272..=279).contains(&binding.code) {
                binding.code
            } else {
                272
            },
            reference: binding.reference.clone(),
        }
    }
    pub fn binding(&self) -> Result<Binding> {
        Ok(Binding {
            kind: self.kind,
            code: match self.kind {
                BindingKind::Gamepad => self.gamepad as i32,
                BindingKind::Mouse => self.mouse,
                _ => 0,
            },
            keys: if self.kind == BindingKind::Keys {
                parse_chord(&self.chord)?
            } else {
                vec![]
            },
            reference: if matches!(
                self.kind,
                BindingKind::Macro | BindingKind::MomentaryLayer | BindingKind::ToggleLayer
            ) {
                self.reference.clone()
            } else {
                String::new()
            },
        })
    }
}

pub fn require_clean(dirty: bool) -> Result<()> {
    ensure!(
        !dirty,
        "Save or discard form fields before navigating. Invalid fields have been retained."
    );
    Ok(())
}

pub fn can_load_initial(chosen: bool, fields_dirty: bool, macro_open: bool, busy: bool) -> bool {
    !chosen && !fields_dirty && !macro_open && !busy
}

pub fn take_initial_profile(
    pending: &mut Option<RemapProfile>,
    chosen: bool,
    fields_dirty: bool,
    macro_open: bool,
    busy: bool,
) -> Option<RemapProfile> {
    if busy {
        return None;
    }
    pending
        .take()
        .filter(|_| can_load_initial(chosen, fields_dirty, macro_open, busy))
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepDraft {
    pub kind: MacroStepKind,
    pub code: String,
    pub delay: String,
}
impl StepDraft {
    pub fn load(step: &MacroStep) -> Self {
        let code = match step.kind {
            MacroStepKind::PressKey | MacroStepKind::ReleaseKey => format_chord(&[step.code]),
            MacroStepKind::PressGamepad | MacroStepKind::ReleaseGamepad => GamepadControl::ALL
                .iter()
                .find(|c| **c as i32 == step.code)
                .map(|c| format!("{c:?}"))
                .unwrap_or_else(|| step.code.to_string()),
            _ => step.code.to_string(),
        };
        Self {
            kind: step.kind,
            code,
            delay: step.delay_ms.to_string(),
        }
    }
    pub fn step(&self) -> Result<MacroStep> {
        let mut step = MacroStep {
            kind: self.kind,
            code: 0,
            delay_ms: 0,
        };
        match self.kind {
            MacroStepKind::Delay => {
                step.delay_ms = self.delay.trim().parse()?;
                ensure!(
                    (0..=60000).contains(&step.delay_ms),
                    "Delay must be 0–60000 ms."
                );
            }
            MacroStepKind::PressKey | MacroStepKind::ReleaseKey => {
                let keys = parse_chord(&self.code)?;
                ensure!(
                    keys.len() == 1,
                    "A macro key step requires exactly one Linux key."
                );
                step.code = keys[0];
            }
            MacroStepKind::PressMouse | MacroStepKind::ReleaseMouse => {
                step.code = self.code.trim().parse()?;
                ensure!(
                    (272..=279).contains(&step.code),
                    "Mouse code must be 272–279."
                );
            }
            MacroStepKind::PressGamepad | MacroStepKind::ReleaseGamepad => {
                step.code = GamepadControl::ALL
                    .iter()
                    .find(|c| {
                        format!("{c:?}").eq_ignore_ascii_case(self.code.trim())
                            || (**c as i32).to_string() == self.code.trim()
                    })
                    .ok_or_else(|| anyhow::anyhow!("Use a gamepad name or code 0–20."))?
                    .to_owned() as i32;
            }
        }
        Ok(step)
    }
}

#[derive(Debug, PartialEq)]
pub struct LaunchPlan {
    pub executable: PathBuf,
    pub args: Vec<String>,
}
pub fn launch_plan(path: &Path) -> Result<LaunchPlan> {
    ensure!(!path.as_os_str().is_empty(), "Bridge path is empty.");
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
    {
        bail!("The native editor requires a native bridge executable, not a .NET DLL.");
    }
    Ok(LaunchPlan {
        executable: path.into(),
        args: vec!["--daemon".into()],
    })
}

pub fn start_once(
    mut probe: impl FnMut() -> Result<ControlResponse>,
    launch: impl FnOnce() -> Result<()>,
) -> Result<bool> {
    match probe() {
        Ok(response) => {
            ensure!(
                response.success,
                "Bridge rejected probe: {}",
                response.error.unwrap_or_default()
            );
            ensure!(
                response.state.is_some(),
                "Bridge returned no state; refusing duplicate launch."
            );
            Ok(false)
        }
        Err(_) => {
            launch()?;
            Ok(true)
        }
    }
}
