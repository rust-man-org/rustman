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
    prefer_lean_gpu_on_windows();
    app::run()
}

/// Windows GPU defaults that keep RAM down (only when the user set nothing).
///
/// Unrestricted, wgpu brings up *every* backend on Windows: Vulkan and
/// DX12 both load their drivers (NVIDIA and AMD on a hybrid laptop), which
/// cost ~172 MB private working set at idle. DX12 alone measured ~74–80 MB on
/// the same fixture (release build, 2026-11-05); software rendering ~13 MB.
/// `low` power picks the integrated GPU like on Linux: less power, no
/// discrete-GPU wake-up. If DX12 is missing, wgpu finds no adapter and iced
/// falls back to the next `ICED_BACKEND` entry (tiny-skia by default).
fn prefer_lean_gpu_on_windows() {
    #[cfg(target_os = "windows")]
    for (key, value) in windows_gpu_defaults(|k| std::env::var_os(k).is_some()) {
        // SAFETY: called at the very start of `main`, before any threads
        // exist and before wgpu reads these variables.
        unsafe {
            std::env::set_var(key, value);
        }
    }
}

/// The env defaults to set, skipping any the user already chose.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn windows_gpu_defaults(is_set: impl Fn(&str) -> bool) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    if !is_set("WGPU_BACKEND") {
        out.push(("WGPU_BACKEND", "dx12"));
    }
    if !is_set("WGPU_POWER_PREF") && !is_set("WGPU_ADAPTER_NAME") {
        out.push(("WGPU_POWER_PREF", "low"));
    }
    out
}

#[cfg(test)]
mod gpu_default_tests {
    use super::windows_gpu_defaults;

    #[test]
    fn defaults_apply_only_when_unset() {
        assert_eq!(
            windows_gpu_defaults(|_| false),
            vec![("WGPU_BACKEND", "dx12"), ("WGPU_POWER_PREF", "low")]
        );
        assert!(windows_gpu_defaults(|_| true).is_empty());
        // A chosen adapter keeps its own power choice; the backend still
        // defaults to DX12.
        assert_eq!(
            windows_gpu_defaults(|k| k == "WGPU_ADAPTER_NAME"),
            vec![("WGPU_BACKEND", "dx12")]
        );
    }
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
