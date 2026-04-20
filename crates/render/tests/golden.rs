use activity::{Activity, Sample};
use chrono::{TimeZone, Utc};
use image::RgbaImage;
use layout::{
    Canvas, DistanceUnit, ElevationUnit, Layout, Rect, SpeedUnit, TempUnit, Theme, Units, Widget,
};
use render::{render_frame, TextCtx};
use std::path::Path;
use std::time::Duration;
use tiny_skia::Pixmap;

fn mk_sample(secs: u64, lat: f64, lon: f64) -> Sample {
    Sample {
        t: Duration::from_secs(secs),
        lat,
        lon,
        altitude_m: None,
        speed_mps: None,
        heart_rate_bpm: None,
        cadence_rpm: None,
        power_w: None,
        distance_m: None,
        elev_gain_cum_m: None,
        gradient_pct: None,
    }
}

#[test]
fn readout_speed_matches_golden() {
    let layout = Layout {
        version: 1,
        canvas: Canvas {
            width: 400,
            height: 200,
            fps: 30,
        },
        units: Units {
            speed: SpeedUnit::Kmh,
            distance: DistanceUnit::Km,
            elevation: ElevationUnit::M,
            temp: TempUnit::C,
        },
        theme: Theme {
            font: "Inter".into(),
            fg: "#ffffff".into(),
            accent: "#ffcc00".into(),
            shadow: None,
        },
        widgets: vec![Widget::Readout {
            id: "speed".into(),
            metric: "speed".into(),
            rect: Rect {
                x: 40,
                y: 40,
                w: 320,
                h: 140,
            },
            label: "SPEED".into(),
            decimals: 1,
            font_size: 72.0,
        }],
    };
    let samples = vec![Sample {
        t: Duration::ZERO,
        lat: 0.0,
        lon: 0.0,
        altitude_m: None,
        speed_mps: Some(42.5 / 3.6), // 42.5 km/h
        heart_rate_bpm: None,
        cadence_rpm: None,
        power_w: None,
        distance_m: None,
        elev_gain_cum_m: None,
        gradient_pct: None,
    }];
    let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);

    let mut ctx = TextCtx::new();
    let mut pix = Pixmap::new(400, 200).unwrap();
    render_frame(&layout, &activity, Duration::ZERO, &mut ctx, &mut pix).unwrap();

    assert_golden(&pix, "readout_speed.png");
}

#[test]
fn course_widget_matches_golden() {
    let layout = Layout {
        version: 1,
        canvas: Canvas {
            width: 300,
            height: 300,
            fps: 30,
        },
        units: Units {
            speed: SpeedUnit::Kmh,
            distance: DistanceUnit::Km,
            elevation: ElevationUnit::M,
            temp: TempUnit::C,
        },
        theme: Theme {
            font: "Inter".into(),
            fg: "#ffffff".into(),
            accent: "#ffcc00".into(),
            shadow: None,
        },
        widgets: vec![Widget::Course {
            id: "map".into(),
            rect: Rect {
                x: 20,
                y: 20,
                w: 260,
                h: 260,
            },
            line_width: 3.0,
            dot_radius: 8.0,
        }],
    };

    // Simple rectangle course: 5 points forming a closed square.
    let samples = vec![
        mk_sample(0, 0.0, 0.0),
        mk_sample(10, 0.0, 0.001),
        mk_sample(20, 0.001, 0.001),
        mk_sample(30, 0.001, 0.0),
        mk_sample(40, 0.0, 0.0),
    ];
    let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);

    // Dot lands near the midpoint of the polyline by total time.
    let t = Duration::from_secs(20);

    let mut ctx = TextCtx::new();
    let mut pix = Pixmap::new(300, 300).unwrap();
    render_frame(&layout, &activity, t, &mut ctx, &mut pix).unwrap();

    assert_golden(&pix, "course_mid.png");
}

#[test]
fn elevation_profile_matches_golden() {
    let layout = Layout {
        version: 1,
        canvas: Canvas {
            width: 500,
            height: 120,
            fps: 30,
        },
        units: Units {
            speed: SpeedUnit::Kmh,
            distance: DistanceUnit::Km,
            elevation: ElevationUnit::M,
            temp: TempUnit::C,
        },
        theme: Theme {
            font: "Inter".into(),
            fg: "#ffffff".into(),
            accent: "#ffcc00".into(),
            shadow: None,
        },
        widgets: vec![Widget::ElevationProfile {
            id: "elev".into(),
            rect: Rect {
                x: 20,
                y: 20,
                w: 460,
                h: 80,
            },
        }],
    };

    // Triangle elevation: rises 0->100m over 0->500m distance, then back down
    // to 0m at distance 1000m. The marker at t=10 lands at distance ~500m, on
    // the peak.
    let mut samples: Vec<Sample> = Vec::new();
    for i in 0..=20 {
        let d = i as f64 * 50.0;
        let a = if i <= 10 {
            i as f32 * 10.0
        } else {
            (20 - i) as f32 * 10.0
        };
        samples.push(Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0,
            lon: 0.0,
            altitude_m: Some(a),
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_w: None,
            distance_m: Some(d),
            elev_gain_cum_m: None,
            gradient_pct: None,
        });
    }
    let activity = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);

    let mut ctx = TextCtx::new();
    let mut pix = Pixmap::new(500, 120).unwrap();
    render_frame(&layout, &activity, Duration::from_secs(10), &mut ctx, &mut pix).unwrap();

    assert_golden(&pix, "elev_mid.png");
}

fn assert_golden(pix: &Pixmap, name: &str) {
    let golden_path = Path::new("tests/golden").join(name);
    let actual_img: RgbaImage =
        RgbaImage::from_raw(pix.width(), pix.height(), pix.data().to_vec())
            .expect("pixmap -> image");

    if !golden_path.exists() {
        // First run — write the golden for the user to inspect and commit.
        std::fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        actual_img.save(&golden_path).unwrap();
        panic!(
            "wrote new golden at {:?}; re-run the test to confirm it matches",
            golden_path
        );
    }

    let expected = image::open(&golden_path).expect("load golden").to_rgba8();
    assert_eq!(expected.width(), actual_img.width());
    assert_eq!(expected.height(), actual_img.height());

    // Per-channel tolerance of 2.
    let mut diff_count = 0;
    for (a, b) in actual_img.pixels().zip(expected.pixels()) {
        for i in 0..4 {
            let d = (a.0[i] as i32 - b.0[i] as i32).abs();
            if d > 2 {
                diff_count += 1;
            }
        }
    }
    if diff_count > 0 {
        // Save actual for inspection.
        let actual_path = golden_path.with_extension("actual.png");
        actual_img.save(&actual_path).unwrap();
        panic!(
            "golden mismatch on {:?}: {} channels exceed tolerance. Wrote {:?}",
            golden_path, diff_count, actual_path
        );
    }
}
