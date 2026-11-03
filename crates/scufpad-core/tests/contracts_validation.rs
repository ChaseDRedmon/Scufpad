use scufpad_core::*;
use serde_json::{Value, json};

fn rejected(value: Value) {
    match serde_json::from_value::<RemapProfile>(value) {
        Err(error) => assert!(!error.to_string().is_empty()),
        Ok(profile) => assert!(validate_profile(&profile).is_err(), "Accepted {profile:?}"),
    }
}
fn binding(value: Value) -> Value {
    json!({"layers":[{"bindings":{"A":value}}]})
}

#[test]
fn defaults_match_csharp_and_contracts_roundtrip_exact_names() {
    let profile: RemapProfile = serde_json::from_str("{}").unwrap();
    assert_eq!(profile, RemapProfile::default());
    validate_profile(&profile).unwrap();
    let state: BridgeState = serde_json::from_str("{}").unwrap();
    assert_eq!(state.profile_name, "Default");
    assert_eq!(state.status, "Waiting for controller");
    let request: ControlRequest =
        serde_json::from_value(json!({"command":"ApplyProfile","profile":{
            "layers":[{"bindings":{"Guide":{"kind":"Keys","keys":[29,30]}}}],
            "macros":[{"steps":[{"kind":"Delay","delayMs":1}]}]
        }}))
        .unwrap();
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["command"], "ApplyProfile");
    assert_eq!(
        value["profile"]["layers"][0]["bindings"]["Guide"]["kind"],
        "Keys"
    );
    assert_eq!(value["profile"]["macros"][0]["steps"][0]["kind"], "Delay");
    assert_eq!(value["profile"]["triggerPressThreshold"], 512);
    assert_eq!(
        request,
        serde_json::from_value::<ControlRequest>(value).unwrap()
    );
    for control in PhysicalControl::ALL {
        let value = serde_json::to_string(control).unwrap();
        assert_eq!(
            *control,
            serde_json::from_str::<PhysicalControl>(&value).unwrap()
        );
        assert_eq!(control.bit(), 1 << *control as u8);
    }
    for control in GamepadControl::ALL {
        assert_eq!(
            *control,
            serde_json::from_value::<GamepadControl>(serde_json::to_value(control).unwrap())
                .unwrap()
        );
    }
}

#[test]
fn reject_all_profile_shape_version_threshold_and_id_failures() {
    for value in [
        json!(null),
        json!({"version":2}),
        json!({"name":" "}),
        json!({"name":"a".repeat(65)}),
        json!({"name":"😀".repeat(33)}),
        json!({"name":null}),
        json!({"layers":[]}),
        json!({"layers":null}),
        json!({"layers":[null]}),
        json!({"layers":[{"id":""}]}),
        json!({"layers":[{"id":"a".repeat(65)}]}),
        json!({"layers":[{},{}]}),
        json!({"baseLayer":"Other"}),
        json!({"baseLayer":null}),
        json!({"layers":[{"bindings":null}]}),
        json!({"triggerPressThreshold":1024}),
        json!({"triggerReleaseThreshold":-1}),
        json!({"triggerReleaseThreshold":512}),
        json!({"triggerPressThreshold":0}),
        json!({"macros":null}),
        json!({"macros":[null]}),
        json!({"macros":[{"id":""}]}),
        json!({"macros":[{"id":"a".repeat(65)}]}),
        json!({"macros":[{},{}]}),
        json!({"macros":[{"steps":null}]}),
        json!({"macros":[{"steps":[null]}]}),
        json!({"macros":[{"repeatWhileHeld":true,"steps":[]}]}),
        json!({"macros":[{"repeatWhileHeld":true,"steps":[{"kind":"PressKey","code":30}]}]}),
    ] {
        rejected(value);
    }
    rejected(
        json!({"layers":(0..33).map(|i|json!({"id":if i==0 { "Base".to_owned() }else{format!("L{i}")}})).collect::<Vec<_>>()}),
    );
    rejected(json!({"macros":(0..129).map(|i|json!({"id":format!("M{i}")})).collect::<Vec<_>>()}));
    rejected(json!({"macros":[{"steps":vec![json!({"kind":"Delay","delayMs":1});257]}]}));
}

#[test]
fn reject_invalid_macro_steps_codes_delays_and_total_duration() {
    for (kind, code) in [
        ("PressKey", 0),
        ("ReleaseKey", 256),
        ("PressMouse", 271),
        ("ReleaseMouse", 280),
        ("PressGamepad", -1),
        ("ReleaseGamepad", 21),
    ] {
        rejected(json!({"macros":[{"steps":[{"kind":kind,"code":code}]}]}));
    }
    for value in [
        json!({"kind":"Delay","delayMs":-1}),
        json!({"kind":"Delay","delayMs":60001}),
        json!({"kind":99}),
        json!({"kind":"Unknown"}),
        json!({"kind":null}),
        json!({"kind":"PressKey","code":30,"delayMs":-1}),
    ] {
        rejected(json!({"macros":[{"steps":[value]}]}));
    }
    rejected(
        json!({"macros":[{"steps":[{"kind":"Delay","delayMs":60000},{"kind":"Delay","delayMs":1}]}]}),
    );
}

#[test]
fn reject_invalid_controls_bindings_chords_and_references() {
    for control in ["HardwareProfile", "17", "18", "31", "Unknown"] {
        let mut bindings = serde_json::Map::new();
        bindings.insert(control.into(), json!({}));
        rejected(json!({"layers":[{"bindings":bindings}]}));
    }
    for value in [
        json!(null),
        json!({"kind":99}),
        json!({"kind":"Unknown"}),
        json!({"kind":null}),
        json!({"kind":"Transparent"}),
        json!({"kind":"Gamepad","code":-1}),
        json!({"kind":"Gamepad","code":21}),
        json!({"kind":"Mouse","code":271}),
        json!({"kind":"Mouse","code":280}),
        json!({"kind":"Keys","keys":[]}),
        json!({"kind":"Keys","keys":[0]}),
        json!({"kind":"Keys","keys":[256]}),
        json!({"kind":"Keys","keys":[30,30]}),
        json!({"keys":(1..=17).collect::<Vec<_>>()}),
        json!({"keys":null}),
        json!({"reference":null}),
        json!({"kind":"Macro","reference":"Missing"}),
        json!({"kind":"MomentaryLayer","reference":"Missing"}),
        json!({"kind":"ToggleLayer","reference":"Base"}),
    ] {
        rejected(binding(value));
    }
    rejected(binding(json!({"kind":"Disabled","keys":[0]})));
}

#[test]
fn reject_layer_cycles_even_when_unreachable_from_base() {
    for kind in ["MomentaryLayer", "ToggleLayer"] {
        rejected(json!({"layers":[{},
            {"id":"One","bindings":{"A":{"kind":kind,"reference":"Two"}}},
            {"id":"Two","bindings":{"B":{"kind":"ToggleLayer","reference":"One"}}}]}));
        rejected(
            json!({"layers":[{}, {"id":"One","bindings":{"A":{"kind":kind,"reference":"One"}}}]}),
        );
    }
}

#[test]
fn accept_all_limits_and_resolved_references() {
    let mut profile = RemapProfile {
        name: "a".repeat(64),
        trigger_release_threshold: 0,
        trigger_press_threshold: 1023,
        ..RemapProfile::default()
    };
    profile.layers.extend((1..32).map(|i| LayerDefinition {
        id: format!("L{i}"),
        ..LayerDefinition::default()
    }));
    profile.layers[0].bindings.insert(
        PhysicalControl::A,
        Binding {
            kind: BindingKind::Keys,
            keys: (1..=16).collect(),
            ..Binding::default()
        },
    );
    profile.layers[0].bindings.insert(
        PhysicalControl::B,
        Binding {
            kind: BindingKind::MomentaryLayer,
            reference: "L31".into(),
            ..Binding::default()
        },
    );
    profile.layers[0].bindings.insert(
        PhysicalControl::X,
        Binding {
            kind: BindingKind::Macro,
            reference: "M127".into(),
            ..Binding::default()
        },
    );
    profile.layers[31].bindings.insert(
        PhysicalControl::A,
        Binding {
            kind: BindingKind::Transparent,
            ..Binding::default()
        },
    );
    let mut steps = vec![
        MacroStep {
            kind: MacroStepKind::PressKey,
            code: 255,
            delay_ms: 0
        };
        255
    ];
    steps.push(MacroStep {
        kind: MacroStepKind::Delay,
        code: 0,
        delay_ms: 60000,
    });
    profile.macros = (0..128)
        .map(|i| MacroDefinition {
            id: format!("M{i}"),
            repeat_while_held: true,
            steps: steps.clone(),
        })
        .collect();
    validate_profile(&profile).unwrap();
    profile.name = "😀".repeat(32);
    validate_profile(&profile).unwrap();
}

#[test]
fn identifiers_are_case_sensitive_and_only_delay_steps_contribute_to_duration() {
    let mut p = RemapProfile::default();
    p.layers.push(LayerDefinition {
        id: "base".into(),
        ..LayerDefinition::default()
    });
    p.macros = vec![MacroDefinition {
        steps: vec![
            MacroStep {
                code: 30,
                delay_ms: 60000,
                ..MacroStep::default()
            };
            256
        ],
        ..MacroDefinition::default()
    }];
    validate_profile(&p).unwrap();
    p.macros[0].repeat_while_held = true;
    assert!(validate_profile(&p).is_err());
}
