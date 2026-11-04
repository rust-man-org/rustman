#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod consts;
mod domain;
mod jobs;
mod message;
mod services;
mod state;
mod ui;

fn main() -> iced::Result {
    prefer_integrated_gpu_on_linux();
    app::run()
}

/// Default to the integrated GPU on Linux unless the user says otherwise.
///
/// `iced_wgpu` asks wgpu for `PowerPreference::HighPerformance` unless
/// `WGPU_POWER_PREF` is set (see `iced_wgpu`'s `window::compositor`, which calls
/// `PowerPreference::from_env().unwrap_or(HighPerformance)`). On a hybrid laptop
/// that selects the *discrete* GPU, and on the common Linux setup where that
/// card is driven by the open-source `nouveau` driver, presenting to a Wayland
/// surface intermittently kills the Vulkan device:
///
/// ```text
/// Error in Surface::present: Validation Error
///   Parent device is lost
/// ```
///
/// wgpu panics on that, and the panic then re-enters wgpu's own `Drop`
/// (`SurfaceAcquireSemaphores ... still in use`), which aborts the process — so
/// it is not something the app can catch and recover from. Reproduced on an
/// Intel Iris Xe + RTX 3060 (nouveau) machine at roughly one launch in three;
/// forcing the integrated GPU ran indefinitely without a single failure.
///
/// A UI-heavy API client is nowhere near GPU-bound, so the integrated GPU is
/// also simply the better default here: less power, no discrete-GPU wake-up,
/// and on hybrid laptops it owns the display outputs anyway, avoiding a
/// cross-GPU present path entirely.
///
/// This only sets a *default*: an explicit `WGPU_POWER_PREF` (or
/// `WGPU_ADAPTER_NAME`) from the environment is respected untouched, so anyone
/// who wants the discrete GPU can still ask for it with
/// `WGPU_POWER_PREF=high cargo run`.
fn prefer_integrated_gpu_on_linux() {
    #[cfg(target_os = "linux")]
    {
        // Never override a deliberate choice.
        let already_chosen = std::env::var_os("WGPU_POWER_PREF").is_some()
            || std::env::var_os("WGPU_ADAPTER_NAME").is_some();

        if !already_chosen {
            // SAFETY: called from the very start of `main`, before any threads
            // exist and before wgpu reads this variable, so there is no
            // concurrent access to the environment.
            unsafe {
                std::env::set_var("WGPU_POWER_PREF", "low");
            }
        }
    }
}
