#!/usr/bin/env sh
# Build Rustman and start the app. The default renderer uses the GPU.
#
#   scripts/ui.sh                 build (debug) + run with your normal data
#   scripts/ui.sh -t              run tests and checks first; stop on failure
#   scripts/ui.sh -r              release build (slower build, faster app);
#                                 output target/release/rustman(.exe)
#   scripts/ui.sh -b              build only, do not start the app
#                                 (release binary: scripts/ui.sh -r -b)
#   scripts/ui.sh -d DIR          use DIR as the data folder instead of your
#                                 real one (created if missing)
#   scripts/ui.sh -s              sample mode: works on a copy of your data
#                                 (target/ui-data/), the real data stays as is
#   scripts/ui.sh -x              example workspace (target/ui-example/): a
#                                 "Sample API" collection with folders and
#                                 working requests + a "Sample" environment.
#                                 Delete that folder to get a fresh one.
#   scripts/ui.sh -g MODE         renderer: gpu (default), gpu-high or software
#
# Renderer modes (-g). RAM numbers: release build, idle, Windows laptop
# with AMD iGPU + NVIDIA, Task Manager "Memory" (2026-11-05).
#
#   gpu       GPU on the integrated card (Windows: DX12 only).
#             + Smooth scrolling and repaint, CPU stays idle
#             + Integrated GPU: saves battery, no discrete-GPU wake-up
#             + Falls back to software when no GPU is usable
#             - About 80-100 MB RAM (graphics driver and buffers)
#   gpu-high  GPU on the discrete card (WGPU_POWER_PREF=high).
#             + Most graphics headroom on very large responses
#             - Wakes the discrete GPU: more power, less battery
#             - About 75-90 MB RAM, no real gain for a UI app like this
#   software  CPU rendering (tiny-skia), no graphics driver.
#             + Smallest RAM, about 13 MB
#             + No GPU driver issues (old drivers, remote desktop, VMs)
#             - Repaint uses the CPU: more CPU work while scrolling or
#               typing in big bodies
#             - Less smooth on large screens or high DPI
#
# An ICED_BACKEND / WGPU_* value from your environment wins when -g is not
# given. Combine flags: sh scripts/ui.sh -t -r -x -g software
# Works in Git Bash (Windows), Linux and macOS.

set -eu

cd "$(dirname "$0")/.."

profile=debug
run_tests=0
build_only=0
data_dir=""
sample=0
example=0
renderer=""

# Help = the comment block at the top of this file.
show_help() {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
}

while getopts "trbd:sxg:h" opt; do
    case "$opt" in
        t) run_tests=1 ;;
        r) profile=release ;;
        b) build_only=1 ;;
        d) data_dir="$OPTARG" ;;
        s) sample=1 ;;
        x) example=1 ;;
        g) renderer="$OPTARG" ;;
        h | *)
            show_help
            exit 0
            ;;
    esac
done

# Renderer: -g wins; without it keep the environment, else GPU first with
# software as the fallback (the app picks DX12 + integrated GPU on Windows).
case "$renderer" in
    "")
        export ICED_BACKEND="${ICED_BACKEND:-wgpu,tiny-skia}"
        renderer_label="gpu (default)"
        ;;
    gpu)
        export ICED_BACKEND="wgpu,tiny-skia"
        unset WGPU_POWER_PREF 2>/dev/null || true
        renderer_label="gpu (integrated)"
        ;;
    gpu-high)
        export ICED_BACKEND="wgpu,tiny-skia" WGPU_POWER_PREF="high"
        renderer_label="gpu-high (discrete)"
        ;;
    software)
        export ICED_BACKEND="tiny-skia"
        renderer_label="software (CPU)"
        ;;
    *)
        echo "unknown renderer: $renderer (use gpu, gpu-high or software)" >&2
        exit 2
        ;;
esac

# Optional: build temp files on another drive when the system one is full,
# e.g. RUSTMAN_TMP=D:/tmp-rustman scripts/ui.sh
if [ -n "${RUSTMAN_TMP:-}" ]; then
    mkdir -p "$RUSTMAN_TMP"
    export TMP="$RUSTMAN_TMP" TEMP="$RUSTMAN_TMP" TMPDIR="$RUSTMAN_TMP"
fi

if [ "$run_tests" = 1 ]; then
    echo "==> cargo test --locked --all-targets"
    cargo test --locked --all-targets
    echo "==> cargo test --locked --no-default-features --all-targets"
    cargo test --locked --no-default-features --all-targets
    echo "==> cargo check --locked --no-default-features --all-targets"
    cargo check --locked --no-default-features --all-targets
fi

echo "==> cargo build ($profile)"
if [ "$profile" = release ]; then
    cargo build --release --locked
else
    cargo build --locked
fi

exe="target/$profile/rustman"
[ -f "$exe.exe" ] && exe="$exe.exe"
echo "==> built $exe ($(du -h "$exe" | cut -f1))"

if [ "$build_only" = 1 ]; then
    echo "==> build only (-b): not starting the app"
    exit 0
fi

if [ "$sample" = 1 ]; then
    # Snapshot the real data once; later runs keep the sample's changes.
    # Delete target/ui-data/ to start over from a fresh copy.
    data_dir="target/ui-data"
    if [ ! -d "$data_dir" ]; then
        case "$(uname -s)" in
            MINGW* | MSYS* | CYGWIN*) real="${APPDATA:-}/rustman" ;;
            Darwin) real="$HOME/Library/Application Support/rustman" ;;
            *) real="${XDG_DATA_HOME:-$HOME/.local/share}/rustman" ;;
        esac
        mkdir -p "$data_dir"
        if [ -d "$real" ]; then
            echo "==> copying $real -> $data_dir"
            cp -R "$real/." "$data_dir/"
        fi
    fi
fi

if [ "$example" = 1 ]; then
    # The app seeds the example only into a folder without collections.
    data_dir="target/ui-example"
    export RUSTMAN_SAMPLE=1
fi

if [ -n "$data_dir" ]; then
    mkdir -p "$data_dir"
    export RUSTMAN_DATA_DIR="$data_dir"
    echo "==> data folder: $data_dir"
else
    unset RUSTMAN_DATA_DIR 2>/dev/null || true
    echo "==> data folder: your normal Rustman data"
fi

echo "==> renderer: $renderer_label (ICED_BACKEND=$ICED_BACKEND)"
echo "==> launching $exe"
exec "$exe"
