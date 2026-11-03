use scufpad_core::*;
use serde::Deserialize;

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    name: String,
    hex: String,
    expected_physical_mask: u64,
    axes: [i32; 6],
}

#[test]
fn decode_all_38_static_usb_fixture_reports() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("../../../tests/fixtures/envision-v2-usb.json")).unwrap();
    assert_eq!(fixtures.len(), 38);
    for f in fixtures {
        let [
            left_x,
            left_y,
            right_x,
            right_y,
            left_trigger,
            right_trigger,
        ] = f.axes;
        assert_eq!(
            decode_usb_report(&hex(&f.hex)),
            Some(PhysicalState {
                buttons: f.expected_physical_mask,
                left_x,
                left_y,
                right_x,
                right_y,
                left_trigger,
                right_trigger
            }),
            "{}",
            f.name
        );
    }
}

#[test]
fn reject_reserved_raw_bits_and_wrong_lengths_ids_and_hats() {
    let neutral = hex("06000000000000000000008000000000");
    for bit in [17, 18, 25, 26, 27, 28, 29, 30, 31] {
        let mut report = neutral.clone();
        report[12 + bit / 8] = 1 << (bit % 8);
        assert_eq!(decode_usb_report(&report), None, "bit {bit}");
    }
    for length in [0, 15, 17] {
        assert_eq!(decode_usb_report(&vec![0; length]), None);
    }
    for hat in 9..=15 {
        let mut report = neutral.clone();
        report[11] = hat << 4;
        assert_eq!(decode_usb_report(&report), None);
    }
    let mut report = neutral;
    report[0] = 7;
    assert_eq!(decode_usb_report(&report), None);
}

#[test]
fn diagonal_hats_and_hardware_profile_are_preserved() {
    use PhysicalControl::*;
    for (hat, a, b) in [
        (1, DpadUp, DpadRight),
        (3, DpadRight, DpadDown),
        (5, DpadDown, DpadLeft),
        (7, DpadLeft, DpadUp),
    ] {
        let mut report = [0; 16];
        report[0] = 6;
        report[11] = hat << 4;
        assert_eq!(
            decode_usb_report(&report).unwrap().buttons,
            a.bit() | b.bit()
        );
    }
    assert_eq!(
        decode_usb_report(&hex("06000000000000000000008000000100"))
            .unwrap()
            .buttons,
        HARDWARE_PROFILE_MASK
    );
}
