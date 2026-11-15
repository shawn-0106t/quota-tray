// Snapshot CLI for the Slint spike: render one window with the same sample data
// the HTML reference paints, then write the window's own pixels to a PNG.
//
//   slint-demo --theme dark --tab ds --renderer skia --scale 2 --out shot.png
//
// `--renderer` maps to SLINT_BACKEND before Slint initialises; `--stage none`
// leaves the window transparent so the real shadow-on-desktop can be checked.

slint::include_modules!();

use std::time::Duration;

fn arg(args: &[String], name: &str, default: &str) -> String {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn save_png(
    buffer: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
    path: &str,
) -> Result<(), String> {
    let mut bytes = Vec::with_capacity((buffer.width() * buffer.height() * 4) as usize);
    for pixel in buffer.as_slice() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
    let image = image::RgbaImage::from_raw(buffer.width(), buffer.height(), bytes)
        .ok_or_else(|| "pixel buffer does not match its size".to_string())?;
    image.save(path).map_err(|e| e.to_string())
}

fn models() -> Vec<ModelRow> {
    vec![
        ModelRow {
            name: "DeepSeek-V4.1-Flash".into(),
            cost: "¥28.50".into(),
            share: 1.0,
            tokens: "665.8M".into(),
            hit: "cache 99.6%".into(),
            hit_bad: false,
            today: "today ¥4.53".into(),
        },
        ModelRow {
            name: "DeepSeek-V4.1-Pro".into(),
            cost: "¥12.88".into(),
            share: 0.2766,
            tokens: "184.2M".into(),
            hit: "cache 71.3%".into(),
            hit_bad: false,
            today: "today ¥1.90".into(),
        },
        ModelRow {
            name: "DeepSeek-V3.2".into(),
            cost: "¥2.13".into(),
            share: 0.0619,
            tokens: "41.2M".into(),
            hit: "cache 58.4%".into(),
            hit_bad: true,
            today: "today ¥0.30".into(),
        },
        ModelRow {
            name: "DeepSeek-V4.1-Reasoner-Pro-Max-Extended".into(),
            cost: "¥0.88".into(),
            share: 0.0136,
            tokens: "9.03M".into(),
            hit: "cache --".into(),
            hit_bad: false,
            today: "today ¥0.11".into(),
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let theme = arg(&args, "--theme", "light");
    let tab = arg(&args, "--tab", "kimi");
    let renderer = arg(&args, "--renderer", "wgpu");
    let scale = arg(&args, "--scale", "2");
    let stage = arg(&args, "--stage", "on");
    let window = arg(&args, "--window", "panel");
    let out = arg(&args, "--out", "shot.png");
    let blur: f32 = arg(&args, "--blur", "18.0").parse().unwrap_or(18.0);

    // Both must be set before the backend is created by the first component.
    std::env::set_var("SLINT_SCALE_FACTOR", &scale);
    match renderer.as_str() {
        "wgpu" => std::env::set_var("SLINT_BACKEND", "winit-femtovg-wgpu"),
        "gl" => std::env::set_var("SLINT_BACKEND", "winit-femtovg"),
        "skia" => std::env::set_var("SLINT_BACKEND", "winit-skia"),
        other => return Err(format!("unknown renderer: {other} (wgpu | gl | skia)").into()),
    }

    let dark = theme == "dark";
    let stage_visible = stage != "none";

    let window_kind = window;
    if window_kind == "settings" {
        let ui = SettingsSheet::new()?;
        ui.global::<Tokens>().set_dark(dark);
        ui.set_dark(dark);
        ui.set_stage_visible(stage_visible);
        ui.show()?;
        run_and_snapshot(ui, &out)?;
    } else {
        let ui = Panel::new()?;
        ui.global::<Tokens>().set_dark(dark);
        ui.set_dark(dark);
        ui.set_stage_visible(stage_visible);
        ui.set_tab_ds(tab == "ds");
        ui.set_shadow_blur(blur);
        ui.set_models(slint::ModelRc::new(slint::VecModel::from(models())));
        ui.show()?;
        run_and_snapshot(ui, &out)?;
    }
    Ok(())
}

fn run_and_snapshot<T: slint::ComponentHandle + 'static>(
    ui: T,
    out: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let weak = ui.as_weak();
    let out = out.to_string();
    slint::Timer::single_shot(Duration::from_millis(900), move || {
        if let Some(ui) = weak.upgrade() {
            let result = {
                let window = ui.window();
                let size = window.size();
                println!("window: {}x{} px", size.width, size.height);
                window.take_snapshot()
            };
            match result {
                Ok(buffer) => match save_png(&buffer, &out) {
                    Ok(()) => println!("written: {out} ({}x{})", buffer.width(), buffer.height()),
                    Err(e) => eprintln!("save failed: {e}"),
                },
                Err(e) => eprintln!("snapshot failed: {e}"),
            }
            ui.hide().ok();
        }
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()?;
    Ok(())
}
