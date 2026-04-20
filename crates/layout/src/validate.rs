use crate::{Canvas, Layout, Rect, Widget};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("unknown layout version {0} (expected 1)")]
    UnsupportedVersion(u32),
    #[error("widget '{id}' rect {rect:?} overflows canvas {canvas_w}x{canvas_h}")]
    RectOverflow {
        id: String,
        rect: Rect,
        canvas_w: u32,
        canvas_h: u32,
    },
    #[error("widget '{id}' references unknown metric '{metric}'")]
    UnknownMetric { id: String, metric: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    MetricAbsent { widget_id: String, metric: String },
}

/// Abstract list of metrics known to the caller. Keeps `layout` crate
/// independent from `activity`.
pub struct MetricCatalog<'a> {
    /// names of metrics that *exist*
    pub known: &'a [&'a str],
    /// subset of `known` that are present in the current activity
    pub present: &'a [&'a str],
}

impl Layout {
    /// Check version, rect bounds, and metric references against a catalog.
    /// Returns Ok(warnings) on success (warnings list may be empty).
    pub fn validate(&self, catalog: &MetricCatalog) -> Result<Vec<Warning>, ValidationError> {
        if self.version != 1 {
            return Err(ValidationError::UnsupportedVersion(self.version));
        }
        let mut warnings = Vec::new();
        for w in &self.widgets {
            let id = w.id().to_string();
            let rect = w.rect();
            check_rect(&id, rect, &self.canvas)?;
            if let Widget::Readout { metric, .. } = w {
                if !catalog.known.iter().any(|k| *k == metric) {
                    return Err(ValidationError::UnknownMetric {
                        id: id.clone(),
                        metric: metric.clone(),
                    });
                }
                if !catalog.present.iter().any(|p| *p == metric) {
                    warnings.push(Warning::MetricAbsent {
                        widget_id: id.clone(),
                        metric: metric.clone(),
                    });
                }
            }
            // Course + ElevationProfile have no metric string; they implicitly
            // require lat/lon (course) or altitude+distance (elev profile).
            // For v1, we don't require these here — they'll degrade gracefully
            // at render time (draw empty rect).
        }
        Ok(warnings)
    }
}

fn check_rect(id: &str, rect: Rect, canvas: &Canvas) -> Result<(), ValidationError> {
    let x0 = rect.x as i64;
    let y0 = rect.y as i64;
    let x1 = x0 + rect.w as i64;
    let y1 = y0 + rect.h as i64;
    if x0 < 0 || y0 < 0 || x1 > canvas.width as i64 || y1 > canvas.height as i64 {
        return Err(ValidationError::RectOverflow {
            id: id.to_string(),
            rect,
            canvas_w: canvas.width,
            canvas_h: canvas.height,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn minimal_canvas() -> Canvas {
        Canvas {
            width: 100,
            height: 100,
            fps: 30,
        }
    }
    fn minimal_units() -> Units {
        Units {
            speed: SpeedUnit::Kmh,
            distance: DistanceUnit::Km,
            elevation: ElevationUnit::M,
            temp: TempUnit::C,
        }
    }
    fn minimal_theme() -> Theme {
        Theme {
            font: "Inter".into(),
            fg: "#fff".into(),
            accent: "#ff0".into(),
            shadow: None,
        }
    }

    fn make_layout(widgets: Vec<Widget>, version: u32) -> Layout {
        Layout {
            version,
            canvas: minimal_canvas(),
            units: minimal_units(),
            theme: minimal_theme(),
            rider: None,
            widgets,
        }
    }

    fn known_metrics() -> Vec<&'static str> {
        vec![
            "speed",
            "heart_rate",
            "power",
            "cadence",
            "altitude",
            "distance",
            "elev_gain",
            "gradient",
            "time_elapsed",
            "time_of_day",
        ]
    }

    #[test]
    fn unknown_version_is_error() {
        let l = make_layout(vec![], 2);
        let known = known_metrics();
        let catalog = MetricCatalog {
            known: &known,
            present: &known,
        };
        assert!(matches!(
            l.validate(&catalog),
            Err(ValidationError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn rect_overflow_is_error() {
        let w = Widget::Readout {
            id: "r1".into(),
            metric: "speed".into(),
            rect: Rect {
                x: 90,
                y: 0,
                w: 30,
                h: 10,
            }, // extends past x=100
            label: "SPEED".into(),
            decimals: 1,
            font_size: 24.0,
            label_font_size: None,
            unit_font_size: None,
        };
        let l = make_layout(vec![w], 1);
        let known = known_metrics();
        let catalog = MetricCatalog {
            known: &known,
            present: &known,
        };
        assert!(matches!(
            l.validate(&catalog),
            Err(ValidationError::RectOverflow { .. })
        ));
    }

    #[test]
    fn unknown_metric_is_error() {
        let w = Widget::Readout {
            id: "r1".into(),
            metric: "blood_pressure".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            label: "BP".into(),
            decimals: 0,
            font_size: 24.0,
            label_font_size: None,
            unit_font_size: None,
        };
        let l = make_layout(vec![w], 1);
        let known = known_metrics();
        let catalog = MetricCatalog {
            known: &known,
            present: &[],
        };
        assert!(matches!(
            l.validate(&catalog),
            Err(ValidationError::UnknownMetric { .. })
        ));
    }

    #[test]
    fn absent_metric_is_warning() {
        let w = Widget::Readout {
            id: "r1".into(),
            metric: "power".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            label: "POWER".into(),
            decimals: 0,
            font_size: 24.0,
            label_font_size: None,
            unit_font_size: None,
        };
        let l = make_layout(vec![w], 1);
        // Catalog knows "power" but says it's absent on this activity.
        let known = known_metrics();
        let catalog = MetricCatalog {
            known: &known,
            present: &["speed"],
        };
        let warnings = l.validate(&catalog).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(&warnings[0], Warning::MetricAbsent { metric, .. } if metric == "power"));
    }

    #[test]
    fn valid_layout_no_warnings() {
        let w = Widget::Course {
            id: "map".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 50,
                h: 50,
            },
            line_width: 2.0,
            dot_radius: 4.0,
        };
        let l = make_layout(vec![w], 1);
        let known = known_metrics();
        let catalog = MetricCatalog {
            known: &known,
            present: &known,
        };
        let warnings = l.validate(&catalog).unwrap();
        assert!(warnings.is_empty());
    }
}
