use serde::{Deserialize, Serialize};

mod validate;
pub use validate::{MetricCatalog, ValidationError, Warning};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub version: u32,
    pub canvas: Canvas,
    pub units: Units,
    pub theme: Theme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rider: Option<Rider>,
    pub widgets: Vec<Widget>,
}

/// Rider-specific configuration (separate from layout proper, but colocated
/// in the same file for v1 so a layout can carry everything needed to render
/// derived metrics like W/kg).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Rider {
    pub weight_kg: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Units {
    pub speed: SpeedUnit,
    pub distance: DistanceUnit,
    pub elevation: ElevationUnit,
    pub temp: TempUnit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpeedUnit {
    Kmh,
    Mph,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DistanceUnit {
    Km,
    Mi,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ElevationUnit {
    M,
    Ft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TempUnit {
    C,
    F,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub font: String,
    pub fg: String,
    pub accent: String,
    pub shadow: Option<Shadow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shadow {
    pub blur: f32,
    pub color: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorKind {
    #[default]
    Fill,
    Rect,
    Arrow,
    Needle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Indicator {
    #[serde(default)]
    pub kind: IndicatorKind,
    #[serde(default)]
    pub fill_under: bool,
}

fn default_show_numbers() -> bool {
    true
}

fn default_gauge_start_deg() -> f32 {
    -135.0
}

fn default_gauge_end_deg() -> f32 {
    135.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Ticks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major_every: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor_every: Option<f32>,
    #[serde(default = "default_show_numbers")]
    pub show_numbers: bool,
    #[serde(default)]
    pub decimals: u32,
}

impl Default for Ticks {
    fn default() -> Self {
        Self {
            major_every: None,
            minor_every: None,
            show_numbers: true,
            decimals: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Widget {
    Readout {
        id: String,
        metric: String,
        rect: Rect,
        label: String,
        decimals: u32,
        font_size: f32,
        /// Label font size. When omitted, defaults to `font_size * 0.35`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label_font_size: Option<f32>,
        /// Unit font size (e.g. "km/h", "W/kg"). When omitted, defaults to
        /// `font_size`. When smaller, the unit baseline-aligns with the
        /// number's baseline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit_font_size: Option<f32>,
    },
    Course {
        id: String,
        rect: Rect,
        line_width: f32,
        dot_radius: f32,
    },
    ElevationProfile {
        id: String,
        rect: Rect,
    },
    /// Horizontal progress bar for a continuous cumulative metric.
    ///
    /// v1 supports `metric = "distance"` (auto-maxes at the activity's final
    /// distance) or `metric = "elev_gain"` (auto-maxes at final cumulative
    /// gain). `min` defaults to 0.0 when omitted; `max` must be supplied for
    /// any other metric.
    Bar {
        id: String,
        metric: String,
        rect: Rect,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f32>,
        #[serde(default)]
        show_text: bool,
        #[serde(default)]
        decimals: u32,
    },
    Meter {
        id: String,
        metric: String,
        rect: Rect,
        min: f32,
        max: f32,
        #[serde(default)]
        orientation: Orientation,
        #[serde(default)]
        indicator: Indicator,
        #[serde(default)]
        ticks: Ticks,
        #[serde(default)]
        show_value: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_font_size: Option<f32>,
    },
    Gauge {
        id: String,
        metric: String,
        rect: Rect,
        min: f32,
        max: f32,
        #[serde(default = "default_gauge_start_deg")]
        start_deg: f32,
        #[serde(default = "default_gauge_end_deg")]
        end_deg: f32,
        #[serde(default)]
        indicator: Indicator,
        #[serde(default)]
        ticks: Ticks,
        #[serde(default)]
        show_value: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_font_size: Option<f32>,
    },
}

impl Widget {
    pub fn id(&self) -> &str {
        match self {
            Widget::Readout { id, .. }
            | Widget::Course { id, .. }
            | Widget::ElevationProfile { id, .. }
            | Widget::Bar { id, .. }
            | Widget::Meter { id, .. }
            | Widget::Gauge { id, .. } => id,
        }
    }

    pub fn rect(&self) -> Rect {
        match self {
            Widget::Readout { rect, .. }
            | Widget::Course { rect, .. }
            | Widget::ElevationProfile { rect, .. }
            | Widget::Bar { rect, .. }
            | Widget::Meter { rect, .. }
            | Widget::Gauge { rect, .. } => *rect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_example_layout() {
        let json = r##"{
            "version": 1,
            "canvas": { "width": 1920, "height": 1080, "fps": 30 },
            "units": { "speed": "kmh", "distance": "km", "elevation": "m", "temp": "c" },
            "theme": {
                "font": "Inter",
                "fg": "#ffffff",
                "accent": "#ffcc00",
                "shadow": { "blur": 4.0, "color": "#000000cc" }
            },
            "widgets": [
                {
                    "type": "readout",
                    "id": "speed_readout",
                    "metric": "speed",
                    "rect": { "x": 80, "y": 900, "w": 260, "h": 120 },
                    "label": "SPEED",
                    "decimals": 1,
                    "font_size": 72.0
                },
                {
                    "type": "course",
                    "id": "course_map",
                    "rect": { "x": 1560, "y": 60, "w": 300, "h": 300 },
                    "line_width": 4.0,
                    "dot_radius": 8.0
                },
                {
                    "type": "elevation_profile",
                    "id": "elev_profile",
                    "rect": { "x": 80, "y": 60, "w": 500, "h": 120 }
                }
            ]
        }"##;
        let layout: Layout = serde_json::from_str(json).unwrap();
        let back = serde_json::to_string(&layout).unwrap();
        let layout2: Layout = serde_json::from_str(&back).unwrap();
        assert_eq!(layout, layout2);
        assert_eq!(layout.widgets.len(), 3);
    }

    #[test]
    fn widget_tagged_by_type() {
        let w: Widget = serde_json::from_str(
            r#"{
            "type": "readout", "id": "x", "metric": "hr",
            "rect": { "x": 0, "y": 0, "w": 10, "h": 10 },
            "label": "HR", "decimals": 0, "font_size": 48.0
        }"#,
        )
        .unwrap();
        match w {
            Widget::Readout { .. } => {}
            _ => panic!("expected Readout variant"),
        }
    }

    #[test]
    fn orientation_serde_snake_case() {
        let j = serde_json::to_string(&Orientation::Horizontal).unwrap();
        assert_eq!(j, "\"horizontal\"");
        let j = serde_json::to_string(&Orientation::Vertical).unwrap();
        assert_eq!(j, "\"vertical\"");
        let h: Orientation = serde_json::from_str("\"horizontal\"").unwrap();
        assert_eq!(h, Orientation::Horizontal);
    }

    #[test]
    fn indicator_defaults_to_fill() {
        let ind: Indicator = serde_json::from_str("{}").unwrap();
        assert_eq!(ind.kind, IndicatorKind::Fill);
        assert!(!ind.fill_under);
    }

    #[test]
    fn indicator_kind_roundtrip() {
        for (name, k) in [
            ("fill", IndicatorKind::Fill),
            ("rect", IndicatorKind::Rect),
            ("arrow", IndicatorKind::Arrow),
            ("needle", IndicatorKind::Needle),
        ] {
            let quoted = format!("\"{}\"", name);
            let parsed: IndicatorKind = serde_json::from_str(&quoted).unwrap();
            assert_eq!(parsed, k);
            assert_eq!(serde_json::to_string(&k).unwrap(), quoted);
        }
    }

    #[test]
    fn ticks_defaults() {
        let t: Ticks = serde_json::from_str("{}").unwrap();
        assert_eq!(t.major_every, None);
        assert_eq!(t.minor_every, None);
        assert!(t.show_numbers);
        assert_eq!(t.decimals, 0);
    }

    #[test]
    fn meter_round_trip() {
        let json = r#"{
            "type": "meter",
            "id": "spd",
            "metric": "speed",
            "rect": { "x": 0, "y": 0, "w": 100, "h": 20 },
            "min": 0.0,
            "max": 60.0
        }"#;
        let w: Widget = serde_json::from_str(json).unwrap();
        match w {
            Widget::Meter { metric, min, max, orientation, .. } => {
                assert_eq!(metric, "speed");
                assert_eq!(min, 0.0);
                assert_eq!(max, 60.0);
                assert_eq!(orientation, Orientation::Horizontal); // default
            }
            _ => panic!("expected Meter"),
        }
    }

    #[test]
    fn gauge_defaults_to_classic_sweep() {
        let json = r#"{
            "type": "gauge",
            "id": "spd_g",
            "metric": "speed",
            "rect": { "x": 0, "y": 0, "w": 200, "h": 200 },
            "min": 0.0,
            "max": 60.0
        }"#;
        let w: Widget = serde_json::from_str(json).unwrap();
        match w {
            Widget::Gauge { start_deg, end_deg, .. } => {
                assert_eq!(start_deg, -135.0);
                assert_eq!(end_deg, 135.0);
            }
            _ => panic!("expected Gauge"),
        }
    }
}
