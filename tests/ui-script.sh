#!/usr/bin/env sh
# Test the UI launcher with a fake Cargo and fake binaries. No app is started.
set -eu
repo_dir=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
mkdir -p "$repo_dir/target"
scratch_dir=$(mktemp -d "$repo_dir/target/ui-script-tests.XXXXXX")
# Only remove the test directory created inside this repository's target folder.
case "$scratch_dir" in "$repo_dir"/target/ui-script-tests.*) ;; *) exit 1 ;; esac
trap 'rm -rf -- "$scratch_dir"' EXIT HUP INT TERM
mkdir -p "$scratch_dir/scripts" "$scratch_dir/bin" "$scratch_dir/target/debug" "$scratch_dir/target/release"
cp "$repo_dir/scripts/ui.sh" "$scratch_dir/scripts/ui.sh"
cat > "$scratch_dir/bin/cargo" <<'MOCK'
#!/usr/bin/env sh
printf '%s\n' "$*" >> "$UI_TEST_LOG"
if [ "${UI_FAIL_TEST:-0}" = 1 ] && [ "$1" = test ]; then exit 1; fi
MOCK
cat > "$scratch_dir/target/debug/rustman" <<'MOCK'
#!/usr/bin/env sh
printf 'launched=1\nbackend=%s\ndata=%s\nsample=%s\n' "${ICED_BACKEND-unset}" "${RUSTMAN_DATA_DIR-unset}" "${RUSTMAN_SAMPLE-0}"
MOCK
cp "$scratch_dir/target/debug/rustman" "$scratch_dir/target/release/rustman"
chmod +x "$scratch_dir/bin/cargo" "$scratch_dir/target/debug/rustman" "$scratch_dir/target/release/rustman"
export PATH="$scratch_dir/bin:$PATH"
export UI_TEST_LOG="$scratch_dir/cargo.log"
assert_has() { printf '%s\n' "$1" | grep -F -- "$2" >/dev/null || { printf 'FAIL: expected %s\n' "$2"; exit 1; }; }

: > "$UI_TEST_LOG"
output=$(ICED_BACKEND= sh "$scratch_dir/scripts/ui.sh" -r -x)
assert_has "$output" 'backend=wgpu,tiny-skia'
assert_has "$output" 'data=target/ui-example'
assert_has "$output" 'sample=1'
assert_has "$(cat "$UI_TEST_LOG")" 'build --release --locked'
printf 'PASS: release launcher defaults to GPU and supports example data\n'

output=$(ICED_BACKEND=tiny-skia sh "$scratch_dir/scripts/ui.sh" -d "$scratch_dir/data with spaces")
assert_has "$output" 'backend=tiny-skia'
assert_has "$output" "data=$scratch_dir/data with spaces"
printf 'PASS: renderer override and data paths with spaces are preserved\n'

: > "$UI_TEST_LOG"
output=$(sh "$scratch_dir/scripts/ui.sh" -t)
assert_has "$(cat "$UI_TEST_LOG")" 'test --locked --all-targets'
assert_has "$(cat "$UI_TEST_LOG")" 'test --locked --no-default-features --all-targets'
assert_has "$(cat "$UI_TEST_LOG")" 'check --locked --no-default-features --all-targets'
assert_has "$(cat "$UI_TEST_LOG")" 'build --locked'
assert_has "$output" 'launched=1'
printf 'PASS: test mode runs both feature sets and the check gate\n'

: > "$UI_TEST_LOG"
if output=$(UI_FAIL_TEST=1 sh "$scratch_dir/scripts/ui.sh" -t 2>&1); then
    printf 'FAIL: launcher ignored a failed test\n'; exit 1
fi
if printf '%s\n' "$output" | grep -F 'launched=1' >/dev/null; then
    printf 'FAIL: app launched after a failed test\n'; exit 1
fi
if grep -F 'build' "$UI_TEST_LOG" >/dev/null; then
    printf 'FAIL: build ran after a failed test\n'; exit 1
fi
printf 'PASS: a failed test prevents build and launch\n'
