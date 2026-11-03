use crate::*;
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.encode_utf16().count() <= 64
}

pub fn validate_profile(profile: &RemapProfile) -> Result<()> {
    ensure!(profile.version == 1, "Profile version must be 1.");
    ensure!(
        valid_id(&profile.name),
        "Profile name must contain 1–64 characters."
    );
    ensure!(
        profile.trigger_release_threshold >= 0
            && profile.trigger_press_threshold <= 1023
            && profile.trigger_release_threshold < profile.trigger_press_threshold,
        "Trigger thresholds must satisfy 0 <= release < press <= 1023."
    );
    ensure!(
        (1..=32).contains(&profile.layers.len()),
        "A profile needs 1–32 layers."
    );
    ensure!(
        profile.macros.len() <= 128,
        "A profile supports at most 128 macros."
    );
    let mut layers = BTreeMap::new();
    for (i, layer) in profile.layers.iter().enumerate() {
        ensure!(
            valid_id(&layer.id),
            "Layer IDs must contain 1–64 characters."
        );
        ensure!(
            layers.insert(layer.id.as_str(), i).is_none(),
            "Duplicate layer ID: {}.",
            layer.id
        );
    }
    ensure!(
        profile.layers[0].id == profile.base_layer,
        "The first layer must be the base layer."
    );
    let mut macros = BTreeSet::new();
    for definition in &profile.macros {
        ensure!(
            valid_id(&definition.id),
            "Macro IDs must contain 1–64 characters."
        );
        ensure!(
            macros.insert(definition.id.as_str()),
            "Duplicate macro ID: {}.",
            definition.id
        );
        ensure!(
            definition.steps.len() <= 256,
            "Macros support at most 256 steps."
        );
        let mut delay = 0u64;
        for step in &definition.steps {
            ensure!(
                (0..=60000).contains(&step.delay_ms),
                "Macro delays must be between 0 and 60000 ms."
            );
            match step.kind {
                MacroStepKind::PressKey | MacroStepKind::ReleaseKey => ensure!(
                    (1..=255).contains(&step.code),
                    "Keyboard codes must be 1–255."
                ),
                MacroStepKind::PressMouse | MacroStepKind::ReleaseMouse => ensure!(
                    (272..=279).contains(&step.code),
                    "Mouse codes must be 272–279."
                ),
                MacroStepKind::PressGamepad | MacroStepKind::ReleaseGamepad => {
                    ensure!((0..=20).contains(&step.code), "Gamepad codes must be 0–20.")
                }
                MacroStepKind::Delay => delay += step.delay_ms as u64,
            }
        }
        ensure!(delay <= 60000, "Macro {} exceeds 60000 ms.", definition.id);
        ensure!(
            !definition.repeat_while_held || delay > 0,
            "Repeating macros need a positive delay."
        );
    }
    let mut edges = vec![Vec::new(); profile.layers.len()];
    for (i, layer) in profile.layers.iter().enumerate() {
        for (control, binding) in &layer.bindings {
            ensure!(
                *control != PhysicalControl::HardwareProfile,
                "Bindings cannot use the hardware profile button."
            );
            ensure!(binding.keys.len() <= 16, "Chords support at most 16 keys.");
            let mut keys = BTreeSet::new();
            for key in &binding.keys {
                ensure!(
                    (1..=255).contains(key) && keys.insert(key),
                    "Chord keys must be unique codes from 1–255."
                );
            }
            match binding.kind {
                BindingKind::Transparent => ensure!(
                    i != 0,
                    "The base layer cannot contain transparent bindings."
                ),
                BindingKind::Keys => ensure!(
                    !binding.keys.is_empty(),
                    "A key binding needs at least one key."
                ),
                BindingKind::Gamepad => ensure!(
                    (0..=20).contains(&binding.code),
                    "Gamepad codes must be 0–20."
                ),
                BindingKind::Mouse => ensure!(
                    (272..=279).contains(&binding.code),
                    "Mouse codes must be 272–279."
                ),
                BindingKind::Macro => ensure!(
                    macros.contains(binding.reference.as_str()),
                    "Unknown macro: {}.",
                    binding.reference
                ),
                BindingKind::MomentaryLayer | BindingKind::ToggleLayer => {
                    let target = *layers
                        .get(binding.reference.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Unknown layer: {}.", binding.reference))?;
                    ensure!(target != 0, "Layer actions cannot target the base layer.");
                    edges[i].push(target);
                }
                _ => {}
            }
        }
    }
    fn visit(layer: usize, edges: &[Vec<usize>], visited: &mut [u8]) -> Result<()> {
        ensure!(
            visited[layer] != 1,
            "Layer references cannot contain cycles."
        );
        if visited[layer] == 2 {
            return Ok(());
        }
        visited[layer] = 1;
        for &target in &edges[layer] {
            visit(target, edges, visited)?;
        }
        visited[layer] = 2;
        Ok(())
    }
    let mut visited = vec![0; edges.len()];
    for i in 0..edges.len() {
        visit(i, &edges, &mut visited)?;
    }
    Ok(())
}
