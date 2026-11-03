use scufpad_core::*;

#[test]
fn all_linux_key_codes_roundtrip_with_canonical_names_or_raw_fallback() {
    for code in 1..=255 {
        assert_eq!(parse_chord(&format_chord(&[code])).unwrap(), [code]);
    }
    assert_eq!(
        parse_chord(" control + a + RightAlt + KEY_255 ").unwrap(),
        [29, 30, 100, 255]
    );
    assert_eq!(format_chord(&[29, 30, 100, 255]), "CTRL+A+RIGHTALT+KEY_255");
    assert_eq!(parse_chord("SUPER+ESCAPE+RETURN").unwrap(), [125, 1, 28]);
    assert_eq!(
        parse_chord("LEFTCTRL+LEFTSHIFT+LEFTALT").unwrap(),
        [29, 42, 56]
    );
    assert_eq!(
        parse_chord("F1+F10+F11+F12+1+9+0").unwrap(),
        [59, 68, 87, 88, 2, 10, 11]
    );
    assert_eq!(parse_chord("key_030").unwrap(), [30]);
    assert!(parse_chord("  ").unwrap().is_empty());
    assert_eq!(format_chord(&[]), "");
}

#[test]
fn chords_reject_duplicate_aliases_invalid_codes_and_oversized_sets() {
    for value in [
        "CTRL+CONTROL",
        "A+A",
        "A+",
        "+A",
        "KEY_0",
        "KEY_256",
        "-1",
        "+1",
        "KEY_+1",
        "KEY_ 1",
        "Unknown",
        "F01",
        "F13",
        "é",
        "Ｆ1",
    ] {
        assert!(parse_chord(value).is_err(), "{value}");
    }
    let sixteen = (1..=16)
        .map(|code| format!("KEY_{code}"))
        .collect::<Vec<_>>()
        .join("+");
    assert_eq!(parse_chord(&sixteen).unwrap().len(), 16);
    assert!(parse_chord(&(sixteen + "+KEY_17")).is_err());
}
