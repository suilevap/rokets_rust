//! Value formats of XNA's intermediate XML (`<XnaContent><Asset Type=...>`),
//! as written by the C# game and its level editor.
//!
//! - `Vector2`: `"x y"`
//! - `Color`: `"AARRGGBB"` hex
//! - `Rectangle`: `"x y width height"`
//! - integer arrays: `"1 2 3"`

use serde::{Deserialize, Deserializer, de::Error as _};

/// A 2D vector as stored in the XML (map cells or pixels; the field says which).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct XVec2 {
    pub x: f32,
    pub y: f32,
}

/// A colour with straight (non-premultiplied) 8-bit channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for XColor {
    fn default() -> Self {
        XColor::WHITE
    }
}

impl XColor {
    pub const WHITE: XColor = XColor::rgba(255, 255, 255, 255);
    pub const BLACK: XColor = XColor::rgba(0, 0, 0, 255);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        XColor { r, g, b, a }
    }

    /// Parses `AARRGGBB` (an optional leading `#` is accepted).
    pub fn parse_argb(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 8 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(XColor {
            a: (v >> 24) as u8,
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        })
    }

    /// Channels as 0..1 floats.
    pub fn to_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

/// An integer rectangle in map cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl XRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + self.w) as f32
            && y < (self.y + self.h) as f32
    }
}

fn floats<const N: usize>(s: &str) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    let mut it = s.split_whitespace();
    for v in out.iter_mut() {
        *v = it.next()?.parse().ok()?;
    }
    it.next().is_none().then_some(out)
}

impl XVec2 {
    pub fn parse(s: &str) -> Option<Self> {
        floats::<2>(s).map(|[x, y]| XVec2 { x, y })
    }
}

impl XRect {
    pub fn parse(s: &str) -> Option<Self> {
        let [x, y, w, h] = floats::<4>(s)?;
        Some(XRect {
            x: x as i32,
            y: y as i32,
            w: w as i32,
            h: h as i32,
        })
    }
}

impl<'de> Deserialize<'de> for XVec2 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        XVec2::parse(&s).ok_or_else(|| D::Error::custom(format!("bad Vector2 {s:?}")))
    }
}

impl<'de> Deserialize<'de> for XColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        XColor::parse_argb(&s).ok_or_else(|| D::Error::custom(format!("bad Color {s:?}")))
    }
}

impl<'de> Deserialize<'de> for XRect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        XRect::parse(&s).ok_or_else(|| D::Error::custom(format!("bad Rectangle {s:?}")))
    }
}

/// `<Name><Item>a</Item><Item>b</Item></Name>`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Items<T> {
    #[serde(rename = "Item", default = "Vec::new")]
    pub items: Vec<T>,
}

impl<T> Default for Items<T> {
    fn default() -> Self {
        Items { items: Vec::new() }
    }
}

impl<T> Items<T> {
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}

/// Space-separated integers: `<Subscribers>4001 1004</Subscribers>`.
pub fn int_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i64>, D::Error> {
    let s = String::deserialize(d)?;
    s.split_whitespace()
        .map(|v| v.parse().map_err(D::Error::custom))
        .collect()
}

/// Booleans in either case (`true`, `True`).
pub fn boolean<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let s = String::deserialize(d)?;
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" | "" => Ok(false),
        other => Err(D::Error::custom(format!("bad bool {other:?}"))),
    }
}

/// Converts an XNA content path (`Entities\CargoQuest`) to a relative asset
/// path with forward slashes and no extension.
pub fn content_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values() {
        assert_eq!(XVec2::parse("9.5 3.5"), Some(XVec2 { x: 9.5, y: 3.5 }));
        assert_eq!(
            XColor::parse_argb("FFFF8000"),
            Some(XColor::rgba(255, 128, 0, 255))
        );
        assert_eq!(
            XColor::parse_argb("#100000A0"),
            Some(XColor::rgba(0, 0, 0xA0, 0x10))
        );
        assert_eq!(
            XRect::parse("8 20 1 3"),
            Some(XRect {
                x: 8,
                y: 20,
                w: 1,
                h: 3
            })
        );
        assert_eq!(XVec2::parse("1"), None);
    }
}
