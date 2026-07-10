# 0.3.0 Release-Preparation Plan

*Drafted 2026-07-10. Scope: release hygiene + documentation reconciliation only.
No feature work remains. 0.3 ships **upstream-only** (stays on `rumqttc` 0.25.1
from crates.io). Verified against the repo at this date — see "Verified repo
state" for the facts each step relies on.*

**0.3 ships (DONE, on `main`):** §1 API de-leak, §5 backpressure ordering-fix +
knobs, §2b `ReceiveEvent` receive() shape, §2 `MessageMeta`, §4 connection-state
observability.

**Deferred to 0.4 (must be relabelled in the docs):** §3 typed SubAck surfacing,
§2c resubscribe-failure surfacing, §6 backend swap to `rumqttc-v4-next`. Reason:
clean addressable SubAck correlation requires the next backend (upstream rumqttc
cannot correlate SUBACK without a fragile pkid hack), and the audited next
backend is not yet on crates.io.

---

## Verified repo state (as of drafting)

- Working tree clean, on `main`, `origin/main..HEAD` empty (all 0.3 commits pushed).
- Current versions: root `0.2.0`, core `0.2.0`, macros `0.2.0`, **engine `0.1.1`**.
- Tags present: `v0.1.0`, `v0.2.0`, `mqtt-topic-engine-v0.1.1`.
- Engine changed this cycle: exactly one code commit since `v0.2.0` AND since
  `mqtt-topic-engine-v0.1.1` → `c05d383 feat(engine): derive Clone on TopicMatch`
  (`mqtt-topic-engine/src/topic_match.rs`). Additive: a new `#[derive(Clone)]`.
- `core/Cargo.toml` pins the engine path-dep at `version = "0.1.0"` (stale; engine
  is already `0.1.1` on crates.io).
- `core` dev-dep on macros is PATH-ONLY, no version: `mqtt-typed-client-macros =
  { path = "../macros" }` — correct, do NOT add a version (cycle trap).
- docs.rs metadata blocks present: root `all-features=true`, core `all-features=true`,
  engine `features=["rumqttc","ntex-mqtt"]`.
- `include_str!` targets (grepped): root `README.md`, `docs/COMPARISON_WITH_RUMQTTC.md`,
  `examples/README.md`, examples `000`–`010` + `100`; engine `README.md`. All must
  stay packaged. `exclude` does NOT cover any of these (it excludes `docs/ROADMAP.md`,
  `docs/research/`, `scripts/`, `examples/internal/`, `dev/`, CI/config files).
- Cargo.lock is gitignored (present locally) → all publish/verify commands use `--locked`.
- Pre-commit hook = fail-fast `cargo +nightly fmt --check`, does NOT `git add`.

---

## ⚠️ Gotchas carried from 0.2 (read before touching anything)

> - **Dev-dep cycle trap.** `core`'s dev-dep on `macros` MUST remain
>   `{ path = "../macros" }` with NO version. A versioned dev-dep forms an
>   unpublishable cycle (macros→core, core→macros) — this broke the first 0.2 publish.
> - **`exclude` vs `include_str!`.** The root crate packages from the repo root;
>   `exclude=[...]` is load-bearing (once leaked a `dev/certs` private key). But
>   excluding a directory also removes `include_str!` targets under it from the
>   package → a compile failure on the docs.rs / verify build. Every `include_str!`
>   target must stay inside the package.
> - **paho-mqtt native-lib trap.** `paho-mqtt` is an OPTIONAL dep of the engine that
>   links a native C library absent locally/CI. NEVER run `--workspace --all-features`
>   or `-p mqtt-topic-engine --all-features` for test/clippy/publish. Engine checks use
>   explicit `--features rumqttc,ntex-mqtt`. `--all-features` on the ROOT package is safe.
> - **Index propagation.** Between each `cargo publish` the next crate's dependency
>   must appear in the crates.io index first — pause and confirm before the next publish.
> - **Immutability.** A published version can never be overwritten or re-used. Get the
>   version numbers and CHANGELOG right BEFORE publishing.
> - **`--lib`+`--doc` cannot be combined** in one `cargo test` invocation — run separately.
> - **Cargo.lock is gitignored** → use `--locked`, and regenerate the lockfile
>   (`cargo build`/`cargo update -w`) AFTER version bumps or `--locked` will fail.

---

## 1. Pre-flight verification (read-only)

- [ ] `git status` → working tree clean.
- [ ] `git branch --show-current` → `main`.
- [ ] `git log origin/main..HEAD --oneline` → empty (all pushed).
- [ ] Confirm current versions: root/core/macros = `0.2.0`, engine = `0.1.1`
      (`grep -rn '^version' Cargo.toml core/Cargo.toml macros/Cargo.toml mqtt-topic-engine/Cargo.toml`).
- [ ] Sanity that the five features + connection-state are present:
      `git log v0.2.0..HEAD --oneline` — expect commits for de-leak (§1),
      routing FIFO fix (§5, `419beca`), `ReceiveEvent` (§2b, `d687ce8`),
      `MessageMeta` (§2, `89c383c`) incl. engine `derive Clone` (`c05d383`),
      connection-state (§4, `6c5de2a`) + zombie-consumer fix (`0d6ee3c`).

## 2. Version bumps

*Rationale: independent (non-lockstep) versioning. Root/core/macros → `0.3.0`.
Engine → `0.1.2` (patch: `#[derive(Clone)]` is additive/non-breaking — see OPEN
QUESTIONS for the patch-vs-minor call). Every internal path-dep `version` field
must be raised to match, so the published manifests pin the right floor.*

Exact edits (found via `grep -rn '0\.2\.0' … ` + the engine version):

- [ ] `Cargo.toml:12` — `version = "0.2.0"` → `"0.3.0"` (root package).
- [ ] `Cargo.toml:55` — `mqtt-typed-client-core = { path = "./core", version = "0.2.0" }` → `"0.3.0"`.
- [ ] `Cargo.toml:57` — `mqtt-typed-client-macros = { path = "./macros", version = "0.2.0", optional = true }` → `"0.3.0"`.
- [ ] `core/Cargo.toml:3` — `version = "0.2.0"` → `"0.3.0"`.
- [ ] `core/Cargo.toml` (engine dep line) — `mqtt-topic-engine = { path = "../mqtt-topic-engine", version = "0.1.0", … }` → **`version = "0.1.2"`**.
      *Rationale (REQUIRED, not optional — critic finding #2): the macro's by-value
      `topic`/`meta` codegen emits `Arc::unwrap_or_clone` (`macros/src/analysis.rs:142,328`),
      which needs `TopicMatch: Clone`. That derive exists only from engine 0.1.2
      (`topic_match.rs:92`, commit `c05d383`). A stale `0.1.0` floor lets a downstream
      lockfile resolve engine 0.1.0/0.1.1 → COMPILE ERROR in generated user code.*
- [ ] `core/Cargo.toml` **dev-dep** — LEAVE `mqtt-typed-client-macros = { path = "../macros" }` PATH-ONLY, NO version (cycle trap — explicit no-op check).
- [ ] `core/Cargo.toml:64` — comment text "core would require macros 0.2.0" — cosmetic; optionally update to 0.3.0 (comment only, fmt-safe).
- [ ] `macros/Cargo.toml:3` — `version = "0.2.0"` → `"0.3.0"`.
- [ ] `macros/Cargo.toml:21` — `mqtt-typed-client-core = { path = "../core", version = "0.2.0" }` → `"0.3.0"`.
- [ ] `macros/Cargo.toml:25` — dev-dep `mqtt-typed-client-core = { path = "../core", version = "0.2.0" }` → `"0.3.0"` (versioned here is fine — macros→core is the safe direction).
- [ ] `mqtt-topic-engine/Cargo.toml:3` — `version = "0.1.1"` → `"0.1.2"`.
- [ ] `README.md:56` — `mqtt-typed-client = "0.2.0"` → `"0.3.0"`.
- [ ] `README.md:171` — `mqtt-typed-client = { version = "0.2.0", features = […] }` → `"0.3.0"`.
- [ ] README badges (`README.md:9-13`) are dynamic (crates.io/docs.rs shields) — no change; MSRV badge stays `1.85.1` (confirm unchanged).
- [ ] There is NO `[workspace.package]` shared version and NO internal version pin in `[workspace.dependencies]` (only `rumqttc`) — nothing else to touch.
- [ ] After bumps: `cargo update -w` (or `cargo build`) to refresh the gitignored `Cargo.lock` so later `--locked` commands pass.

## 3. CHANGELOG finalization

*Root `CHANGELOG.md` `[Unreleased]` already contains all five features (verified):
API de-leak table + Added/Fixed/Changed(BREAKING), backpressure knobs +
`dropped_messages()`, `MessageMeta`, `ReceiveEvent`/`IncomingMessage`/`DecodeFailure`,
connection-state. No SubAck / §2c `SubscriptionLost` / backend-swap item is claimed
as shipped — confirm this stays true after edits.*

- [ ] `CHANGELOG.md:8` — `## [Unreleased]` → `## [0.3.0] - <RELEASE-DATE>`.
- [ ] `CHANGELOG.md:182` — replace the `[Unreleased]: …compare/v0.2.0...HEAD` link with
      `[0.3.0]: …/compare/v0.2.0...v0.3.0` and (optionally) add a fresh
      `[Unreleased]: …/compare/v0.3.0...HEAD`.
- [ ] Verify NO deferred item leaks in: search the `[0.3.0]` block for `SubAck`,
      `SubscriptionLost`, `resubscribe`, `rumqttc-next`/`rumqttc-v4-next`, `backend swap`
      → expect zero.
- [ ] **Per-crate CHANGELOGs also ship immutably — do NOT leave them empty (critic finding #1).**
      Both `core/CHANGELOG.md` and `macros/CHANGELOG.md` exist, are packaged, and today
      have an EMPTY `## [Unreleased]` block. `core 0.3.0` carries the major BREAKING
      de-leak (own error types, `ConnectionOptions` facade, protocol-neutral QoS) plus
      MessageMeta/ReceiveEvent/backpressure — its published changelog must say so.
  - [ ] `core/CHANGELOG.md` — convert `## [Unreleased]` → `## [0.3.0] - <DATE>` with the
        core-level changes (de-leak BREAKING, `MessageMeta`, `ReceiveEvent`, backpressure
        knobs + `dropped_messages()`, connection-state). Do not duplicate the whole root
        migration table; summarise + point to the root CHANGELOG.
  - [ ] `macros/CHANGELOG.md` — convert `## [Unreleased]` → `## [0.3.0] - <DATE>` with the
        macro-surface changes (MessageMeta `meta`/`topic` codegen, reserved-field handling).
- [ ] **Engine CHANGELOG** `mqtt-topic-engine/CHANGELOG.md` — it ALREADY has an empty
      `## [Unreleased]` (`:8`); CONVERT it to `## [0.1.2] - <DATE>` (do not leave a stray
      empty `[Unreleased]` above it). Content: "Added: `#[derive(Clone)]` on `TopicMatch`
      (cheap: one Arc bump + two inline-vec copies)."

## 4. Documentation reconciliation (the "we deferred things" cleanup)

*The plan docs still describe §3/§2c/§6 as 0.3 order-of-work. Relabel to 0.4.*

- [ ] `docs/PLAN_0.3.md` — Order of work:
  - [ ] Step 6 (`:518`) "SubAck minimal (§3) … **← NEXT**" — mark **deferred to 0.4**; remove the `← NEXT` marker.
  - [ ] Step 7 (`:520-522`) "Resubscribe-failure surfacing (§2c)" — mark **deferred to 0.4**.
  - [ ] Step 8 (`:523-524`) "Backend swap (§6)" — mark **deferred to 0.4** (already "may slip to 0.4"; make it definite).
- [ ] `docs/PLAN_0.3.md` — narrative sections: §3 (`:282-301`), §2c (`:409-449`), §6 (`:451-479`)
      — add a "DEFERRED TO 0.4" banner at each; soften the theme line (`:3-5`,
      "…ack surfacing…") and the "Lean (a) for 0.3" note (`:447`).
- [ ] `docs/ROADMAP.md`:
  - [ ] "Add retain, qos, dup flags to incoming message metadata" (`:9-10`) — mark
        **DONE (shipped 0.3 as `MessageMeta`)**.
  - [ ] "Add subscription acknowledgment confirmation" (`:12-17`) — tag **(0.4)**.
  - [ ] "Add publish acknowledgment confirmation" (`:19-23`) — tag **(0.4)**.
  - [ ] The `MessageConversionError` concrete-topic item (`:49`) already reads
        "(post-0.3)" — leave.
- [ ] Whole-tree grep confirmation that no user-facing surface promises §3/§2c/backend
      in 0.3: `README.md` (clean — only "automatic resubscribe on reconnect (happy path)",
      already softened) and `examples/README.md` (clean). `docs/research/*` and
      `docs/FUTURE_WORK_RESEARCH.md` are timestamped historical research and are
      EXCLUDED from the package (`docs/research/` in `exclude`) — leave as-is.
- [ ] `docs/PLAN_0.3_DELEAK.md:218` references "PLAN_0.3 §6 swap" — a cross-reference,
      not a promise; leave (or note it now points at a 0.4 item).

## 5. Build / test gate

*Respect the paho trap: never `--workspace --all-features` or engine `--all-features`.
Root `--all-features` is safe. Use `--locked` throughout (lockfile refreshed in step 2).*

- [ ] `cargo +nightly fmt --check` (matches the pre-commit hook; must be clean before any commit).
- [ ] Clippy (per-crate to dodge paho):
  - [ ] `cargo clippy -p mqtt-typed-client --all-features --all-targets --locked`
  - [ ] `cargo clippy -p mqtt-typed-client-core --all-features --all-targets --locked`
  - [ ] `cargo clippy -p mqtt-typed-client-macros --all-targets --locked`
  - [ ] `cargo clippy -p mqtt-topic-engine --features rumqttc,ntex-mqtt --all-targets --locked`
- [ ] Tests (`--lib` and `--doc` SEPARATELY):
  - [ ] `cargo test -p mqtt-typed-client --all-features --lib --locked`
  - [ ] `cargo test -p mqtt-typed-client --all-features --doc --locked`
  - [ ] `cargo test -p mqtt-typed-client-core --all-features --lib --locked`
  - [ ] `cargo test -p mqtt-typed-client-core --all-features --doc --locked`
  - [ ] `cargo test -p mqtt-topic-engine --features rumqttc,ntex-mqtt --lib --locked`
  - [ ] Integration tests (feature-gated): `cargo test -p mqtt-typed-client --all-features --test '*' --locked`
        (covers `serializers_integration` + `serializer_macro_integration`, which
        require the serializer/macros features).
- [ ] Build examples (root, safe): `cargo build -p mqtt-typed-client --all-features --examples --locked`
      (must include `010_connection_state`, `004_*_tls`, `100_all_serializers_demo`).

## 6. Package verification

*Assert every `include_str!` target ships, no secret/junk leaks, and `010` is packaged.*

- [ ] `cargo package -p mqtt-topic-engine --features rumqttc,ntex-mqtt --list --locked`
      → includes `README.md`, `src/**`; no paho build artifacts.
- [ ] `cargo package -p mqtt-typed-client-core --list --locked` → `README.md`, `src/**`.
- [ ] `cargo package -p mqtt-typed-client-macros --list --locked` → `README.md`, `src/**`.
- [ ] `cargo package -p mqtt-typed-client --list --locked` (root) — assert PRESENT:
      `README.md`, `docs/COMPARISON_WITH_RUMQTTC.md`, `examples/README.md`,
      `examples/000_hello_world.rs` … `examples/010_connection_state.rs`,
      `examples/100_all_serializers_demo.rs`, **`examples/102_multi_serializer_macro.rs`,
      `examples/shared/`, `examples/modular_example/`** (all auto-built + shipped — do NOT
      flag as junk, critic finding #4); and assert ABSENT:
      `dev/` (certs incl. private key), `docs/ROADMAP.md`, `docs/research/`,
      `scripts/`, `examples/internal/`, `.github/`, `deny.toml`, CLAUDE.md,
      `check_result.txt`.
- [ ] Cross-check: `grep -rn 'include_str!' src/ mqtt-topic-engine/src/` — every target
      must appear in the corresponding `--list` output.

## 7. Publish dry-run (verify, in order)

*Publish order: engine → core → macros → root. Non-leaf dry-runs may FAIL because a
dependency version is not yet on crates.io — that is expected, not a defect. The true
gate is the packaging + build steps above; real ordering happens in the MANUAL section.*

- [ ] `cargo publish -p mqtt-topic-engine --features rumqttc,ntex-mqtt --dry-run --locked`
      — **MUST PASS** (leaf; no unpublished deps). NB: `--all-features` is forbidden here.
- [ ] `cargo publish -p mqtt-typed-client-core --dry-run --locked`
      — **expected to fail** until engine `0.1.2` is live on crates.io (verify build
      pulls the registry dep). Re-run and confirm PASS after the engine publish.
- [ ] `cargo publish -p mqtt-typed-client-macros --dry-run --locked`
      — **expected to fail** until core `0.3.0` is live. Re-run after core publish.
- [ ] `cargo publish -p mqtt-typed-client --dry-run --locked`
      — **expected to fail** until core + macros `0.3.0` are live. Re-run after both.

---

## 8. 🔴 MANUAL — DO NOT AUTOMATE (executed by the human, not by any agent)

> Everything below mutates crates.io (irreversible) and the shared git history.
> No agent runs these. Perform them by hand, verifying each step before the next.

- [ ] Ensure all of steps 1–7 are green and the version-bump + CHANGELOG + docs
      commit is made and pushed (`cargo +nightly fmt`-clean).
- [ ] Publish in order, pausing for index propagation between each:
  1. [ ] `cargo publish -p mqtt-topic-engine --features rumqttc,ntex-mqtt --locked`
  2. [ ] wait until `mqtt-topic-engine 0.1.2` is visible in the index, then
         `cargo publish -p mqtt-typed-client-core --locked`
  3. [ ] wait for `mqtt-typed-client-core 0.3.0`, then
         `cargo publish -p mqtt-typed-client-macros --locked`
  4. [ ] wait for `mqtt-typed-client-macros 0.3.0`, then
         `cargo publish -p mqtt-typed-client --locked`
- [ ] Tag the monorepo: `git tag v0.3.0` (single tag for the release). Decide whether
      to ALSO add `mqtt-topic-engine-v0.1.2` (0.2 practice added a per-engine tag — see
      OPEN QUESTIONS).
- [ ] `git push` and `git push --tags` (or `git push origin v0.3.0`).
- [ ] Post-publish: confirm docs.rs built cleanly for all four crates
      (root/core all-features; engine `rumqttc,ntex-mqtt`) and that
      `crates.io/crates/mqtt-typed-client` shows 0.3.0.
- [ ] ⚠️ crates.io publishes are IMMUTABLE — a mistake means yanking + a new patch.

---

## OPEN QUESTIONS

1. **Engine semver bump: `0.1.2` (patch) vs `0.2.0` (minor)?** The only change is an
   additive `#[derive(Clone)]` on `TopicMatch`. Non-breaking → this plan recommends
   `0.1.2`. (`cargo-semver-checks` classifies a new inherent/trait impl as a minor
   bump; for a 0.x crate a patch is the pragmatic, common choice for a purely additive
   derive. Artem's call.)
2. **Release date** for the `[0.3.0] - <DATE>` CHANGELOG header / tag.
3. **Separate engine tag?** 0.2 cut `mqtt-topic-engine-v0.1.1` in addition to `v0.2.0`.
   Add `mqtt-topic-engine-v0.1.2` this cycle, or rely on the single `v0.3.0`?
4. **Confirm the engine floor bump in `core/Cargo.toml`** (`version = "0.1.0"` →
   `"0.1.2"`). Recommended (the generated bare-`topic` clone needs it), but it makes
   0.1.2 the hard minimum for downstream resolution — confirm that is intended.
5. Leave the historical `docs/research/*` and `docs/FUTURE_WORK_RESEARCH.md` untouched
   (timestamped, package-excluded)? Assumed yes.
