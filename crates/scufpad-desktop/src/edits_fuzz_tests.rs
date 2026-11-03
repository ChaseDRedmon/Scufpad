use super::*;

struct Random(u64);
impl Random {
    fn index(&mut self, length: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as usize % length
    }
}

fn workspace() -> RemapProfile {
    let mut profile = RemapProfile {
        macros: (0..4)
            .map(|index| MacroDefinition {
                id: format!("Macro{index}"),
                ..Default::default()
            })
            .collect(),
        layers: (0..4)
            .map(|index| LayerDefinition {
                id: if index == 0 {
                    "Base".into()
                } else {
                    format!("Layer{index}")
                },
                bindings: [(
                    PhysicalControl::A,
                    Binding {
                        kind: BindingKind::Macro,
                        reference: format!("Macro{index}"),
                        ..Default::default()
                    },
                )]
                .into(),
            })
            .collect(),
        ..Default::default()
    };
    for index in 0..3 {
        let reference = profile.layers[index + 1].id.clone();
        profile.layers[index].bindings.insert(
            PhysicalControl::G1,
            Binding {
                kind: BindingKind::MomentaryLayer,
                reference,
                ..Default::default()
            },
        );
    }
    checked(profile).unwrap()
}

#[test]
fn invalid_edit_indices_return_errors_without_changing_workspace() {
    for profile in [RemapProfile::default(), workspace()] {
        let before = profile.clone();
        for index in [profile.layers.len(), usize::MAX] {
            assert!(
                rename_layer(&profile, index, "Renamed").is_err(),
                "layer index {index}"
            );
            assert!(set_binding(&profile, index, PhysicalControl::A, None).is_err());
        }
        assert_eq!(profile, before);
    }
}

#[test]
fn invalid_macro_indices_return_errors_without_changing_workspace() {
    for profile in [RemapProfile::default(), workspace()] {
        let before = profile.clone();
        for index in [profile.macros.len(), usize::MAX] {
            assert!(
                save_macro(&profile, Some(index), MacroDefinition::default()).is_err(),
                "macro index {index}"
            );
        }
        assert_eq!(profile, before);
    }
}

#[test]
fn seeded_mixed_edits_preserve_validity_and_all_rename_references() {
    let mut referenced_renames = [0; 2];
    for seed in 1..=32 {
        let mut random = Random(seed);
        let mut profile = workspace();
        for step in 0..256 {
            let before = profile.clone();
            let mut renamed = None;
            let mut must_succeed = false;
            let result = match random.index(8) {
                0 => {
                    let index = random.index(profile.layers.len() + 2);
                    let name = if random.index(4) == 0 {
                        "😀".repeat(33)
                    } else {
                        format!("層-{seed}-{step}")
                    };
                    if let Some(layer) = profile.layers.get(index) {
                        must_succeed = name.encode_utf16().count() <= 64;
                        renamed = Some((layer.id.clone(), name.clone(), false));
                    }
                    rename_layer(&profile, index, &name)
                }
                1 => {
                    let index = random.index(profile.macros.len() + 2);
                    let name = format!("操作-{seed}-{step}");
                    if let Some(definition) = profile.macros.get(index) {
                        must_succeed = true;
                        renamed = Some((definition.id.clone(), name.clone(), true));
                    }
                    save_macro(
                        &profile,
                        Some(index),
                        MacroDefinition {
                            id: name,
                            ..Default::default()
                        },
                    )
                }
                2 => {
                    let control = PhysicalControl::ALL[random.index(PhysicalControl::ALL.len())];
                    let kind = KINDS[random.index(KINDS.len())];
                    let reference = match kind {
                        BindingKind::Macro => profile
                            .macros
                            .get(random.index(profile.macros.len() + 1))
                            .map(|item| item.id.clone())
                            .unwrap_or_else(|| "missing".into()),
                        BindingKind::MomentaryLayer | BindingKind::ToggleLayer => profile.layers
                            [random.index(profile.layers.len())]
                        .id
                        .clone(),
                        _ => String::new(),
                    };
                    let binding = Binding {
                        kind,
                        reference,
                        code: random.index(282) as i32 - 1,
                        keys: if kind == BindingKind::Keys {
                            vec![random.index(257) as i32]
                        } else {
                            vec![]
                        },
                    };
                    set_binding(
                        &profile,
                        random.index(profile.layers.len() + 1),
                        control,
                        Some(binding),
                    )
                }
                3 => save_macro(
                    &profile,
                    None,
                    MacroDefinition {
                        id: format!("Added-{seed}-{step}"),
                        ..Default::default()
                    },
                ),
                4 if profile.layers.len() > 1 => {
                    let mut candidate = profile.clone();
                    candidate
                        .layers
                        .remove(1 + random.index(profile.layers.len() - 1));
                    checked(candidate)
                }
                5 if !profile.macros.is_empty() => {
                    let mut candidate = profile.clone();
                    candidate.macros.remove(random.index(profile.macros.len()));
                    checked(candidate)
                }
                _ => set_binding(
                    &profile,
                    random.index(profile.layers.len()),
                    PhysicalControl::ALL[random.index(PhysicalControl::ALL.len())],
                    None,
                ),
            };
            assert_eq!(profile, before, "seed={seed} step={step}: input changed");
            if must_succeed {
                assert!(
                    result.is_ok(),
                    "valid rename rejected: seed={seed} step={step}: {result:?}"
                );
            }
            if let Ok(next) = result {
                validate_profile(&next)
                    .unwrap_or_else(|error| panic!("seed={seed} step={step}: {error}"));
                if let Some((old, new, is_macro)) = renamed {
                    for (index, layer) in profile.layers.iter().enumerate() {
                        for (control, binding) in &layer.bindings {
                            let affected = if is_macro {
                                binding.kind == BindingKind::Macro
                            } else {
                                matches!(
                                    binding.kind,
                                    BindingKind::MomentaryLayer | BindingKind::ToggleLayer
                                )
                            };
                            if affected && binding.reference == old {
                                referenced_renames[usize::from(is_macro)] += 1;
                                assert_eq!(
                                    next.layers[index].bindings[control].reference, new,
                                    "seed={seed} step={step}: missed reference"
                                );
                            }
                        }
                    }
                    if !is_macro && old == profile.base_layer {
                        assert_eq!(next.base_layer, new);
                    }
                }
                profile = next;
            }
        }
    }
    assert!(
        referenced_renames.iter().all(|&count| count > 0),
        "missing rename witnesses: {referenced_renames:?}"
    );
}

#[test]
fn utf16_boundaries_and_canonical_drafts_roundtrip() {
    let profile = RemapProfile::default();
    for name in ["a".repeat(64), "😀".repeat(32)] {
        assert!(rename_layer(&profile, 0, &name).is_ok());
        assert!(
            save_macro(
                &profile,
                None,
                MacroDefinition {
                    id: name,
                    ..Default::default()
                }
            )
            .is_ok()
        );
    }
    for name in ["a".repeat(65), "😀".repeat(33), " \t\n".into()] {
        assert!(rename_layer(&profile, 0, &name).is_err());
        assert!(
            save_macro(
                &profile,
                None,
                MacroDefinition {
                    id: name,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    let mut random = Random(0x51eed);
    for case in 0..2048 {
        let mut profile = workspace();
        let kind = KINDS[random.index(KINDS.len())];
        let binding = Binding {
            kind,
            code: match kind {
                BindingKind::Gamepad => random.index(21) as i32,
                BindingKind::Mouse => 272 + random.index(8) as i32,
                _ => 0,
            },
            keys: if kind == BindingKind::Keys {
                vec![1 + random.index(255) as i32]
            } else {
                vec![]
            },
            reference: match kind {
                BindingKind::Macro => "Macro0".into(),
                BindingKind::MomentaryLayer | BindingKind::ToggleLayer => "Layer2".into(),
                _ => String::new(),
            },
        };
        profile.layers[1]
            .bindings
            .insert(PhysicalControl::G2, binding.clone());
        assert_eq!(
            BindingDraft::load(&profile, 1, PhysicalControl::G2)
                .binding()
                .unwrap(),
            binding,
            "case={case}"
        );
        let kind = STEP_KINDS[random.index(STEP_KINDS.len())];
        let step = MacroStep {
            kind,
            code: match kind {
                MacroStepKind::PressKey | MacroStepKind::ReleaseKey => 1 + random.index(255) as i32,
                MacroStepKind::PressMouse | MacroStepKind::ReleaseMouse => {
                    272 + random.index(8) as i32
                }
                MacroStepKind::PressGamepad | MacroStepKind::ReleaseGamepad => {
                    random.index(21) as i32
                }
                MacroStepKind::Delay => 0,
            },
            delay_ms: if kind == MacroStepKind::Delay {
                random.index(60001) as i32
            } else {
                0
            },
        };
        assert_eq!(StepDraft::load(&step).step().unwrap(), step, "case={case}");
    }
}

fn maximum_workspace(steps: usize) -> RemapProfile {
    RemapProfile {
        macros: (0..128)
            .map(|index| MacroDefinition {
                id: format!("Macro{index}"),
                steps: vec![
                    MacroStep {
                        kind: MacroStepKind::Delay,
                        delay_ms: 1,
                        code: 0
                    };
                    steps
                ],
                ..Default::default()
            })
            .collect(),
        layers: (0..32)
            .map(|index| LayerDefinition {
                id: if index == 0 {
                    "Base".into()
                } else {
                    format!("Layer{index}")
                },
                bindings: PhysicalControl::ALL
                    .iter()
                    .filter(|&&control| control != PhysicalControl::HardwareProfile)
                    .map(|&control| {
                        (
                            control,
                            Binding {
                                kind: BindingKind::Macro,
                                reference: format!("Macro{index}"),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn oversize_editor_workspace_is_rejected_before_apply_or_save() {
    let profile = maximum_workspace(256);
    validate_profile(&profile).unwrap();
    assert!(
        write_message(
            &mut Vec::new(),
            &ControlRequest {
                command: ControlCommand::ApplyProfile,
                profile: Some(profile.clone())
            }
        )
        .is_err()
    );
    assert!(
        checked(profile).is_err(),
        "editor accepted a profile the bridge cannot receive"
    );
    let profile = checked(maximum_workspace(128)).unwrap();
    write_message(
        &mut Vec::new(),
        &ControlRequest {
            command: ControlCommand::ApplyProfile,
            profile: Some(profile.clone()),
        },
    )
    .unwrap();
    write_message(
        &mut Vec::new(),
        &ControlResponse {
            success: true,
            profile: Some(profile),
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
#[ignore = "opt-in large deliverable profile edit stress; run in Release with --nocapture"]
fn large_profile_edit_stress() {
    use std::time::Instant;
    let profile = checked(maximum_workspace(128)).unwrap();
    validate_profile(&profile).unwrap();
    let transport_accepts_profile = write_message(
        &mut Vec::new(),
        &ControlRequest {
            command: ControlCommand::ApplyProfile,
            profile: Some(profile.clone()),
        },
    )
    .is_ok();
    for sample in 0..7 {
        let start = Instant::now();
        for index in 0..16 {
            let renamed = rename_layer(&profile, 31, &format!("Stress{index}")).unwrap();
            let edited =
                set_binding(&renamed, 31, PhysicalControl::G1, Some(Binding::default())).unwrap();
            validate_profile(&edited).unwrap();
            assert_eq!(edited.layers[31].id, format!("Stress{index}"));
            assert_eq!(
                edited.layers[31].bindings[&PhysicalControl::G1],
                Binding::default()
            );
        }
        eprintln!(
            "large_profile_edit_stress sample={sample} iterations=16 layers=32 macros=128 steps_per_macro=128 transport_accepts_profile={transport_accepts_profile} ns_per_rename_and_edit={}",
            start.elapsed().as_nanos() / 16
        );
    }
}
