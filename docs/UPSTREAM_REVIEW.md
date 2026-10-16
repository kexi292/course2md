# Upstream Fix Review

## Course Features Follow-up (2026-10-16)

Integration branch: `feature/windows-desktop-setup`, based on fork main `93d1782`.
Adapted `1c56b0f`, `2961d91`, `46fa89a` and the metadata concurrency part of
`b7ddbe9`: online collection multiselect, exact part identities, part titles,
duration and cached previews. Auxiliary metadata requests are bounded and
cancellable between batches; failure preserves the original candidates.
Batch submission reuses fork plan validation, frozen configuration, transaction
rollback and task deduplication. Existing notes are excluded from select-all;
manual selection visibly warns that processing creates another version.
Local folder import, translation and credential references retain their paths.
The upstream redesign, AI token counters and Ollama configuration are excluded.

Root tests passed. Desktop tests: 245 passed, 4 ignored, using an isolated
`XDG_CACHE_HOME` inside the workspace (the sandbox denies the normal user cache).
New regressions cover title fallbacks, distinct part identities, same-part
deduplication and frozen plans after restart. Native verification is recorded in
the Windows usability follow-up below when completed.

Reviewed on 2026-10-16. Integration branch: `feature/upstream-fixes`.

## Sources and Scope

- Fork baseline `main`: `8c958af14d3730983f85b830955f1baf20039e9f`.
- Upstream snapshot `sync/upstream-main`: `dcaa270cf3f7d6693e6bf9de5fb884f2a756f177` (rc.9).
- Upstream follow-up `upstream/refactor/review-fixes`: `a8b744d951aeba895eda8997e41aaa95e23ad290` (64 commits after the snapshot).
- Common ancestor: `edad2ee0f9008cd82110c450e6ed6eb942da62a6`.

The upstream follow-up is the review source; there is no literal `copy` branch.
Changes below adapt individual issues to this fork, rather than merging the
upstream UI or replacing fork modules. Translation, FunASR Flash, folder batch
processing, exports, durable request receipts, recovery, the reader and Windows
window closing remain supported. No push, main merge, release tag or published
asset replacement was performed.

## Absorbed Changes

| Issue in this fork | Upstream reference | Fork commit | Adaptation |
| --- | --- | --- | --- |
| Qwen prefix could split a UTF-8 character; unknown duration could drop speech; control errors became generic failures | `2359826`, `55e1e65`, `6861cd5`, `ff2d664`, `38ea131` | `ec5c028` | Validate duration, retain typed pause/cancel, defer energy analysis until needed. |
| Repeated empty endpoint results could poison checkpoints and transcript caches | `367fb64`, `9a773ea`, `db96e35` | `b691109` | Bound empty results, archive invalid completed caches, retain per-segment progress. |
| Unbounded probes, corrupt cached audio, incomplete sidecar subtitle discovery | `ed26a0e` | `23d61f9` | Bound ffprobe, rebuild unverified audio, include language-qualified SRT/VTT sidecars. |
| Model commands ignored the configured directory; displayed defaults diverged from runtime | `f9ecf9a` | `49864f8` | Share defaults and honor explicit model-directory overrides. |
| Metadata replacement and download verification could leave unsafe recovery inputs | `b7ddbe9` | `c39c824` | Atomic metadata saves, reject unverified existing downloads, cancellable bounded Bilibili backoff. Metadata enrichment concurrency is not imported. |
| Large media hashes and file verification blocked async workers | `8e9ec73` | `2973a98` | Move expensive work to blocking workers without changing fork task identity. |
| Summary-only output was missing from exports or required another request | `92692a9` | `a51d603` | Export saved summaries repeatedly; preserve legacy summary recovery and atomic legacy writes. |
| Windows default encoding and CRLF broke packaging inputs and fixtures | `0b3425e` | `2a1a2f2` | Explicit UTF-8 manifest reads and CRLF-safe fixture expectations. |
| Model aliases, cache checks and preparation could disagree | `c763561` | `e3bee4f` | Match canonical loader identity, reject non-GGUF cache files, check task control before preparation. |
| Text proofreading created one request per small screenshot section | `7459e22`, `ed4a437` | `cc4a4cb` | Batch up to 20 text items across sections; persist batch strategy so legacy request receipts remain reusable. Preserve image-aware and translation behavior. |
| Temporary desktop Bearer header retained secret bytes after use | `e840eba` | `93f2bf6` | Zeroize the temporary header; preserve existing bounded, redacted diagnostic errors. HTTP-library internal copies are outside this change. |
| Windows PATH lookup missed `.cmd`, `.bat` and custom launcher extensions | `d118e62` | `f2b6d34` | Honor PATHEXT in the existing helper without adding the upstream `which` dependency. |
| ASR and model-download retry waits delayed pause/cancel | `bb8aa54` | `86ba5e8` | Shared interruptible backoff; retain fork protocols and retry classification. |
| Online tasks without selected text skipped subtitle discovery | `2961d91` | `b924be6` | Discover subtitles at execution, retain user title, fill missing author/duration. Published results resume without network discovery. Online collection multi-selection and candidate identity changes are deferred. |
| NPU session token was derived from predictable process/time values | `c763561` | `36d0931` | Use 32 bytes from OS randomness. Add direct `getrandom` declaration using the already locked version. Existing token authorization remains. |
| Rewriting timeline JSONL could truncate the old file or hide flush failures | `ac8426e` | `ccf4aa2` | Serialize first, then reuse the existing atomic-write helper. |
| HTML export rescanned and copied the growing document for each image | `ac8426e` | `6cd5530` | Set image data URLs before rendering; retain portable output and no-overwrite guarantees. |

## Existing Protections Retained

The fork already has equivalents of the following fixes, so they were not copied
again: Windows close/quit (`7e100db`), per-session NPU authorization (`4bfe373`),
bounded ASR subprocesses (`00452d8`), sibling cancellation (`44ac0ac`), blocking
publication outside async workers (`f579c92`), screenshot cache verification
(`9947969`), and the applicable small engine fixes in `feb89cc` and `04a8e5b`.
The energy-analysis fallback warning from `85e2e80` is included in the ASR work;
the fork already supports USERPROFILE tilde expansion and shared filename safety.

## Deferred or Excluded

| Group | References | Reason |
| --- | --- | --- |
| Desktop redesign and related shared UI cleanup | rc.9 UI; `01eb02d`, `37c0d3c`, `cdc3195`, `8f7f553`, `93d4310`, `b777a3c`, `bb9c194`, `859813d`, `fc3de7f`, `f943806`, `1b57020`, `9042ea3`, `6efdcad`, `739cfde`, `5170cbc`, `7c7c32e`, `6ca463f`, `8cfb492`, `f38dd80`, `6a7077e`, `a8b744d` | User explicitly excluded upstream UI. No layout, colors, motion, scrollbar or desktop navigation changes imported. |
| Codex/Ollama authentication | `b489689`, `01439cc`, `86bf39b`, `8be1165`, `b842031` | New authentication, credential and service-editor workflows require a separate feature review. |
| Bundled yt-dlp and platform packaging policy | `e62d833`, `50b1e08`, `4b3dccf`, `d2382e3`, `27164bc`, `ef9721c`, `39add82`, `65c1733` | Installation, dependency provenance, CI or benchmark policy; not required for these runtime fixes. |
| Live usage/progress and collection previews | `fbb9bc8`, `1c56b0f`, remaining `2961d91` | Requires desktop/event-contract changes and collection batch workflow absent from this fork. |
| In-memory unresolved receipt cache | `a9a8170` | Keep disk receipts authoritative. Current recovery behavior observes durable receipt updates; changing consistency needs its own validation. |
| Unix publication permissions | `15ea5d0` | Explicit 0644 changes privacy and umask behavior; not validated in this Windows session. |
| Structural refactors | `4a2946e`, `acb12f7`, `756be14`, `37f2283`, `b443632`, `17a9178`, `5e9c623` | Existing fork interfaces already provide the required behavior. Avoid unrelated module churn. |
| Removal of legacy compatibility | `959681f`, `34a9703` | Would remove recovery paths retained by this fork. |
| Documentation, wording, assets and development rules | `7b90bda`, `d57a254`, `9597829`, `4ed48b0`, `b36e4db`, `b9afd37`, `d618d47`, `8624df1`, `323c11e`, `0feb89c` | No required runtime fix; preserve fork documentation and release identity. |

## Verification

- Final root `cargo test`: 196 passed, 0 failed, 2 ignored (166 library,
  13 CLI, 3 login CLI, 1 model preparation, 13 task execution).
- Final desktop build and `model_discovery::tests`: 8 passed.
- Earlier desktop `service_test::tests`: 9 passed.
- Python packaging/source tests: 9 passed.
- Root `cargo fmt --check` and `git diff --check main` passed. The modified
  desktop HTTP helper passes standalone rustfmt checking. Whole-desktop formatting
  check reports existing differences in unchanged files (including course library,
  import, reader, settings, tasks and workspace); those files were not reformatted.
- Regression cases include cross-section batching (24 items become 2 requests),
  legacy receipt reuse, typed cancellation during retry, repeat summary export
  without network requests, model cache identity, multi-image portable HTML,
  timeline replacement, and existing translation/recovery workflows.

The two ignored tests require live Bilibili access. No live paid ASR/LLM calls,
macOS/Linux validation, Apple native inference, Intel NPU inference, or fresh
native GUI walkthrough was performed for this backend-only integration. Existing
desktop compiler warnings remain. Timeline atomic writes inherit the helper's
documented lack of parent-directory fsync on sudden power loss; NPU tokens are
still passed to the local worker through process arguments.
