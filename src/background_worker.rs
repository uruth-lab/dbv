// TODO 3: Remove this file and just use the tokio macro
#[cfg(not(target_arch = "wasm32"))]
pub fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Unable to create Runtime")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start_background_worker(rt: tokio::runtime::Runtime) {
    // Execute the runtime in its own thread.
    std::thread::spawn(move || {
        log::info!("Background worker started");
        rt.block_on(async {
            loop {
                // Can use this loop for background tasks
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            }
        })
    });
}
