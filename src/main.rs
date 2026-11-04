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
    prefer_memory_efficient_renderer_on_windows();
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

/// Prefer the built-in software renderer on Windows. Native profiling showed
/// that initializing the GPU backend dominates idle memory for this UI.
/// Keep GPU support as a fallback and respect explicit renderer/adapter choices.
fn prefer_memory_efficient_renderer_on_windows() {
    #[cfg(target_os = "windows")]
    {
        let backend = std::env::var_os("ICED_BACKEND");
        let gpu_requested = ["WGPU_BACKEND", "WGPU_ADAPTER_NAME", "WGPU_POWER_PREF"]
            .iter()
            .any(|key| std::env::var_os(key).is_some());
        if let Some(default) = memory_backend_default("windows", backend.as_deref(), gpu_requested)
        {
            // SAFETY: main calls this before starting threads or initializing iced.
            unsafe {
                std::env::set_var("ICED_BACKEND", default);
            }
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn memory_backend_default(
    platform: &str,
    backend_override: Option<&std::ffi::OsStr>,
    gpu_requested: bool,
) -> Option<&'static str> {
    (platform == "windows" && backend_override.is_none() && !gpu_requested)
        .then_some("tiny-skia,wgpu")
}

#[cfg(test)]
mod renderer_policy_tests {
    use super::memory_backend_default;
    use std::ffi::OsStr;
    #[test]
    fn windows_prefers_software_with_gpu_fallback() {
        assert_eq!(
            memory_backend_default("windows", None, false),
            Some("tiny-skia,wgpu")
        );
    }
    #[test]
    fn explicit_renderer_and_gpu_preferences_are_preserved() {
        for backend in ["wgpu", "tiny-skia", ""] {
            assert_eq!(
                memory_backend_default("windows", Some(OsStr::new(backend)), false),
                None
            );
        }
        assert_eq!(memory_backend_default("windows", None, true), None);
    }
    #[test]
    fn other_platforms_keep_their_existing_renderer_default() {
        for platform in ["linux", "macos"] {
            assert_eq!(memory_backend_default(platform, None, false), None);
        }
    }
}
