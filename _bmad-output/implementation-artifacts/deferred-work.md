# Deferred work

- source_spec: `_bmad-output/specs/spec-magpie-mvp/stories/1-stage-b-capture-text-into-sqlite.md`
  summary: Cap the size of a single captured item (e.g. skip text over a configurable limit) so a huge copy cannot bloat the database or hold the store lock for long.
  evidence: capture_once hashes and inserts any text length while holding the store mutex; belongs with retention and settings in stage f (medium).
- source_spec: `_bmad-output/specs/spec-magpie-mvp/stories/1-stage-b-capture-text-into-sqlite.md`
  summary: Add a frontend test runner and unit-test the selection rule (keep by id on history-changed, reset on window-shown, arrow clamping) as a pure function.
  evidence: package.json has no test script; acceptance criteria 2 and 3 are only verified manually.
- source_spec: `_bmad-output/specs/spec-magpie-mvp/stories/1-stage-b-capture-text-into-sqlite.md`
  summary: Add a release-profile smoke test that a watcher panic is caught and the watcher restarts.
  evidence: cargo test always builds with unwinding, so re-adding panic = "abort" to the release profile would pass every check while breaking AD-18 in shipped builds.
- source_spec: `_bmad-output/specs/spec-magpie-mvp/stories/1-stage-b-capture-text-into-sqlite.md`
  summary: Copies made within one 500 ms macOS poll are merged (only the last is kept), and copies made while the watcher is in restart backoff are not captured.
  evidence: inherent to changeCount polling and to restarting the clipboard-rs watcher from the current count; documented in DECISIONS.md; revisit if users report missed copies.
