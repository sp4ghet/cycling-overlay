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
