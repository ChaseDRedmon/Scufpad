use super::*;
use std::time::Instant;

// Only the macro composer is instantiated: no Studio, Worker, IPC, display or I/O.
fn composer_campaign(frames: usize) {
    let ctx = egui::Context::default();
    let profile = RemapProfile::default();
    let original = profile.clone();
    let mut editor = MacroEditor::new(
        None,
        MacroDefinition {
            id: "Headless composer".into(),
            steps: vec![
                MacroStep {
                    kind: MacroStepKind::Delay,
                    delay_ms: 1,
                    code: 0
                };
                256
            ],
            ..Default::default()
        },
    );
    let mut random = 0xc0ffeeu64;
    let mut samples = Vec::with_capacity(frames);
    let mut rejected = 0;
    for frame in 0..frames {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        if !editor.definition.steps.is_empty() {
            editor.select(Some(random as usize % editor.definition.steps.len()));
            editor.draft.delay =
                ["", "-1", "60001", "😀", "0", "12"][(random >> 16) as usize % 6].into();
            let before = editor.definition.clone();
            let draft = editor.draft.clone();
            if editor.save_step().is_err() {
                rejected += 1;
                assert_eq!(editor.definition, before, "frame={frame}");
                assert_eq!(editor.draft, draft, "frame={frame}");
            }
        }
        let size = egui::vec2(
            320.0 + (random % 1200) as f32,
            240.0 + ((random >> 12) % 800) as f32,
        );
        let position = egui::pos2(
            (random % size.x as u64) as f32,
            ((random >> 20) % size.y as u64) as f32,
        );
        let mut events = vec![egui::Event::PointerMoved(position)];
        // Paired press/releases at a fixed position also exercise widget dispatch.
        events.push(egui::Event::PointerButton {
            pos: egui::pos2(50.0, 50.0),
            button: egui::PointerButton::Primary,
            pressed: frame % 2 == 0,
            modifiers: egui::Modifiers::default(),
        });
        events.push(egui::Event::Text("é😀KEY_255".into()));
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 / 60.0),
            events,
            ..Default::default()
        };
        let start = Instant::now();
        let output = ctx.run(input, |ctx| {
            if let MacroOutcome::Saved(candidate) = editor.show(ctx, &profile) {
                validate_profile(&candidate).unwrap();
            }
        });
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        samples.push(start.elapsed().as_nanos());
        for primitive in &primitives {
            assert!(primitive.clip_rect.is_finite(), "frame={frame}");
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|vertex| vertex.pos.is_finite() && vertex.uv.is_finite()),
                    "frame={frame}"
                );
            }
        }
        assert!(editor.definition.steps.len() <= 256, "frame={frame}");
        assert_eq!(
            profile, original,
            "frame={frame}: rendering changed workspace"
        );
    }
    assert!(rejected > 0);
    samples.sort_unstable();
    eprintln!(
        "headless_macro_composer frames={frames} rejected_drafts={rejected} render_and_tessellate_ns_p50={} p95={} max={}",
        samples[frames / 2],
        samples[frames * 95 / 100],
        samples[frames - 1]
    );
}

#[test]
fn rejected_loaded_workspace_error_survives_successful_worker_completion() {
    let (worker, events) = Worker::test_channel();
    let mut studio = Studio::with_worker(worker);
    let before = studio.profile.clone();
    let mut oversized = before.clone();
    oversized.layers[0].bindings.insert(
        PhysicalControl::A,
        Binding {
            reference: "x".repeat(MAX_MESSAGE_BYTES),
            ..Default::default()
        },
    );
    validate_profile(&oversized).unwrap();
    events
        .try_send(Event::InitialProfile(oversized.clone()))
        .unwrap();
    studio.receive();
    assert_eq!(studio.profile, before);
    assert!(
        studio.pending_apply,
        "rejected initial profile marked unchanged workspace as applied"
    );
    events
        .try_send(Event::Loaded {
            profile: oversized,
            path: None,
            revision: 0,
            imported: true,
        })
        .unwrap();
    events
        .try_send(Event::Done(Ok("Imported successfully".into())))
        .unwrap();
    studio.receive();
    assert_eq!(studio.profile, before);
    assert!(studio.error);
    assert!(studio.message.contains("1 MiB"), "{}", studio.message);
    assert!(!studio.busy);
    studio.say("Working…"); // The next explicit action clears the old error.
    events
        .try_send(Event::Done(Ok("Recovered".into())))
        .unwrap();
    studio.receive();
    assert!(!studio.error);
    assert_eq!(studio.message, "Recovered");
}

#[test]
fn seeded_headless_macro_composer_retains_invalid_drafts_and_finite_geometry() {
    composer_campaign(128);
}

#[test]
#[ignore = "opt-in headless rendering stress; not a GPU or interactive acceptance test"]
fn extended_headless_macro_composer() {
    composer_campaign(4096);
}
