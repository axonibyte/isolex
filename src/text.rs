//! Text, the parts of it that need no font: how an interpolated number
//! reads, and which characters cannot be set one glyph after another.
//! Layout itself, which needs a font's glyphs and metrics, is expansion's
//! (`expand.rs`).

use crate::ir::format_number;

/// A number interpolated into text (SPEC §4): formatted by the output
/// rule, two decimals ties to even, then without the zeros that add
/// nothing, so 48 reads `48`, 2.5 `2.5` and 1/3 `0.33`.
pub fn format_value(v: f64) -> String {
    let s = format_number(v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Why a character cannot be set, if it cannot. Text is set left to
/// right, a glyph for each character or ligature, so what needs more
/// than that is refused rather than drawn wrong: control characters,
/// combining marks, right-to-left scripts and the controls that order
/// them, scripts whose letters change shape with their neighbors, and
/// the joiners and selectors that ask for it.
pub fn refusal(c: char) -> Option<&'static str> {
    let u = c as u32;
    let within = |ranges: &[(u32, u32)]| ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&u));
    if c.is_control() {
        Some("a control character, which text cannot hold; text is one line")
    } else if within(&[
        (0x0300, 0x036F),
        (0x1AB0, 0x1AFF),
        (0x1DC0, 0x1DFF),
        (0x20D0, 0x20FF),
        (0xFE20, 0xFE2F),
    ]) {
        Some("a combining mark, which is set on the glyph before it; use the precomposed character")
    } else if within(&[
        (0x0590, 0x08FF),
        (0xFB1D, 0xFDFF),
        (0xFE70, 0xFEFF),
        (0x10800, 0x10FFF),
        (0x1E800, 0x1EFFF),
        (0x200E, 0x200F),
        (0x202A, 0x202E),
        (0x2066, 0x2069),
    ]) {
        Some("right-to-left text, which is not set yet")
    } else if within(&[
        (0x0900, 0x0DFF),
        (0x0E00, 0x0EFF),
        (0x0F00, 0x0FFF),
        (0x1000, 0x109F),
        (0x1100, 0x11FF),
        (0x1780, 0x17FF),
        (0xA960, 0xA97F),
        (0xD7B0, 0xD7FF),
    ]) {
        Some("a script whose letters change shape with their neighbors, which is not set yet")
    } else if within(&[(0x200C, 0x200D), (0xFE00, 0xFE0F), (0xE0100, 0xE01EF)]) {
        Some("a joiner or variation selector, which asks for shaping text does not do")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_without_the_zeros_that_add_nothing() {
        assert_eq!(format_value(48.0), "48");
        assert_eq!(format_value(2.5), "2.5");
        assert_eq!(format_value(1.0 / 3.0), "0.33");
        assert_eq!(format_value(-1.5), "-1.5");
        assert_eq!(format_value(-0.0), "0");
        assert_eq!(format_value(-0.001), "0");
        assert_eq!(format_value(0.125), "0.12");
        assert_eq!(format_value(100.0), "100");
        assert_eq!(format_value(1e20), "100000000000000000000");
    }

    #[test]
    fn what_cannot_be_set_left_to_right_is_refused() {
        for c in ['A', 'é', 'ß', '€', '0', ' ', 'Ω', '中', 'ж'] {
            assert_eq!(refusal(c), None, "{c}");
        }
        assert!(refusal('\n').unwrap().contains("control"));
        assert!(refusal('\u{0301}').unwrap().contains("combining"));
        assert!(refusal('א').unwrap().contains("right-to-left"));
        assert!(refusal('ب').unwrap().contains("right-to-left"));
        assert!(refusal('\u{202E}').unwrap().contains("right-to-left"));
        assert!(refusal('क').unwrap().contains("shape"));
        assert!(refusal('ก').unwrap().contains("shape"));
        assert!(refusal('\u{200D}').unwrap().contains("joiner"));
        assert!(refusal('\u{FE0F}').unwrap().contains("selector"));
    }
}
