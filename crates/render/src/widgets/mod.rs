pub mod bar;
pub mod course;
pub mod elevation_profile;
pub mod readout;
pub mod scale;

use tiny_skia::Color;

/// Parse a hex color string of the form `#rgb`, `#rrggbb`, or `#rrggbbaa`.
///
/// Returns `None` for malformed input (missing `#`, bad hex digits, wrong length).
pub(crate) fn parse_hex(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#')?;
    let bytes = match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            [r, g, b, 255]
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            [r, g, b, 255]
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            [r, g, b, a]
        }
        _ => return None,
    };
    Some(Color::from_rgba8(bytes[0], bytes[1], bytes[2], bytes[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_rrggbb() {
        let c = parse_hex("#ffcc00").unwrap();
        let u8c = c.to_color_u8();
        assert_eq!(u8c.red(), 0xff);
        assert_eq!(u8c.green(), 0xcc);
        assert_eq!(u8c.blue(), 0x00);
        assert_eq!(u8c.alpha(), 0xff);
    }

    #[test]
    fn parse_hex_rgb_short() {
        let c = parse_hex("#f00").unwrap();
        let u8c = c.to_color_u8();
        assert_eq!(u8c.red(), 0xff);
        assert_eq!(u8c.green(), 0x00);
        assert_eq!(u8c.blue(), 0x00);
        assert_eq!(u8c.alpha(), 0xff);
    }

    #[test]
    fn parse_hex_rrggbbaa() {
        let c = parse_hex("#00112233").unwrap();
        let u8c = c.to_color_u8();
        assert_eq!(u8c.red(), 0x00);
        assert_eq!(u8c.green(), 0x11);
        assert_eq!(u8c.blue(), 0x22);
        assert_eq!(u8c.alpha(), 0x33);
    }

    #[test]
    fn parse_hex_rejects_bad_input() {
        assert!(parse_hex("ffcc00").is_none()); // missing #
        assert!(parse_hex("#ggg").is_none());
        assert!(parse_hex("#12345").is_none());
    }
}
