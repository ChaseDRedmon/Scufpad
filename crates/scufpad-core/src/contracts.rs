use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! controls {
    ($name:ident { $($variant:ident = $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[repr(u8)]
        pub enum $name { $($variant = $value),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn bit(self) -> u64 { 1u64 << self as u8 }
        }
    };
}

controls!(PhysicalControl {
    A=0, B=1, X=2, Y=3, LeftBumper=4, RightBumper=5, View=6, Menu=7,
    LeftStickClick=8, RightStickClick=9, Guide=10, G1=11, G2=12, G3=13, G4=14,
    G5=15, HardwareProfile=16, RearOuterRight=19, RearInnerRight=20,
    RearInnerLeft=21, RearOuterLeft=22, SaxLeft=23, SaxRight=24,
    DpadUp=25, DpadRight=26, DpadDown=27, DpadLeft=28, LeftTrigger=29, RightTrigger=30,
});
controls!(GamepadControl {
    A=0, B=1, X=2, Y=3, LeftBumper=4, RightBumper=5, View=6, Menu=7, Guide=8,
    LeftStickClick=9, RightStickClick=10, Paddle1=11, Paddle2=12, Paddle3=13,
    Paddle4=14, DpadUp=15, DpadRight=16, DpadDown=17, DpadLeft=18,
    LeftTrigger=19, RightTrigger=20,
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PhysicalState {
    pub buttons: u64,
    pub left_x: i32,
    pub left_y: i32,
    pub right_x: i32,
    pub right_y: i32,
    pub left_trigger: i32,
    pub right_trigger: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingKind {
    #[default]
    Passthrough,
    Disabled,
    Transparent,
    Gamepad,
    Keys,
    Mouse,
    Macro,
    MomentaryLayer,
    ToggleLayer,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Binding {
    pub kind: BindingKind,
    pub code: i32,
    pub keys: Vec<i32>,
    pub reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayerDefinition {
    pub id: String,
    pub bindings: BTreeMap<PhysicalControl, Binding>,
}
impl Default for LayerDefinition {
    fn default() -> Self {
        Self {
            id: "Base".into(),
            bindings: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MacroDefinition {
    pub id: String,
    pub repeat_while_held: bool,
    pub steps: Vec<MacroStep>,
}
impl Default for MacroDefinition {
    fn default() -> Self {
        Self {
            id: "Macro".into(),
            repeat_while_held: false,
            steps: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroStepKind {
    #[default]
    PressKey,
    ReleaseKey,
    PressMouse,
    ReleaseMouse,
    PressGamepad,
    ReleaseGamepad,
    Delay,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MacroStep {
    pub kind: MacroStepKind,
    pub code: i32,
    pub delay_ms: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemapProfile {
    pub version: i32,
    pub name: String,
    pub base_layer: String,
    pub layers: Vec<LayerDefinition>,
    pub macros: Vec<MacroDefinition>,
    pub trigger_press_threshold: i32,
    pub trigger_release_threshold: i32,
}
impl Default for RemapProfile {
    fn default() -> Self {
        Self {
            version: 1,
            name: "Default".into(),
            base_layer: "Base".into(),
            layers: vec![LayerDefinition::default()],
            macros: Vec::new(),
            trigger_press_threshold: 512,
            trigger_release_threshold: 480,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlCommand {
    #[default]
    GetState,
    GetProfile,
    ApplyProfile,
    Arm,
    StopAll,
    Shutdown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ControlRequest {
    pub command: ControlCommand,
    pub profile: Option<RemapProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BridgeState {
    pub connected: bool,
    pub armed: bool,
    pub hardware_ready: bool,
    pub verified_extra_controls: u64,
    pub physical_buttons: u64,
    pub recent_presses: u64,
    pub gamepad_buttons: u64,
    pub active_layers: u64,
    pub keyboard0: u64,
    pub keyboard1: u64,
    pub keyboard2: u64,
    pub keyboard3: u64,
    pub mouse_buttons: u8,
    pub left_x: i32,
    pub left_y: i32,
    pub right_x: i32,
    pub right_y: i32,
    pub left_trigger: i32,
    pub right_trigger: i32,
    pub profile_name: String,
    pub status: String,
}
impl Default for BridgeState {
    fn default() -> Self {
        Self {
            connected: false,
            armed: false,
            hardware_ready: false,
            verified_extra_controls: 0,
            physical_buttons: 0,
            recent_presses: 0,
            gamepad_buttons: 0,
            active_layers: 0,
            keyboard0: 0,
            keyboard1: 0,
            keyboard2: 0,
            keyboard3: 0,
            mouse_buttons: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
            left_trigger: 0,
            right_trigger: 0,
            profile_name: "Default".into(),
            status: "Waiting for controller".into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ControlResponse {
    pub success: bool,
    pub error: Option<String>,
    pub profile: Option<RemapProfile>,
    pub state: Option<BridgeState>,
}
