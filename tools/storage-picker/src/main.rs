// A separate process puts AppKit's dialog on the main thread on macOS.
fn main() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        let mtm=objc2::MainThreadMarker::new().expect("Picker runs on the main thread");
        let app=NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.finishLaunching();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
    let mut dialog = rfd::FileDialog::new().set_title("STRATOS data directory");
    if let Some(path) = std::env::args_os().nth(1) { dialog = dialog.set_directory(path); }
    let path = dialog.pick_folder();
    println!("{}", serde_json::json!({"path":path.map(|p| p.to_string_lossy().into_owned())}));
}
