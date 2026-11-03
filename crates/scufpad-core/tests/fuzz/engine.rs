use super::*;

// Fixed-seed randomized property tests, not coverage-guided/libFuzzer targets.
struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    fn index(&mut self, length: usize) -> usize {
        self.next() as usize % length
    }
}

fn passthrough(control: PhysicalControl) -> Option<GamepadControl> {
    use GamepadControl as G;
    use PhysicalControl as P;
    Some(match control {
        P::A => G::A,
        P::B => G::B,
        P::X => G::X,
        P::Y => G::Y,
        P::LeftBumper => G::LeftBumper,
        P::RightBumper => G::RightBumper,
        P::View => G::View,
        P::Menu => G::Menu,
        P::Guide => G::Guide,
        P::LeftStickClick => G::LeftStickClick,
        P::RightStickClick => G::RightStickClick,
        P::RearOuterLeft => G::Paddle1,
        P::RearInnerLeft => G::Paddle2,
        P::RearInnerRight => G::Paddle3,
        P::RearOuterRight => G::Paddle4,
        P::DpadUp => G::DpadUp,
        P::DpadRight => G::DpadRight,
        P::DpadDown => G::DpadDown,
        P::DpadLeft => G::DpadLeft,
        P::LeftTrigger => G::LeftTrigger,
        P::RightTrigger => G::RightTrigger,
        _ => return None,
    })
}

fn random_profile(random: &mut Random) -> RemapProfile {
    let mut profile = RemapProfile::default();
    profile.trigger_press_threshold = 1 + random.index(1023) as i32;
    profile.trigger_release_threshold =
        random.index(profile.trigger_press_threshold as usize) as i32;
    for &control in PhysicalControl::ALL {
        if control == PhysicalControl::HardwareProfile {
            continue;
        }
        let binding = match random.index(5) {
            0 => Binding::default(),
            1 => Binding {
                kind: BindingKind::Disabled,
                ..Default::default()
            },
            2 => Binding {
                kind: BindingKind::Gamepad,
                code: random.index(21) as i32,
                ..Default::default()
            },
            3 => {
                let first = random.index(255);
                let chord: Vec<_> = (0..1 + random.index(16))
                    .map(|offset| (1 + (first + offset) % 255) as i32)
                    .collect();
                keys(&chord)
            }
            _ => Binding {
                kind: BindingKind::Mouse,
                code: 272 + random.index(8) as i32,
                ..Default::default()
            },
        };
        profile.layers[0].bindings.insert(control, binding);
    }
    profile
}

// For one layer without macros, held output is simply the union of each eligible
// source's binding, independent of the engine's owner counters and press/release loops.
fn reference(
    profile: &RemapProfile,
    input: PhysicalState,
    current: u64,
    suppressed: u64,
) -> OutputState {
    let mut output = OutputState {
        left_x: input.left_x,
        left_y: input.left_y,
        right_x: input.right_x,
        right_y: input.right_y,
        ..Default::default()
    };
    let mut digital_triggers = [false; 2];
    for (&control, binding) in &profile.layers[0].bindings {
        if current & control.bit() == 0 {
            continue;
        }
        match binding.kind {
            BindingKind::Passthrough => {
                if let Some(gamepad) = passthrough(control) {
                    output.gamepad_buttons |= gamepad.bit();
                }
            }
            BindingKind::Gamepad => {
                output.gamepad_buttons |= 1 << binding.code;
                if binding.code >= 19 {
                    digital_triggers[(binding.code - 19) as usize] = true;
                }
            }
            BindingKind::Keys => {
                for &key in &binding.keys {
                    output.keyboard[key as usize] = true;
                }
            }
            BindingKind::Mouse => output.mouse[(binding.code - 272) as usize] = true,
            BindingKind::Disabled => {}
            _ => unreachable!("oracle only accepts direct bindings"),
        }
    }
    for (index, (control, analog)) in [
        (PhysicalControl::LeftTrigger, input.left_trigger),
        (PhysicalControl::RightTrigger, input.right_trigger),
    ]
    .into_iter()
    .enumerate()
    {
        let native = profile.layers[0].bindings[&control].kind == BindingKind::Passthrough
            && suppressed & control.bit() == 0;
        let value = if digital_triggers[index] {
            1023
        } else if native {
            analog
        } else {
            0
        };
        if index == 0 {
            output.left_trigger = value;
        } else {
            output.right_trigger = value;
        }
    }
    output
}

fn tick_without_allocation(engine: &mut MappingEngine, input: PhysicalState, now: u64) {
    ALLOCATIONS.set(0);
    TRACK_ALLOCATIONS.set(true);
    let result = engine.tick(input, now);
    TRACK_ALLOCATIONS.set(false);
    result.unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get), 0, "tick allocated at {now}");
}

fn mapping_campaign(cases: u64, frames: u64) {
    let mask = PhysicalControl::ALL
        .iter()
        .fold(0, |bits, control| bits | control.bit());
    for seed in 0..cases {
        let mut random = Random(seed);
        let mut profile = random_profile(&mut random);
        let mut engine = MappingEngine::new(profile.clone()).unwrap();
        let (mut suppressed, mut previous_physical) = (0, 0);
        let mut triggers = [false; 2];
        for frame in 0..frames {
            if frame % 97 == 96 {
                engine.cancel_all();
                suppressed |= previous_physical;
                assert_eq!(engine.output, OutputState::default());
            }
            if frame % 257 == 256 {
                profile = random_profile(&mut random);
                engine.apply_profile(profile.clone()).unwrap();
                suppressed |= previous_physical;
            }
            let input = PhysicalState {
                buttons: random.next() & mask,
                left_x: random.next() as i16 as i32,
                left_y: random.next() as i16 as i32,
                right_x: random.next() as i16 as i32,
                right_y: random.next() as i16 as i32,
                left_trigger: random.index(1024) as i32,
                right_trigger: random.index(1024) as i32,
            };
            let mut physical = input.buttons & !((1 << 29) | (1 << 30));
            for (index, value) in [input.left_trigger, input.right_trigger]
                .into_iter()
                .enumerate()
            {
                triggers[index] = if triggers[index] {
                    value > profile.trigger_release_threshold
                } else {
                    value >= profile.trigger_press_threshold
                };
                if triggers[index] {
                    physical |= 1 << (29 + index);
                }
            }
            if frame == 0 {
                suppressed |= physical;
            }
            suppressed &= physical;
            let expected = reference(&profile, input, physical & !suppressed, suppressed);
            tick_without_allocation(&mut engine, input, frame);
            assert_eq!(engine.output, expected, "seed={seed} frame={frame}");
            assert_eq!(
                engine.physical_buttons, physical,
                "seed={seed} frame={frame}"
            );
            assert_eq!(engine.active_layers, 1);
            assert_eq!(engine.next_due_ms, None);
            if frame % 127 == 126 {
                let before = engine.output.clone();
                assert!(engine.tick(PhysicalState::default(), frame - 1).is_err());
                assert_eq!(
                    engine.output, before,
                    "backwards clock seed={seed} frame={frame}"
                );
                let mut invalid = profile.clone();
                invalid.trigger_press_threshold = invalid.trigger_release_threshold;
                assert!(engine.apply_profile(invalid).is_err());
                assert_eq!(engine.profile(), &profile);
                assert_eq!(
                    engine.output, before,
                    "invalid apply seed={seed} frame={frame}"
                );
            }
            previous_physical = physical;
        }
        engine.cancel_all();
        assert_eq!(engine.output, OutputState::default());
    }
}

#[test]
fn seeded_mapping_matches_independent_union_model_without_tick_allocations() {
    mapping_campaign(64, 512);
}

#[test]
fn seeded_usb_reports_match_encoded_state_and_reject_reserved_bits() {
    let mut random = Random(0xdec0de);
    let hats = [
        1u64 << 25,
        (1 << 25) | (1 << 26),
        1 << 26,
        (1 << 26) | (1 << 27),
        1 << 27,
        (1 << 27) | (1 << 28),
        1 << 28,
        (1 << 28) | (1 << 25),
        0,
    ];
    let allowed = ((1u32 << 17) - 1) | (0x3f << 19);
    for case in 0..8192 {
        let hat = random.index(9);
        let mut expected = PhysicalState {
            buttons: random.next() & u64::from(allowed),
            left_x: random.next() as i16 as i32,
            left_y: random.next() as i16 as i32,
            right_x: random.next() as i16 as i32,
            right_y: random.next() as i16 as i32,
            left_trigger: random.index(1024) as i32,
            right_trigger: random.index(1024) as i32,
        };
        let mut report = [0u8; 16];
        report[0] = 6;
        for (offset, axis) in [
            (1, expected.left_x),
            (3, expected.left_y),
            (5, expected.right_x),
            (7, expected.right_y),
        ] {
            report[offset..offset + 2].copy_from_slice(&(axis as i16).to_le_bytes());
        }
        report[9] = expected.left_trigger as u8;
        report[10] = ((expected.left_trigger >> 8) | (expected.right_trigger << 2)) as u8;
        report[11] = ((hat as i32) << 4 | (expected.right_trigger >> 6)) as u8;
        report[12..].copy_from_slice(&(expected.buttons as u32).to_le_bytes());
        expected.buttons |= hats[hat];
        assert_eq!(decode_usb_report(&report), Some(expected), "case={case}");
        for bit in [17, 18, 25, 26, 27, 28, 29, 30, 31] {
            let mut invalid = report;
            invalid[12 + bit / 8] |= 1 << (bit % 8);
            assert!(
                decode_usb_report(&invalid).is_none(),
                "case={case} bit={bit}"
            );
        }
        let mut arbitrary = [0u8; 65];
        for byte in &mut arbitrary {
            *byte = random.next() as u8;
        }
        let length = random.index(arbitrary.len() + 1);
        if let Some(decoded) = decode_usb_report(&arbitrary[..length]) {
            assert_eq!(length, 16);
            assert!((0..=1023).contains(&decoded.left_trigger));
            assert!((0..=1023).contains(&decoded.right_trigger));
            assert_eq!(decoded.buttons & !(((1 << 29) - 1) & !(3 << 17)), 0);
        }
    }
}

fn macro_campaign(cases: u64, frames: u64) {
    let mask = PhysicalControl::ALL
        .iter()
        .fold(0, |bits, control| bits | control.bit());
    for seed in 0..cases {
        let mut random = Random(0xface0000 + seed);
        let mut profile = RemapProfile::default();
        for index in 0..8 {
            let mut steps = vec![delay(1)];
            for _ in 1..256 {
                steps.push(match random.index(7) {
                    0 => step(MacroStepKind::PressKey, 1 + random.index(255) as i32),
                    1 => step(MacroStepKind::ReleaseKey, 1 + random.index(255) as i32),
                    2 => step(MacroStepKind::PressMouse, 272 + random.index(8) as i32),
                    3 => step(MacroStepKind::ReleaseMouse, 272 + random.index(8) as i32),
                    4 => step(MacroStepKind::PressGamepad, random.index(21) as i32),
                    5 => step(MacroStepKind::ReleaseGamepad, random.index(21) as i32),
                    _ => delay(random.index(16) as i32),
                });
            }
            profile.macros.push(MacroDefinition {
                id: format!("Macro{index}"),
                repeat_while_held: index % 2 == 0,
                steps,
            });
        }
        profile.layers = (0..4)
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
                        let binding = if index < 3 && random.index(8) == 0 {
                            layer(&format!("Layer{}", index + 1), random.index(2) == 0)
                        } else {
                            Binding {
                                kind: BindingKind::Macro,
                                reference: format!("Macro{}", random.index(8)),
                                ..Default::default()
                            }
                        };
                        (control, binding)
                    })
                    .collect(),
            })
            .collect();
        profile.macros.push(MacroDefinition {
            id: "DeadlineWitness".into(),
            repeat_while_held: seed % 2 == 0,
            steps: vec![step(MacroStepKind::PressKey, 255), delay(60000)],
        });
        for layer in &mut profile.layers {
            layer.bindings.insert(
                PhysicalControl::G5,
                Binding {
                    kind: BindingKind::Macro,
                    reference: "DeadlineWitness".into(),
                    ..Default::default()
                },
            );
        }
        let mut engine = ready(profile);
        tick_without_allocation(&mut engine, state(PhysicalControl::G5.bit()), 1);
        assert!(
            engine.output.keyboard[255],
            "macro never started: seed={seed}"
        );
        assert_eq!(engine.next_due_ms, Some(60001));
        engine.cancel_all();
        assert_eq!(engine.output, OutputState::default());
        assert_eq!(engine.next_due_ms, None);
        let mut now = 1u64;
        for frame in 0..frames {
            now += random.index(20) as u64;
            let input = PhysicalState {
                buttons: random.next() & mask,
                left_trigger: random.index(1024) as i32,
                right_trigger: random.index(1024) as i32,
                ..Default::default()
            };
            tick_without_allocation(&mut engine, input, now);
            assert_eq!(
                engine.output.gamepad_buttons >> 21,
                0,
                "seed={seed} frame={frame}"
            );
            assert!(!engine.output.keyboard[0], "seed={seed} frame={frame}");
            assert_eq!(engine.active_layers >> 4, 0);
            if frame % 113 == 112 {
                engine.cancel_all();
                assert_eq!(engine.output, OutputState::default());
                assert_eq!(engine.next_due_ms, None);
            }
        }
        tick_without_allocation(
            &mut engine,
            PhysicalState::default(),
            now.saturating_add(60001),
        );
        assert_eq!(
            engine.output,
            OutputState::default(),
            "deadline seed={seed}"
        );
        assert_eq!(engine.next_due_ms, None);
        let held = state(PhysicalControl::G5.bit());
        tick_without_allocation(&mut engine, held, u64::MAX - 1);
        assert!(
            engine.output.keyboard[255],
            "saturation witness never started: seed={seed}"
        );
        assert_eq!(engine.next_due_ms, Some(u64::MAX));
        tick_without_allocation(&mut engine, held, u64::MAX);
        assert_eq!(
            engine.output,
            OutputState::default(),
            "saturating deadline seed={seed}"
        );
        assert_eq!(engine.next_due_ms, None);
    }
}

#[test]
fn seeded_layer_macro_campaign_is_bounded_and_releases_outputs() {
    macro_campaign(8, 512);
}

#[test]
#[ignore = "opt-in longer deterministic hardware-free campaign; run in Release"]
fn extended_seeded_campaign() {
    mapping_campaign(4096, 1024);
    macro_campaign(128, 4096);
}
