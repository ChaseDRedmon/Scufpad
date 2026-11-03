use crate::edits::*;
use anyhow::{Result, ensure};
use scufpad_core::*;
use std::{cell::Cell, path::Path};

pub fn run() -> Result<()> {
    edits_and_references()?;
    validation_retains_candidates()?;
    drafts_and_steps()?;
    crate::app::check_macro_drafts()?;
    setup_order()?;
    launcher_policy()?;
    Ok(())
}

fn edits_and_references() -> Result<()> {
    let original = RemapProfile::default();
    validate_profile(&original)?;
    ensure!(
        PhysicalControl::ALL.len() == 29,
        "Every physical control must be represented."
    );
    let edited = set_binding(
        &original,
        0,
        PhysicalControl::SaxLeft,
        Some(Binding {
            kind: BindingKind::Keys,
            keys: parse_chord("CTRL+C")?,
            ..Default::default()
        }),
    )?;
    ensure!(
        original.layers[0].bindings.is_empty(),
        "Binding edits mutated the original workspace."
    );
    ensure!(
        edited.layers[0].bindings[&PhysicalControl::SaxLeft].keys == [29, 46],
        "Linux keys were not preserved."
    );
    ensure!(
        set_binding(&edited, 0, PhysicalControl::SaxLeft, None)?.layers[0]
            .bindings
            .is_empty(),
        "Remove override did not restore defaults."
    );
    let mut layered = original.clone();
    layered.layers.push(LayerDefinition {
        id: "Shift".into(),
        ..Default::default()
    });
    layered = set_binding(
        &layered,
        0,
        PhysicalControl::G1,
        Some(Binding {
            kind: BindingKind::MomentaryLayer,
            reference: "Shift".into(),
            ..Default::default()
        }),
    )?;
    layered = set_binding(
        &layered,
        0,
        PhysicalControl::G2,
        Some(Binding {
            kind: BindingKind::ToggleLayer,
            reference: "Shift".into(),
            ..Default::default()
        }),
    )?;
    let renamed = rename_layer(&layered, 1, "Combat")?;
    for control in [PhysicalControl::G1, PhysicalControl::G2] {
        ensure!(
            renamed.layers[0].bindings[&control].reference == "Combat",
            "Layer rename missed a reference."
        );
        ensure!(
            layered.layers[0].bindings[&control].reference == "Shift",
            "Layer rename changed the original."
        );
    }
    let base = rename_layer(&renamed, 0, "Primary")?;
    ensure!(
        base.base_layer == "Primary" && base.layers[0].id == "Primary",
        "Base ID did not follow rename."
    );
    ensure!(
        rename_layer(&layered, 1, "Base").is_err(),
        "Duplicate layer ID accepted."
    );
    let mut removed = renamed.clone();
    removed.layers.remove(1);
    ensure!(
        checked(removed).is_err(),
        "Referenced layer deletion accepted."
    );
    let macro_definition = MacroDefinition {
        id: "Copy".into(),
        repeat_while_held: false,
        steps: vec![
            MacroStep {
                kind: MacroStepKind::PressKey,
                code: 46,
                delay_ms: 0,
            },
            MacroStep {
                kind: MacroStepKind::Delay,
                code: 0,
                delay_ms: 20,
            },
            MacroStep {
                kind: MacroStepKind::ReleaseKey,
                code: 46,
                delay_ms: 0,
            },
        ],
    };
    let macros = save_macro(&original, None, macro_definition.clone())?;
    let macros = set_binding(
        &macros,
        0,
        PhysicalControl::G3,
        Some(Binding {
            kind: BindingKind::Macro,
            reference: "Copy".into(),
            ..Default::default()
        }),
    )?;
    let mut definition = macro_definition;
    definition.id = "Copy again".into();
    let renamed = save_macro(&macros, Some(0), definition)?;
    ensure!(
        renamed.layers[0].bindings[&PhysicalControl::G3].reference == "Copy again",
        "Macro rename missed its reference."
    );
    ensure!(
        macros.macros[0].id == "Copy",
        "Macro rename changed its original."
    );
    let mut removed = renamed;
    removed.macros.clear();
    ensure!(
        checked(removed).is_err(),
        "Referenced macro deletion accepted."
    );
    ensure!(
        unique_id("Layer", ["Layer 1", "Layer 3"].into_iter()) == "Layer 2",
        "Unique ID collided."
    );
    Ok(())
}

fn validation_retains_candidates() -> Result<()> {
    let original = RemapProfile::default();
    for (control, binding) in [
        (PhysicalControl::HardwareProfile, Binding::default()),
        (
            PhysicalControl::A,
            Binding {
                kind: BindingKind::Transparent,
                ..Default::default()
            },
        ),
        (
            PhysicalControl::A,
            Binding {
                kind: BindingKind::Gamepad,
                code: 21,
                ..Default::default()
            },
        ),
        (
            PhysicalControl::A,
            Binding {
                kind: BindingKind::Mouse,
                code: 1,
                ..Default::default()
            },
        ),
    ] {
        ensure!(
            set_binding(&original, 0, control, Some(binding)).is_err(),
            "Invalid binding was accepted."
        );
        ensure!(
            original.layers[0].bindings.is_empty(),
            "Rejected edit mutated the workspace."
        );
    }
    let valid = MacroDefinition {
        id: "Finite".into(),
        repeat_while_held: true,
        steps: vec![MacroStep {
            kind: MacroStepKind::Delay,
            code: 0,
            delay_ms: 1,
        }],
    };
    ensure!(
        save_macro(&original, None, valid.clone()).is_ok(),
        "Positive-delay repeat rejected."
    );
    let mut zero = valid.clone();
    zero.steps[0].delay_ms = 0;
    ensure!(
        save_macro(&original, None, zero).is_err(),
        "Zero-delay repeat accepted."
    );
    let mut oversized = valid.clone();
    oversized.steps = vec![valid.steps[0].clone(); 257];
    ensure!(
        save_macro(&original, None, oversized).is_err(),
        "257-step macro accepted."
    );
    let mut duration = valid;
    duration.steps = vec![
        MacroStep {
            kind: MacroStepKind::Delay,
            code: 0,
            delay_ms: 60000,
        },
        MacroStep {
            kind: MacroStepKind::Delay,
            code: 0,
            delay_ms: 1,
        },
    ];
    ensure!(
        save_macro(&original, None, duration).is_err(),
        "Macro exceeding 60 seconds accepted."
    );
    Ok(())
}

fn drafts_and_steps() -> Result<()> {
    let remote = RemapProfile {
        name: "Initial bridge profile".into(),
        ..Default::default()
    };
    let mut pending = Some(remote.clone());
    ensure!(
        take_initial_profile(&mut pending, false, false, false, true).is_none()
            && pending.as_ref() == Some(&remote),
        "A busy library operation dropped the initial profile."
    );
    ensure!(
        take_initial_profile(&mut pending, false, false, false, false) == Some(remote.clone())
            && pending.is_none(),
        "The deferred profile was not delivered after library completion."
    );
    for (chosen, dirty, macro_open) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let mut pending = Some(remote.clone());
        ensure!(
            take_initial_profile(&mut pending, chosen, dirty, macro_open, true).is_none()
                && pending.is_some(),
            "Busy state did not defer profile delivery."
        );
        ensure!(
            take_initial_profile(&mut pending, chosen, dirty, macro_open, false).is_none()
                && pending.is_none(),
            "Deferred delivery overwrote user work."
        );
    }
    ensure!(
        can_load_initial(false, false, false, false),
        "Untouched workspace did not accept initial profile."
    );
    for (chosen, dirty, macro_open, busy) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        ensure!(
            !can_load_initial(chosen, dirty, macro_open, busy),
            "Initial profile clobbered an edited or loading workspace."
        );
    }
    let original = RemapProfile::default();
    let baseline = BindingDraft::load(&original, 0, PhysicalControl::A);
    ensure!(
        require_clean(baseline != baseline.clone()).is_ok(),
        "Matching notifications dirtied the form."
    );
    let mut draft = baseline.clone();
    draft.kind = BindingKind::Keys;
    draft.chord = "CTRL+".into();
    ensure!(
        require_clean(draft != baseline).is_err(),
        "Invalid draft did not block navigation."
    );
    ensure!(
        draft.binding().is_err() && draft.chord == "CTRL+",
        "Validation dropped invalid text."
    );
    draft.chord = "CTRL+C".into();
    ensure!(draft.binding()?.keys == [29, 46], "Valid chord failed.");
    ensure!(
        require_clean(draft != baseline).is_err(),
        "Valid uncommitted draft did not block navigation."
    );
    for code in 1..=255 {
        ensure!(
            parse_chord(&format_chord(&[code]))? == [code],
            "Key {code} did not round-trip."
        );
    }
    let mut step = StepDraft {
        kind: MacroStepKind::PressKey,
        code: "C".into(),
        delay: "invalid-unused".into(),
    };
    ensure!(
        step.step()?.code == 46,
        "Macro key must use Linux evdev code."
    );
    step.code = "CTRL+C".into();
    ensure!(
        step.step().is_err(),
        "Chord accepted as a single macro key."
    );
    step.kind = MacroStepKind::PressGamepad;
    step.code = "Paddle4".into();
    ensure!(
        step.step()?.code == GamepadControl::Paddle4 as i32,
        "Friendly gamepad name failed."
    );
    step.code = "21".into();
    ensure!(step.step().is_err(), "Invalid gamepad code accepted.");
    step.kind = MacroStepKind::PressMouse;
    step.code = "272".into();
    ensure!(step.step()?.code == 272, "Mouse code failed.");
    step.code = "271".into();
    ensure!(step.step().is_err(), "Invalid mouse code accepted.");
    step.kind = MacroStepKind::Delay;
    for invalid in ["invalid", "-1", "60001"] {
        step.delay = invalid.into();
        ensure!(step.step().is_err(), "Invalid delay accepted.");
    }
    step.delay = "60000".into();
    let parsed = step.step()?;
    ensure!(
        parsed.delay_ms == 60000 && parsed.code == 0,
        "Delay step did not normalize unused code."
    );
    let loaded = StepDraft::load(&parsed);
    ensure!(
        loaded == loaded.clone(),
        "Reloaded step invented a dirty draft."
    );
    Ok(())
}

fn setup_order() -> Result<()> {
    let mut state = BridgeState {
        connected: true,
        ..Default::default()
    };
    for control in EXTRAS {
        ensure!(setup_next(&state) == Some(control), "Setup order changed.");
        state.verified_extra_controls |= control.bit();
    }
    ensure!(
        setup_next(&state).is_none(),
        "Complete setup still requested a step."
    );
    state.connected = false;
    ensure!(
        setup_next(&state).is_none(),
        "Disconnected controller reused coverage."
    );
    state.connected = true;
    state.verified_extra_controls = PhysicalControl::SaxRight.bit();
    ensure!(
        setup_next(&state) == Some(PhysicalControl::RearOuterLeft),
        "Out-of-order coverage skipped a step."
    );
    Ok(())
}

fn launcher_policy() -> Result<()> {
    ensure!(
        crate::worker::stale_arm(ControlCommand::Arm, 0, 1),
        "Stop All did not invalidate queued Enable."
    );
    ensure!(
        !crate::worker::stale_arm(ControlCommand::Arm, 1, 1),
        "An explicit Enable after Stop All was rejected."
    );
    ensure!(
        !crate::worker::stale_arm(ControlCommand::Shutdown, 0, 1),
        "Stop All canceled a shutdown request."
    );
    let path = Path::new("/a bridge folder ; $(not-a-command)/scufpad");
    let plan = launch_plan(path)?;
    ensure!(
        plan.executable == path && plan.args == ["--daemon"],
        "Launch plan interpreted or split the executable path."
    );
    ensure!(
        launch_plan(Path::new("Scufpad.dll")).is_err(),
        "Managed fallback accepted."
    );
    for connected in [false, true] {
        let launches = Cell::new(0);
        let launched = start_once(
            || {
                Ok(ControlResponse {
                    success: true,
                    state: Some(BridgeState {
                        connected,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            },
            || {
                launches.set(launches.get() + 1);
                Ok(())
            },
        )?;
        ensure!(
            !launched && launches.get() == 0,
            "A responsive bridge caused a duplicate process."
        );
    }
    let launches = Cell::new(0);
    let probes = Cell::new(0);
    ensure!(
        start_once(
            || {
                probes.set(probes.get() + 1);
                anyhow::bail!("fake unavailable transport")
            },
            || {
                launches.set(launches.get() + 1);
                Ok(())
            }
        )?,
        "Missing bridge was not launched."
    );
    ensure!(
        probes.get() == 1 && launches.get() == 1,
        "Missing bridge retried / launched more than once."
    );
    for response in [
        ControlResponse {
            success: false,
            error: Some("Rejected".into()),
            ..Default::default()
        },
        ControlResponse {
            success: true,
            ..Default::default()
        },
    ] {
        let launched = Cell::new(false);
        ensure!(
            start_once(
                || Ok(response.clone()),
                || {
                    launched.set(true);
                    Ok(())
                }
            )
            .is_err(),
            "Malformed/rejected probe accepted."
        );
        ensure!(
            !launched.get(),
            "Malformed/rejected probe launched a duplicate daemon."
        );
    }
    ensure!(
        start_once(
            || anyhow::bail!("fake timeout"),
            || anyhow::bail!("fake launch failure")
        )
        .is_err(),
        "Launch failure was hidden."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_edits_and_reference_updates() {
        edits_and_references().unwrap();
    }
    #[test]
    fn invalid_candidates_retain_workspace() {
        validation_retains_candidates().unwrap();
    }
    #[test]
    fn drafts_and_macro_step_fields_are_protected() {
        drafts_and_steps().unwrap();
        crate::app::check_macro_drafts().unwrap();
    }
    #[test]
    fn one_step_setup_uses_live_bridge_coverage() {
        setup_order().unwrap();
    }
    #[test]
    fn launcher_probes_once_without_real_ipc_or_processes() {
        launcher_policy().unwrap();
    }
}
