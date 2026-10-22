pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let _force_windowed = args
        .iter()
        .any(|arg| arg == "--windowed" || arg == "--fallback");

    #[cfg(all(target_os = "linux", feature = "wayland"))]
    {
        let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();

        if is_wayland && !_force_windowed {
            println!("[minibro] Launching Wayland Layer Shell notch...");
            infrastructure::runner_wayland::run()?;
            return Ok(());
        }
    }

    println!("[minibro] Launching standard window runner...");
    infrastructure::runner_fallback::run()?;
    Ok(())
}
