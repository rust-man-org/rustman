# Windows Release Memory Optimization

User request: reduce Rustman RAM usage. Local commit/integration into development authorized 2026-11-04; no push.

## Evidence and design

- Profile the existing release binary with identical copies of the nine-request
  fixture. GPU process loads d3d12/Vulkan/NVIDIA modules; software does not.
- Initial visible-window baseline: GPU about 162 MiB private working set;
  software about 12 MiB before editor warmup. Record editor-warmed and rebuilt
  measurements before stating the final saving. This is a local sample, not
  a guarantee for other machines or large responses.
- Use existing iced renderer selection; no fork or dependency is needed.
  On Windows prefer `ICED_BACKEND=tiny-skia,wgpu`: software first, GPU fallback.
  Preserve explicit ICED_BACKEND, WGPU_BACKEND, WGPU_ADAPTER_NAME and WGPU_POWER_PREF.
- Keep existing Linux integrated-GPU defaults and all request/data behavior.
- Software rendering trades lower GPU setup cost for CPU work during repaint;
  verify idle CPU, JSON editor rendering/editing, and the search overlay.
  Users can select `ICED_BACKEND=wgpu` for graphics-heavy use.

## Plan and gate

- [X] Reproduce backend allocation difference before changing source.
- [X] Failing startup-policy regression, then implement the default and overrides.
- [X] Both locked all-target test suites and no-default check.
- [X] Build release; launch without renderer overrides on a copied fixture.
- [X] Repeat private/total working-set samples after warmup and rendering.
- [X] Record measured limits and update progress; integrate locally after user authorization.

## Verified result (2026-11-04)

Default startup without overrides: 18.46 MiB private working set, 40.36 MiB
working set, 20.54 MiB private commit, CPU idle 0%, five stable samples. Fixture:
nine requests, one restored tab with a 39-byte JSON body. No GPU driver modules
loaded. Source evidence: `target/memory-optimized-release.json` and
`target/memory-release-build.log`. Tests: 227 default / 219 no-default passed,
three live-network tests ignored each; no-default all-target check passed.

The earlier controlled backend A/B measured about 162 MiB private with GPU and
12 MiB with software before editor warmup; software reached 14 MiB after actual
JSON editing. The final simultaneous GPU rerun exited before sampling, so the
reported baseline comes from the earlier controlled run. Do not treat this as
an exhaustive workload benchmark. Software repaint can cost more CPU; explicit
GPU selection is preserved. User authorized local commit/integration after measurement; no remote push.
