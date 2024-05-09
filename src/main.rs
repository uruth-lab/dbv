#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let rt = dbv::background_worker::create_runtime();
    let _enter = rt.enter(); // This Guard must be held to call `tokio::spawn` anywhere in the program
    dbv::background_worker::start_background_worker(rt);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 800.0])
            .with_min_inner_size([300.0, 220.0]) // TODO 3: Test if these sizes make sense
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/favicon.ico")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    // TODO 4: Find a way to delete saved data and not save on that close to get back to defaults
    eframe::run_native(
        "DBV - Data Builder Viewer",
        native_options,
        Box::new(|cc| Box::new(dbv::DBV::new(cc))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| Box::new(dbv::DBV::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
