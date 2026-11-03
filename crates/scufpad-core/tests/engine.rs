use scufpad_core::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct CountingAllocator;
fn count_allocation() {
    let _ = TRACK_ALLOCATIONS.try_with(|track| {
        if track.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn state(buttons: u64) -> PhysicalState {
    PhysicalState {
        buttons,
        ..PhysicalState::default()
    }
}

#[test]
fn gamepad_passthrough_preserves_normal_control_identities() {
    for (physical, virtual_control) in [
        (PhysicalControl::A, GamepadControl::A),
        (PhysicalControl::B, GamepadControl::B),
        (PhysicalControl::X, GamepadControl::X),
        (PhysicalControl::Y, GamepadControl::Y),
        (PhysicalControl::LeftBumper, GamepadControl::LeftBumper),
        (PhysicalControl::RightBumper, GamepadControl::RightBumper),
        (PhysicalControl::View, GamepadControl::View),
        (PhysicalControl::Menu, GamepadControl::Menu),
        (PhysicalControl::Guide, GamepadControl::Guide),
        (
            PhysicalControl::LeftStickClick,
            GamepadControl::LeftStickClick,
        ),
        (
            PhysicalControl::RightStickClick,
            GamepadControl::RightStickClick,
        ),
        (PhysicalControl::DpadUp, GamepadControl::DpadUp),
        (PhysicalControl::DpadRight, GamepadControl::DpadRight),
        (PhysicalControl::DpadDown, GamepadControl::DpadDown),
        (PhysicalControl::DpadLeft, GamepadControl::DpadLeft),
        (PhysicalControl::LeftTrigger, GamepadControl::LeftTrigger),
        (PhysicalControl::RightTrigger, GamepadControl::RightTrigger),
    ] {
        let output = OutputState::gamepad_passthrough(state(physical.bit()));
        assert_eq!(output.gamepad_buttons, virtual_control.bit());
        assert!(!output.keyboard.iter().any(|&pressed| pressed));
        assert!(!output.mouse.iter().any(|&pressed| pressed));
    }
}

#[test]
fn gamepad_passthrough_excludes_custom_inputs_and_allocates_nothing() {
    let extras = EXTRA_BUTTONS_MASK
        | HARDWARE_PROFILE_MASK
        | PhysicalControl::G1.bit()
        | PhysicalControl::G2.bit()
        | PhysicalControl::G3.bit()
        | PhysicalControl::G4.bit()
        | PhysicalControl::G5.bit();
    assert_eq!(
        OutputState::gamepad_passthrough(state(extras)),
        OutputState::default()
    );
    let input = PhysicalState {
        buttons: u64::MAX,
        left_x: -32768,
        left_y: 32767,
        right_x: 12345,
        right_y: -23456,
        left_trigger: 99,
        right_trigger: 1023,
    };
    let paddles = GamepadControl::Paddle1.bit()
        | GamepadControl::Paddle2.bit()
        | GamepadControl::Paddle3.bit()
        | GamepadControl::Paddle4.bit();
    let expected = GamepadControl::ALL
        .iter()
        .fold(0, |bits, control| bits | control.bit())
        & !paddles;
    ALLOCATIONS.set(0);
    TRACK_ALLOCATIONS.set(true);
    for _ in 0..2000 {
        let output = OutputState::gamepad_passthrough(input);
        assert_eq!(output.gamepad_buttons, expected);
        assert_eq!(
            (output.left_x, output.left_y, output.right_x, output.right_y),
            (input.left_x, input.left_y, input.right_x, input.right_y)
        );
        assert_eq!((output.left_trigger, output.right_trigger), (99, 1023));
        assert!(!output.keyboard.iter().any(|&pressed| pressed));
        assert!(!output.mouse.iter().any(|&pressed| pressed));
    }
    TRACK_ALLOCATIONS.set(false);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
fn keys(keys: &[i32]) -> Binding {
    Binding {
        kind: BindingKind::Keys,
        keys: keys.into(),
        ..Binding::default()
    }
}
fn layer(id: &str, toggle: bool) -> Binding {
    Binding {
        kind: if toggle {
            BindingKind::ToggleLayer
        } else {
            BindingKind::MomentaryLayer
        },
        reference: id.into(),
        ..Binding::default()
    }
}
fn profile(bindings: &[(PhysicalControl, Binding)], extra: Vec<LayerDefinition>) -> RemapProfile {
    let mut profile = RemapProfile::default();
    profile.layers[0].bindings = bindings.iter().cloned().collect();
    profile.layers.extend(extra);
    profile
}
fn ready(profile: RemapProfile) -> MappingEngine {
    let mut engine = MappingEngine::new(profile).unwrap();
    engine.tick(state(0), 0).unwrap();
    engine
}
fn step(kind: MacroStepKind, code: i32) -> MacroStep {
    MacroStep {
        kind,
        code,
        delay_ms: 0,
    }
}
fn delay(ms: i32) -> MacroStep {
    MacroStep {
        kind: MacroStepKind::Delay,
        delay_ms: ms,
        code: 0,
    }
}
fn macro_profile(repeat: bool, steps: Vec<MacroStep>) -> RemapProfile {
    let mut p = profile(
        &[
            (
                PhysicalControl::A,
                Binding {
                    kind: BindingKind::Macro,
                    reference: "Macro".into(),
                    ..Binding::default()
                },
            ),
            (PhysicalControl::B, keys(&[30])),
        ],
        vec![],
    );
    p.macros.push(MacroDefinition {
        repeat_while_held: repeat,
        steps,
        ..MacroDefinition::default()
    });
    p
}

#[test]
fn all_default_digital_mappings_and_unmapped_controls() {
    use GamepadControl as G;
    use PhysicalControl as P;
    let mappings = [
        (P::A, G::A),
        (P::B, G::B),
        (P::X, G::X),
        (P::Y, G::Y),
        (P::LeftBumper, G::LeftBumper),
        (P::RightBumper, G::RightBumper),
        (P::View, G::View),
        (P::Menu, G::Menu),
        (P::Guide, G::Guide),
        (P::LeftStickClick, G::LeftStickClick),
        (P::RightStickClick, G::RightStickClick),
        (P::RearOuterLeft, G::Paddle1),
        (P::RearInnerLeft, G::Paddle2),
        (P::RearInnerRight, G::Paddle3),
        (P::RearOuterRight, G::Paddle4),
        (P::DpadUp, G::DpadUp),
        (P::DpadRight, G::DpadRight),
        (P::DpadDown, G::DpadDown),
        (P::DpadLeft, G::DpadLeft),
    ];
    for (physical, gamepad) in mappings {
        let mut e = ready(RemapProfile::default());
        e.tick(state(physical.bit()), 1).unwrap();
        assert_eq!(e.output.gamepad_buttons, gamepad.bit(), "{physical:?}");
        e.tick(state(0), 2).unwrap();
        assert_eq!(e.output.gamepad_buttons, 0);
    }
    let mut e = ready(RemapProfile::default());
    e.tick(
        state(
            [
                P::G1,
                P::G2,
                P::G3,
                P::G4,
                P::G5,
                P::SaxLeft,
                P::SaxRight,
                P::HardwareProfile,
            ]
            .iter()
            .fold(0, |bits, p| bits | p.bit()),
        ),
        1,
    )
    .unwrap();
    assert_eq!(e.output.gamepad_buttons, 0);
    assert_eq!(P::ALL.len(), 29);
    assert_eq!(G::ALL.len(), 21);
    assert!(!P::ALL.iter().any(|p| matches!(*p as u8, 17 | 18)));
}

#[test]
fn shared_chord_gamepad_and_mouse_owners_release_independently() {
    use PhysicalControl::{A, B};
    let mut e = ready(profile(&[(A, keys(&[29, 30])), (B, keys(&[30]))], vec![]));
    e.tick(state(A.bit() | B.bit()), 1).unwrap();
    e.tick(state(B.bit()), 2).unwrap();
    assert!(!e.output.keyboard[29]);
    assert!(e.output.keyboard[30]);
    e.tick(state(0), 3).unwrap();
    assert!(e.output.keyboard.iter().all(|b| !b));
    for (kind, code) in [(BindingKind::Gamepad, 0), (BindingKind::Mouse, 272)] {
        let binding = Binding {
            kind,
            code,
            ..Binding::default()
        };
        let mut e = ready(profile(&[(A, binding.clone()), (B, binding)], vec![]));
        e.tick(state(A.bit() | B.bit()), 1).unwrap();
        e.tick(state(B.bit()), 2).unwrap();
        assert_eq!(e.output.mouse[0], kind == BindingKind::Mouse);
        assert_eq!(
            e.output.gamepad_buttons,
            u64::from(kind == BindingKind::Gamepad)
        );
        e.tick(state(0), 3).unwrap();
        assert_eq!(e.output, OutputState::default());
    }
}

fn shifted() -> RemapProfile {
    profile(
        &[
            (PhysicalControl::RightBumper, layer("Shift", false)),
            (PhysicalControl::A, keys(&[30])),
        ],
        vec![LayerDefinition {
            id: "Shift".into(),
            bindings: [(PhysicalControl::A, keys(&[31]))].into(),
        }],
    )
}

#[test]
fn layer_press_precedes_regular_press_and_release_uses_latched_binding() {
    use PhysicalControl::{A, RightBumper as R};
    let mut e = ready(shifted());
    e.tick(state(A.bit() | R.bit()), 1).unwrap();
    assert!(e.output.keyboard[31]);
    assert!(!e.output.keyboard[30]);
    e.tick(state(A.bit()), 2).unwrap();
    assert_eq!(e.active_layers, 1);
    assert!(e.output.keyboard[31]);
    e.tick(state(0), 3).unwrap();
    assert!(!e.output.keyboard[31]);
}

#[test]
fn releases_precede_new_presses_in_same_frame() {
    use PhysicalControl::{A, RightBumper as R};
    let mut e = ready(shifted());
    e.tick(state(R.bit()), 1).unwrap();
    e.tick(state(A.bit()), 2).unwrap();
    assert!(e.output.keyboard[30]);
    assert!(!e.output.keyboard[31]);
}

#[test]
fn momentary_owners_and_toggle_coexist() {
    use PhysicalControl::{LeftBumper as L, RightBumper as R, X};
    let mut e = ready(profile(
        &[
            (L, layer("Shift", false)),
            (R, layer("Shift", false)),
            (X, layer("Shift", true)),
        ],
        vec![LayerDefinition {
            id: "Shift".into(),
            ..LayerDefinition::default()
        }],
    ));
    for (now, buttons, layers) in [
        (1, L.bit() | R.bit(), 3),
        (2, R.bit(), 3),
        (3, R.bit() | X.bit(), 3),
        (4, 0, 3),
        (5, X.bit(), 1),
    ] {
        e.tick(state(buttons), now).unwrap();
        assert_eq!(e.active_layers, layers);
    }
}

#[test]
fn highest_layer_transparency_and_disabled_bindings() {
    use PhysicalControl::{A, B, LeftBumper as L, RightBumper as R};
    let mut e = ready(profile(
        &[
            (L, layer("One", false)),
            (R, layer("Two", false)),
            (A, keys(&[30])),
            (B, keys(&[30])),
        ],
        vec![
            LayerDefinition {
                id: "One".into(),
                bindings: [(A, keys(&[31])), (B, keys(&[31]))].into(),
            },
            LayerDefinition {
                id: "Two".into(),
                bindings: [
                    (
                        A,
                        Binding {
                            kind: BindingKind::Transparent,
                            ..Binding::default()
                        },
                    ),
                    (
                        B,
                        Binding {
                            kind: BindingKind::Disabled,
                            ..Binding::default()
                        },
                    ),
                ]
                .into(),
            },
        ],
    ));
    e.tick(state(L.bit() | R.bit()), 1).unwrap();
    e.tick(state(L.bit() | R.bit() | A.bit() | B.bit()), 2)
        .unwrap();
    assert_eq!(e.active_layers, 7);
    assert!(e.output.keyboard[31]);
    assert!(!e.output.keyboard[30]);
    e.tick(state(B.bit()), 3).unwrap();
    assert!(e.output.keyboard.iter().all(|b| !b));
}

#[test]
fn chained_layer_actions_are_resolved_before_ordinary_presses() {
    use PhysicalControl::{A, B, X};
    let mut e = ready(profile(
        &[(B, layer("One", false))],
        vec![
            LayerDefinition {
                id: "One".into(),
                bindings: [(A, layer("Two", false))].into(),
            },
            LayerDefinition {
                id: "Two".into(),
                bindings: [(X, keys(&[31]))].into(),
            },
        ],
    ));
    e.tick(state(A.bit() | B.bit() | X.bit()), 1).unwrap();
    assert_eq!(e.active_layers, 7);
    assert!(e.output.keyboard[31]);
    e.tick(state(0), 2).unwrap();
    assert_eq!(e.active_layers, 1);
}

#[test]
fn sticks_and_default_triggers_preserve_analog_values() {
    let mut e = ready(RemapProfile::default());
    e.tick(
        PhysicalState {
            left_x: -32768,
            left_y: 32767,
            right_x: -123,
            right_y: 456,
            left_trigger: 700,
            right_trigger: 1023,
            buttons: 0,
        },
        1,
    )
    .unwrap();
    assert_eq!(
        (
            e.output.left_x,
            e.output.left_y,
            e.output.right_x,
            e.output.right_y
        ),
        (-32768, 32767, -123, 456)
    );
    assert_eq!((e.output.left_trigger, e.output.right_trigger), (700, 1023));
}

#[test]
fn trigger_remapping_consumes_axis_and_uses_hysteresis() {
    let mut e = ready(profile(
        &[(PhysicalControl::LeftTrigger, keys(&[30]))],
        vec![],
    ));
    for (now, value, pressed) in [
        (1, 511, false),
        (2, 512, true),
        (3, 481, true),
        (4, 480, false),
    ] {
        e.tick(
            PhysicalState {
                left_trigger: value,
                ..state(0)
            },
            now,
        )
        .unwrap();
        assert_eq!(e.output.keyboard[30], pressed);
        assert_eq!(e.output.left_trigger, 0);
        assert_eq!(
            e.physical_buttons & PhysicalControl::LeftTrigger.bit() != 0,
            pressed
        );
    }
}

#[test]
fn explicit_gamepad_trigger_is_digital_and_native_owner_is_not() {
    let mut e = ready(profile(
        &[(
            PhysicalControl::LeftTrigger,
            Binding {
                kind: BindingKind::Gamepad,
                code: GamepadControl::RightTrigger as i32,
                ..Binding::default()
            },
        )],
        vec![],
    ));
    e.tick(
        PhysicalState {
            left_trigger: 700,
            right_trigger: 123,
            ..state(0)
        },
        1,
    )
    .unwrap();
    assert_eq!((e.output.left_trigger, e.output.right_trigger), (0, 1023));
    e.tick(
        PhysicalState {
            right_trigger: 123,
            ..state(0)
        },
        2,
    )
    .unwrap();
    assert_eq!(e.output.right_trigger, 123);
    let mut e = ready(profile(
        &[(
            PhysicalControl::A,
            Binding {
                kind: BindingKind::Gamepad,
                code: GamepadControl::LeftTrigger as i32,
                ..Binding::default()
            },
        )],
        vec![],
    ));
    e.tick(
        PhysicalState {
            left_trigger: 700,
            ..state(PhysicalControl::A.bit())
        },
        1,
    )
    .unwrap();
    assert_eq!(e.output.left_trigger, 1023);
    e.tick(
        PhysicalState {
            left_trigger: 700,
            ..state(0)
        },
        2,
    )
    .unwrap();
    assert_eq!(e.output.left_trigger, 700);
}

#[test]
fn first_frame_cancel_and_apply_suppress_held_sources_until_release() {
    let p = profile(&[(PhysicalControl::A, keys(&[30]))], vec![]);
    let mut e = MappingEngine::new(p).unwrap();
    let held = PhysicalState {
        left_trigger: 700,
        ..state(PhysicalControl::A.bit())
    };
    e.tick(held, 0).unwrap();
    assert!(!e.output.keyboard[30]);
    assert_eq!(e.output.left_trigger, 0);
    e.tick(state(0), 1).unwrap();
    e.tick(held, 2).unwrap();
    assert!(e.output.keyboard[30]);
    e.cancel_all();
    assert_eq!(e.output, OutputState::default());
    e.tick(held, 3).unwrap();
    assert!(!e.output.keyboard[30]);
    assert_eq!(e.output.left_trigger, 0);
    e.tick(state(0), 4).unwrap();
    e.tick(held, 5).unwrap();
    e.apply_profile(profile(&[(PhysicalControl::A, keys(&[31]))], vec![]))
        .unwrap();
    e.tick(held, 6).unwrap();
    assert!(e.output.keyboard.iter().all(|b| !b));
    e.tick(state(0), 7).unwrap();
    e.tick(held, 8).unwrap();
    assert!(e.output.keyboard[31]);
}

#[test]
fn owned_profile_is_an_immutable_snapshot_and_invalid_apply_is_transactional() {
    let mut p = profile(&[(PhysicalControl::A, keys(&[30]))], vec![]);
    let mut e = ready(p.clone());
    p.layers[0].bindings.clear();
    let mut snapshot = e.profile().clone();
    snapshot.layers[0]
        .bindings
        .get_mut(&PhysicalControl::A)
        .unwrap()
        .keys[0] = 31;
    e.tick(state(PhysicalControl::A.bit()), 1).unwrap();
    assert!(e.output.keyboard[30]);
    assert!(!e.output.keyboard[31]);
    assert!(
        e.apply_profile(RemapProfile {
            version: 2,
            ..RemapProfile::default()
        })
        .is_err()
    );
    assert!(e.output.keyboard[30]);
    assert_eq!(e.profile().version, 1);
}

#[test]
fn oneshot_survives_release_and_cannot_release_another_owner() {
    use MacroStepKind::*;
    use PhysicalControl::{A, B};
    let mut e = ready(macro_profile(
        false,
        vec![
            step(ReleaseKey, 30),
            step(PressKey, 30),
            step(PressKey, 30),
            delay(10),
            step(ReleaseKey, 30),
        ],
    ));
    e.tick(state(A.bit() | B.bit()), 1).unwrap();
    assert_eq!(e.next_due_ms, Some(11));
    e.tick(state(B.bit()), 2).unwrap();
    assert!(e.output.keyboard[30]);
    e.tick(state(B.bit()), 11).unwrap();
    assert_eq!(e.next_due_ms, None);
    assert!(e.output.keyboard[30]);
    e.tick(state(0), 12).unwrap();
    assert!(!e.output.keyboard[30]);
}

#[test]
fn additional_trigger_is_ignored_while_source_instance_runs() {
    let mut e = ready(macro_profile(
        false,
        vec![step(MacroStepKind::PressKey, 30), delay(10)],
    ));
    e.tick(state(PhysicalControl::A.bit()), 1).unwrap();
    e.tick(state(0), 2).unwrap();
    e.tick(state(PhysicalControl::A.bit()), 3).unwrap();
    assert_eq!(e.next_due_ms, Some(11));
    e.tick(state(PhysicalControl::A.bit()), 11).unwrap();
    assert!(!e.output.keyboard[30]);
    e.tick(state(PhysicalControl::A.bit()), 12).unwrap();
    assert_eq!(e.next_due_ms, None);
}

#[test]
fn oneshot_hard_deadline_releases_only_own_outputs_before_budgeted_cleanup() {
    for another_owner in [false, true] {
        let mut steps = vec![step(MacroStepKind::PressKey, 30), delay(60000)];
        steps.extend((2..256).map(|_| step(MacroStepKind::ReleaseKey, 31)));
        let mut e = ready(macro_profile(false, steps));
        let buttons = PhysicalControl::A.bit()
            | if another_owner {
                PhysicalControl::B.bit()
            } else {
                0
            };
        e.tick(state(buttons), 1).unwrap();
        assert!(e.output.keyboard[30]);
        assert_eq!(e.next_due_ms, Some(60001));
        e.tick(state(buttons), 60000).unwrap();
        assert!(e.output.keyboard[30]);
        e.tick(state(buttons), 60001).unwrap();
        assert_eq!(e.output.keyboard[30], another_owner);
        assert_eq!(e.next_due_ms, None);
        e.tick(state(0), 60002).unwrap();
        assert!(!e.output.keyboard[30]);
    }
}

#[test]
fn repeating_macro_stops_on_release_and_deadline_without_releasing_other_owner() {
    use PhysicalControl::{A, B};
    let mut e = ready(macro_profile(
        true,
        vec![step(MacroStepKind::PressKey, 30), delay(10)],
    ));
    e.tick(state(A.bit() | B.bit()), 1).unwrap();
    e.tick(state(B.bit()), 2).unwrap();
    assert!(e.output.keyboard[30]);
    assert_eq!(e.next_due_ms, None);
    e.tick(state(A.bit()), 3).unwrap();
    e.tick(state(A.bit()), 60003).unwrap();
    assert!(!e.output.keyboard[30]);
    assert_eq!(e.next_due_ms, None);
    e.tick(state(A.bit()), 60004).unwrap();
    assert_eq!(e.next_due_ms, None);
}

#[test]
fn repeating_macro_clock_releases_mouse_and_gamepad_and_cancel_neutralizes() {
    use MacroStepKind::*;
    let mut e = ready(macro_profile(
        true,
        vec![
            step(PressMouse, 272),
            step(PressGamepad, 19),
            delay(10),
            step(ReleaseMouse, 272),
            step(ReleaseGamepad, 19),
            delay(10),
        ],
    ));
    let held = state(PhysicalControl::A.bit());
    e.tick(held, 1).unwrap();
    assert!(e.output.mouse[0]);
    assert_eq!(e.output.left_trigger, 1023);
    e.tick(held, 11).unwrap();
    assert!(!e.output.mouse[0]);
    assert_eq!(e.output.left_trigger, 0);
    e.tick(held, 21).unwrap();
    assert!(e.output.mouse[0]);
    e.cancel_all();
    assert_eq!(e.output, OutputState::default());
    assert_eq!(e.next_due_ms, None);
}

#[test]
fn global_budget_defers_remaining_work_and_oneshot_cleans_at_end() {
    let mut steps: Vec<_> = (1..=100)
        .map(|code| step(MacroStepKind::PressKey, code))
        .collect();
    steps.push(delay(10));
    let mut e = ready(macro_profile(false, steps));
    e.tick(state(PhysicalControl::A.bit()), 1).unwrap();
    assert_eq!(e.output.keyboard.iter().filter(|b| **b).count(), 64);
    assert_eq!(e.next_due_ms, Some(1));
    e.tick(state(0), 1).unwrap();
    assert_eq!(e.output.keyboard.iter().filter(|b| **b).count(), 100);
    assert_eq!(e.next_due_ms, Some(11));
    e.tick(state(0), 11).unwrap();
    assert!(e.output.keyboard.iter().all(|b| !b));
    assert_eq!(e.next_due_ms, None);
}

#[test]
fn macro_instances_are_per_source_and_budget_is_fair() {
    use PhysicalControl::{A, B};
    let mut p = macro_profile(
        false,
        (1..=100)
            .map(|code| step(MacroStepKind::PressKey, code))
            .collect(),
    );
    p.layers[0].bindings.insert(
        B,
        Binding {
            kind: BindingKind::Macro,
            reference: "Other".into(),
            ..Binding::default()
        },
    );
    p.macros.push(MacroDefinition {
        id: "Other".into(),
        steps: (101..=200)
            .map(|code| step(MacroStepKind::PressKey, code))
            .collect(),
        ..MacroDefinition::default()
    });
    let mut e = ready(p);
    e.tick(state(A.bit() | B.bit()), 1).unwrap();
    assert_eq!(
        e.output.keyboard[1..=100].iter().filter(|b| **b).count(),
        32
    );
    assert_eq!(
        e.output.keyboard[101..=200].iter().filter(|b| **b).count(),
        32
    );
    for _ in 0..3 {
        e.tick(state(0), 1).unwrap();
    }
    assert!(e.output.keyboard.iter().all(|b| !b));
    assert_eq!(e.next_due_ms, None);
}

#[test]
fn empty_oneshot_finishes_and_normal_end_releases_all_output_classes() {
    let mut e = ready(macro_profile(false, vec![]));
    e.tick(state(PhysicalControl::A.bit()), 1).unwrap();
    assert_eq!(e.next_due_ms, None);
    let mut e = ready(macro_profile(
        false,
        vec![
            step(MacroStepKind::PressKey, 30),
            step(MacroStepKind::PressMouse, 279),
            step(MacroStepKind::PressGamepad, 20),
            delay(1),
        ],
    ));
    e.tick(state(PhysicalControl::A.bit()), 1).unwrap();
    assert!(e.output.keyboard[30] && e.output.mouse[7]);
    assert_eq!(e.output.right_trigger, 1023);
    e.tick(state(0), 2).unwrap();
    assert_eq!(e.output, OutputState::default());
}

#[test]
fn apply_clears_macros_and_clock_is_monotonic_and_saturating() {
    let p = macro_profile(false, vec![step(MacroStepKind::PressKey, 30), delay(10)]);
    let mut e = ready(p.clone());
    let held = state(PhysicalControl::A.bit());
    e.tick(held, 1).unwrap();
    e.apply_profile(p).unwrap();
    assert_eq!(e.output, OutputState::default());
    assert_eq!(e.next_due_ms, None);
    e.tick(held, 2).unwrap();
    assert_eq!(e.next_due_ms, None);
    let before = e.output.clone();
    assert!(e.tick(state(0), 1).is_err());
    assert_eq!(e.output, before);
    e.tick(state(0), u64::MAX - 5).unwrap();
    e.tick(held, u64::MAX - 4).unwrap();
    assert_eq!(e.next_due_ms, Some(u64::MAX));
    assert!(e.output.keyboard[30]);
    e.tick(held, u64::MAX).unwrap();
    assert_eq!(e.next_due_ms, None);
    assert!(!e.output.keyboard[30]);
}

#[test]
fn repeated_ticks_keep_output_buffers_in_place() {
    let mut e = ready(profile(&[(PhysicalControl::A, keys(&[30]))], vec![]));
    let keyboard = e.output.keyboard.as_ptr();
    let mouse = e.output.mouse.as_ptr();
    ALLOCATIONS.set(0);
    TRACK_ALLOCATIONS.set(true);
    for now in 1..2000 {
        e.tick(
            state(if now % 2 == 0 {
                PhysicalControl::A.bit()
            } else {
                0
            }),
            now,
        )
        .unwrap();
        assert_eq!(e.output.keyboard.as_ptr(), keyboard);
        assert_eq!(e.output.mouse.as_ptr(), mouse);
    }
    TRACK_ALLOCATIONS.set(false);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
