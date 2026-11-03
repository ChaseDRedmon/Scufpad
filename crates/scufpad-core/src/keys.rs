use anyhow::{Result, bail, ensure};

const NAMED_KEYS: &[(&str, i32)] = &[
    ("ESC", 1),
    ("MINUS", 12),
    ("EQUAL", 13),
    ("BACKSPACE", 14),
    ("TAB", 15),
    ("LEFTBRACKET", 26),
    ("RIGHTBRACKET", 27),
    ("ENTER", 28),
    ("CTRL", 29),
    ("SEMICOLON", 39),
    ("APOSTROPHE", 40),
    ("GRAVE", 41),
    ("SHIFT", 42),
    ("BACKSLASH", 43),
    ("COMMA", 51),
    ("DOT", 52),
    ("SLASH", 53),
    ("RIGHTSHIFT", 54),
    ("ALT", 56),
    ("SPACE", 57),
    ("CAPSLOCK", 58),
    ("NUMLOCK", 69),
    ("SCROLLLOCK", 70),
    ("KPENTER", 96),
    ("RIGHTCTRL", 97),
    ("PRINTSCREEN", 99),
    ("RIGHTALT", 100),
    ("HOME", 102),
    ("UP", 103),
    ("PAGEUP", 104),
    ("LEFT", 105),
    ("RIGHT", 106),
    ("END", 107),
    ("DOWN", 108),
    ("PAGEDOWN", 109),
    ("INSERT", 110),
    ("DELETE", 111),
    ("MUTE", 113),
    ("VOLUMEDOWN", 114),
    ("VOLUMEUP", 115),
    ("PAUSE", 119),
    ("META", 125),
    ("RIGHTMETA", 126),
    ("COMPOSE", 127),
    ("NEXTSONG", 163),
    ("PLAYPAUSE", 164),
    ("PREVIOUSSONG", 165),
    ("STOPCD", 166),
];
const LETTERS: &[u8] = b"QWERTYUIOPASDFGHJKLZXCVBNM";
const LETTER_CODES: &[i32] = &[
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 44, 45, 46, 47, 48,
    49, 50,
];
const ALIASES: &[(&str, i32)] = &[
    ("CONTROL", 29),
    ("LEFTCTRL", 29),
    ("LEFTSHIFT", 42),
    ("LEFTALT", 56),
    ("SUPER", 125),
    ("ESCAPE", 1),
    ("RETURN", 28),
];

fn parse_key(text: &str) -> Result<i32> {
    let text = text.trim();
    for &(name, code) in NAMED_KEYS.iter().chain(ALIASES) {
        if text.eq_ignore_ascii_case(name) {
            return Ok(code);
        }
    }
    if text.len() == 1 {
        if let Some(i) = LETTERS
            .iter()
            .position(|b| b.eq_ignore_ascii_case(&text.as_bytes()[0]))
        {
            return Ok(LETTER_CODES[i]);
        }
        match text.as_bytes()[0] {
            b'1'..=b'9' => return Ok((text.as_bytes()[0] - b'1') as i32 + 2),
            b'0' => return Ok(11),
            _ => {}
        }
    }
    if text
        .as_bytes()
        .first()
        .is_some_and(|b| b.eq_ignore_ascii_case(&b'F'))
    {
        let n = &text[1..];
        if let Ok(number) = n.parse::<i32>()
            && (1..=12).contains(&number)
            && n == number.to_string()
        {
            return Ok(if number <= 10 {
                58 + number
            } else {
                76 + number
            });
        }
    }
    let numeric = if text
        .get(..4)
        .is_some_and(|p| p.eq_ignore_ascii_case("KEY_"))
    {
        &text[4..]
    } else {
        text
    };
    if !numeric.is_empty()
        && numeric.bytes().all(|b| b.is_ascii_digit())
        && let Ok(code) = numeric.parse::<i32>()
        && (1..=255).contains(&code)
    {
        return Ok(code);
    }
    bail!("Unknown Linux key '{text}'. Use a key name or KEY_1 … KEY_255.")
}

pub fn parse_chord(text: &str) -> Result<Vec<i32>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut keys = Vec::new();
    for key in text.split('+') {
        let code = parse_key(key)?;
        ensure!(
            keys.len() < 16 && !keys.contains(&code),
            "A chord needs at most 16 different keys."
        );
        keys.push(code);
    }
    Ok(keys)
}

fn format_key(code: i32) -> String {
    if let Some(&(name, _)) = NAMED_KEYS.iter().find(|(_, c)| *c == code) {
        return name.into();
    }
    if let Some(i) = LETTER_CODES.iter().position(|&c| c == code) {
        return (LETTERS[i] as char).to_string();
    }
    match code {
        2..=10 => (code - 1).to_string(),
        11 => "0".into(),
        59..=68 => format!("F{}", code - 58),
        87..=88 => format!("F{}", code - 76),
        _ => format!("KEY_{code}"),
    }
}
pub fn format_chord(keys: &[i32]) -> String {
    keys.iter()
        .map(|&key| format_key(key))
        .collect::<Vec<_>>()
        .join("+")
}
