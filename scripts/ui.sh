#!/usr/bin/env sh
# Build Rustman and launch it to try the current UI.
#
#   scripts/ui.sh                 build (debug) + run with your normal data
#   scripts/ui.sh -t              run both test suites first, stop if red
#   scripts/ui.sh -r              release build (slower build, faster app)
#   scripts/ui.sh -d DIR          use DIR as the data folder instead of your
#                                 real one (created if missing)
#   scripts/ui.sh -s              sample mode: works on a copy of your data
#                                 (target/ui-data/), the real data stays as is
#
# Flags combine: scripts/ui.sh -t -s
# Works in Git Bash (Windows), Linux and macOS.

set -eu

cd "$(dirname "$0")/.."

profile=debug
run_tests=0
data_dir=""
sample=0

while getopts "trd:sh" opt; do
    case "$opt" in
        t) run_tests=1 ;;
        r) profile=release ;;
        d) data_dir="$OPTARG" ;;
        s) sample=1 ;;
        h | *)
            sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
    esac
done

# Optional: build temp files on another drive when the system one is full,
# e.g. RUSTMAN_TMP=D:/tmp-rustman scripts/ui.sh
if [ -n "${RUSTMAN_TMP:-}" ]; then
    mkdir -p "$RUSTMAN_TMP"
    export TMP="$RUSTMAN_TMP" TEMP="$RUSTMAN_TMP" TMPDIR="$RUSTMAN_TMP"
fi

if [ "$run_tests" = 1 ]; then
    echo "==> cargo test"
    cargo test
    echo "==> cargo test --no-default-features"
    cargo test --no-default-features
fi

echo "==> cargo build ($profile)"
if [ "$profile" = release ]; then
    cargo build --release
else
    cargo build
fi

exe="target/$profile/rustman"
[ -f "$exe.exe" ] && exe="$exe.exe"

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

if [ -n "$data_dir" ]; then
    mkdir -p "$data_dir"
    export RUSTMAN_DATA_DIR="$data_dir"
    echo "==> data folder: $data_dir"
else
    unset RUSTMAN_DATA_DIR 2>/dev/null || true
    echo "==> data folder: your normal Rustman data"
fi

echo "==> launching $exe"
exec "$exe"
