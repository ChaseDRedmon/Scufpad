use anyhow::{Result, ensure};
use scufpad_core::*;
use serde::Serialize;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

struct CountingAllocator;
static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

fn allocated(size: usize) {
    if COUNT.load(Ordering::Relaxed) {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size as u64, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const CASES: &[&str] = &[
    "decode",
    "idle",
    "idle_32_layers",
    "passthrough_edges",
    "two_key_chord",
    "macro_3_steps",
    "macro_256_steps",
    "macro_26_concurrent",
    "held_layer",
    "trigger_chord",
    "state_serialization",
    "large_profile_compile",
];

fn profile(case: &str) -> RemapProfile {
    let mut profile = RemapProfile::default();
    let keys = || Binding {
        kind: BindingKind::Keys,
        keys: vec![29, 46],
        ..Default::default()
    };
    match case {
        "two_key_chord" => {
            profile.layers[0]
                .bindings
                .insert(PhysicalControl::A, keys());
        }
        "trigger_chord" => {
            profile.layers[0]
                .bindings
                .insert(PhysicalControl::LeftTrigger, keys());
        }
        "idle_32_layers" => {
            profile.layers = (0..32)
                .map(|i| LayerDefinition {
                    id: if i == 0 {
                        "Base".into()
                    } else {
                        format!("Layer{i}")
                    },
                    ..Default::default()
                })
                .collect();
        }
        "macro_3_steps" | "macro_256_steps" | "macro_26_concurrent" => {
            let mut steps = vec![
                MacroStep {
                    kind: MacroStepKind::PressKey,
                    code: 46,
                    delay_ms: 0,
                },
                MacroStep {
                    kind: MacroStepKind::Delay,
                    code: 0,
                    delay_ms: 5,
                },
                MacroStep {
                    kind: MacroStepKind::ReleaseKey,
                    code: 46,
                    delay_ms: 0,
                },
            ];
            if case == "macro_256_steps" {
                steps.resize(
                    256,
                    MacroStep {
                        kind: MacroStepKind::Delay,
                        code: 0,
                        delay_ms: 0,
                    },
                );
            }
            profile.macros.push(MacroDefinition {
                id: "Probe".into(),
                steps,
                ..Default::default()
            });
            profile.layers[0].bindings.insert(
                PhysicalControl::G1,
                Binding {
                    kind: BindingKind::Macro,
                    reference: "Probe".into(),
                    ..Default::default()
                },
            );
            if case == "macro_26_concurrent" {
                for &control in PhysicalControl::ALL.iter().filter(|&&c| {
                    !matches!(
                        c,
                        PhysicalControl::HardwareProfile
                            | PhysicalControl::DpadDown
                            | PhysicalControl::DpadLeft
                    )
                }) {
                    profile.layers[0].bindings.insert(
                        control,
                        Binding {
                            kind: BindingKind::Macro,
                            reference: "Probe".into(),
                            ..Default::default()
                        },
                    );
                }
            }
        }
        "held_layer" => {
            profile.layers.push(LayerDefinition {
                id: "Shift".into(),
                ..Default::default()
            });
            profile.layers[0].bindings.insert(
                PhysicalControl::G1,
                Binding {
                    kind: BindingKind::MomentaryLayer,
                    reference: "Shift".into(),
                    ..Default::default()
                },
            );
            profile.layers[1]
                .bindings
                .insert(PhysicalControl::A, keys());
        }
        "large_profile_compile" => {
            profile.layers = (0..16)
                .map(|i| LayerDefinition {
                    id: if i == 0 {
                        "Base".into()
                    } else {
                        format!("Layer{i}")
                    },
                    bindings: PhysicalControl::ALL
                        .iter()
                        .filter(|&&c| c != PhysicalControl::HardwareProfile)
                        .map(|&c| (c, keys()))
                        .collect(),
                })
                .collect();
            profile.macros = (0..96)
                .map(|i| MacroDefinition {
                    id: format!("Macro{i}"),
                    steps: vec![
                        MacroStep {
                            kind: MacroStepKind::Delay,
                            code: 0,
                            delay_ms: 1
                        };
                        128
                    ],
                    ..Default::default()
                })
                .collect();
        }
        _ => {}
    }
    profile
}

struct Runner {
    case: &'static str,
    engine: MappingEngine,
    profile: RemapProfile,
    response: ControlResponse,
    sequence: u64,
}

impl Runner {
    fn new(case: &'static str) -> Result<Self> {
        let profile = profile(case);
        validate_profile(&profile)?;
        let mut engine = MappingEngine::new(profile.clone())?;
        engine.tick(PhysicalState::default(), 0)?;
        Ok(Self {
            case,
            engine,
            profile,
            response: ControlResponse {
                success: true,
                state: Some(BridgeState::default()),
                ..Default::default()
            },
            sequence: 0,
        })
    }
    fn step(&mut self) -> Result<()> {
        let i = self.sequence;
        self.sequence += 1;
        match self.case {
            "decode" => {
                let mut bytes = [0u8; 16];
                bytes[0] = 6;
                bytes[11] = 0x80;
                bytes[12] = (i & 1) as u8;
                black_box(decode_usb_report(black_box(&bytes)).unwrap());
            }
            "state_serialization" => {
                let mut bytes = Vec::new();
                write_message(&mut bytes, black_box(&self.response))?;
                black_box(bytes);
            }
            "large_profile_compile" => {
                black_box(MappingEngine::new(black_box(self.profile.clone()))?);
            }
            _ => {
                let mut state = PhysicalState::default();
                match self.case {
                    "passthrough_edges" | "two_key_chord" if i & 1 == 0 => {
                        state.buttons = PhysicalControl::A.bit()
                    }
                    "macro_3_steps" | "macro_256_steps" if i.is_multiple_of(16) => {
                        state.buttons = PhysicalControl::G1.bit()
                    }
                    "macro_26_concurrent" if i.is_multiple_of(16) => {
                        state.buttons = PhysicalControl::ALL
                            .iter()
                            .filter(|&&c| {
                                !matches!(
                                    c,
                                    PhysicalControl::HardwareProfile
                                        | PhysicalControl::DpadDown
                                        | PhysicalControl::DpadLeft
                                )
                            })
                            .fold(0, |bits, c| bits | c.bit());
                        state.left_trigger = 600;
                        state.right_trigger = 600;
                    }
                    "held_layer" => {
                        if i % 32 < 16 {
                            state.buttons |= PhysicalControl::G1.bit();
                        }
                        if i & 1 == 0 {
                            state.buttons |= PhysicalControl::A.bit();
                        }
                    }
                    "trigger_chord" => state.left_trigger = [0, 600, 500, 400][(i % 4) as usize],
                    _ => {}
                }
                self.engine.tick(black_box(state), self.sequence)?;
                black_box(&self.engine.output);
            }
        }
        Ok(())
    }
    fn run(&mut self, count: u64) -> Result<()> {
        for _ in 0..count {
            self.step()?;
        }
        Ok(())
    }
}

fn preflight() -> Result<()> {
    for &case in CASES {
        let mut runner = Runner::new(case)?;
        ensure!(
            serde_json::to_vec(&runner.profile)?.len() <= MAX_MESSAGE_BYTES,
            "Oversize probe profile"
        );
        for i in 0..64 {
            runner.step()?;
            let output = &runner.engine.output;
            match case {
                "passthrough_edges" => {
                    ensure!((output.gamepad_buttons & GamepadControl::A.bit() != 0) == (i % 2 == 0))
                }
                "two_key_chord" => ensure!(
                    output.keyboard[29] == (i % 2 == 0) && output.keyboard[46] == (i % 2 == 0)
                ),
                "macro_3_steps" | "macro_256_steps" | "macro_26_concurrent" => {
                    ensure!(output.keyboard[46] == (i % 16 < 5))
                }
                "trigger_chord" => ensure!(
                    output.keyboard[29] == matches!(i % 4, 1 | 2) && output.left_trigger == 0
                ),
                "held_layer" => ensure!(output.keyboard[46] == (i % 32 < 16 && i % 2 == 0)),
                _ => {}
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct Measurement {
    scenario: &'static str,
    iterations_per_sample: u64,
    ns_per_operation: Vec<f64>,
    allocation_operations: u64,
    allocation_calls: u64,
    allocated_bytes: u64,
    profile_json_bytes: usize,
}

fn main() -> Result<()> {
    preflight()?;
    if std::env::args().any(|a| a == "--preflight") {
        println!(
            "PASS: all twelve synthetic workloads validate; no IPC, GUI, process or hardware initialized"
        );
        return Ok(());
    }
    let iterations = std::env::var("SCUFPAD_BENCH_ITERATIONS")
        .unwrap_or_else(|_| "1000000".into())
        .parse::<u64>()?;
    ensure!(
        (10000..=50000000).contains(&iterations),
        "Iterations must be 10000..50000000"
    );
    let mut measurements = Vec::new();
    for &case in CASES {
        let count = if case == "large_profile_compile" {
            (iterations / 10000).max(10)
        } else {
            iterations
        };
        let mut runner = Runner::new(case)?;
        runner.run(if case == "large_profile_compile" {
            2
        } else {
            10000
        })?;
        let mut samples = Vec::with_capacity(7);
        for _ in 0..7 {
            let start = Instant::now();
            runner.run(count)?;
            samples.push(start.elapsed().as_nanos() as f64 / count as f64);
        }
        let allocations = count.min(10000);
        ALLOCS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
        COUNT.store(true, Ordering::Relaxed);
        let result = runner.run(allocations);
        COUNT.store(false, Ordering::Relaxed);
        result?;
        measurements.push(Measurement {
            scenario: case,
            iterations_per_sample: count,
            ns_per_operation: samples,
            allocation_operations: allocations,
            allocation_calls: ALLOCS.load(Ordering::Relaxed),
            allocated_bytes: BYTES.load(Ordering::Relaxed),
            profile_json_bytes: serde_json::to_vec(&runner.profile)?.len(),
        });
    }
    println!("{}", serde_json::to_string(&measurements)?);
    Ok(())
}

#[test]
fn synthetic_workloads_validate_before_timing() {
    preflight().unwrap();
}
