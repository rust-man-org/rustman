# 07 — Self-Update Integrity (minisign + SHA-256)

**Master doc:** — (security hardening; not part of the company roadmap).
**Owner files:** `src/services/update.rs`, `.github/workflows/release.yml`,
`Cargo.toml` (`self-update` feature).

## Objective

Before a downloaded self-update replaces the running executable, verify it:
check a **minisign** signature over a `SHA256SUMS` manifest against an embedded
public key, then check the downloaded asset's **SHA-256** against that signed
manifest, and fail closed. Pure Rust end to end, no C toolchain.

This is cross-cutting hardening — it hardens the app's own release pipeline, not
a company-edition feature. It is included in this set because it belongs to the
same "security" grouping as 05, but it has no dependency on the master-doc
roadmap.

## Codebase anchors

- `src/services/update.rs` — `install()` downloads the release asset and hands it
  to `extract_and_replace()`, whose only check is `verify_binary()` (a 4-byte
  ELF/Mach-O/PE magic sniff). No hash, no signature. `restart()` strips the macOS
  quarantine flag (`xattr -dr`) and spawns the binary. `pick_asset()` is the
  existing asset finder; `client()` reads `GITHUB_TOKEN`/`GH_TOKEN`.
- `.github/workflows/release.yml` — jobs `prepare`, `build-linux-iced`,
  `build-macos-iced`, `build-windows-iced`; publishes no checksums or signatures.
- `Cargo.toml` — the `self-update` feature (line 22) gates the updater; `sha2`
  is already a direct dependency. `minisign-verify` (Frank Denis,
  zero-dependency) is the verifier crate.

## Interface contract

- **Consumes:** none from `00`.
- **Produces:** none shared (terminal workstream).
- **Owns:** `src/services/update.rs`, `.github/workflows/release.yml`, and the
  `self-update` feature in `Cargo.toml` (shared file — additive edits only).

## Deliverables

1. A `sign-checksums` job in `release.yml` producing `SHA256SUMS` +
   `SHA256SUMS.minisig`.
2. `verify_asset` in `update.rs`, called before `extract_and_replace`.
3. Optional `minisign-verify` dependency under the `self-update` feature.
4. Key management (embedded public key + CI secret).

## Ordered tasks

1. **Release job.** Add a `sign-checksums` job that `needs:` the three build
   jobs (so every asset is uploaded), then:
   - `gh release download <tag> -D dist`;
   - `cd dist && sha256sum * > SHA256SUMS` (exclude `SHA256SUMS*` itself);
   - `minisign -Sm SHA256SUMS -s minisign.key -x SHA256SUMS.minisig`
     (passwordless CI key created with `minisign -G -W`);
   - upload `SHA256SUMS` and `SHA256SUMS.minisig` via
     `softprops/action-gh-release@v2`.
   Repo secret `MINISIGN_SECRET_KEY` (the key file contents); install `minisign`
   on the ubuntu runner (`apt-get install -y minisign`).
2. **Cargo.** Add optional `minisign-verify = "0.2"` and `"dep:minisign-verify"`
   to the `self-update` feature so `--no-default-features` builds do not pull it.
3. **Client.** Add `const MINISIGN_PUBKEY: &str = "<base64 public key>";` and
   `async fn verify_asset(client, assets, asset_name, bytes) -> Result<(), String>`:
   - locate assets named **exactly** `SHA256SUMS` and `SHA256SUMS.minisig`;
   - download both;
   - `PublicKey::from_base64(MINISIGN_PUBKEY)?`,
     `Signature::decode(&minisig_text)?`, then
     `public_key.verify(&sums_bytes, &signature, false)?` — **data first,
     signature second**; `allow_legacy = false` for standard pre-hashed
     signatures (what `minisign -S` produces);
   - parse `SHA256SUMS` for the line whose filename matches `asset.name`, take
     the hex digest, compute `Sha256::digest(&bytes)`, compare;
   - fail closed with "update signature/hash verification failed — refusing to
     install" on any mismatch or missing asset.
4. **Wire in.** Call `verify_asset(...)` on the downloaded `bytes` **before**
   `extract_and_replace`. Keep `verify_binary` as a secondary sanity check.
5. **macOS.** Leave the `xattr` strip in `restart`, now only reachable after a
   verified install (already the case via the Install → Ready → Restart
   sequence); add a comment tying it to verification. Optional follow-up (needs
   Apple Developer credentials, out of scope): `codesign` + notarize the `.app`
   and drop the `xattr` call.

## Key management

- Generate once: `minisign -G -W -p rustman.pub -s minisign.key`.
- Embed `rustman.pub`'s base64 line as `MINISIGN_PUBKEY`.
- Store `minisign.key` as the `MINISIGN_SECRET_KEY` repo secret; never commit it.
- Document rotation (bump the embedded pubkey + re-issue).

## Out of scope

- Panic hardening and general CI supply-chain pinning.
- macOS notarization (needs Apple Developer credentials).

## Verification

- `cargo test` green; add tests for `SHA256SUMS` line parsing (picks the right
  asset, ignores others) and rejection of a tampered hash/signature (fixture
  keypair, or the crate's published test vector).
- `cargo check --no-default-features --all-targets` still compiles (dep gated
  out).
- Dry-run `release.yml` via `workflow_dispatch`; confirm `SHA256SUMS` +
  `SHA256SUMS.minisig` attach to the release.
- Manual (or a temporary `#[ignore]`d test): point the client at a release whose
  asset bytes were altered and confirm `install()` refuses.

## Risks

- Adds a release-time key + secret; a lost key means users must update manually
  until a new key ships. Documented.
- `--no-default-features` builds (distro packages) get none of this — correct,
  since the package manager owns updates there.
