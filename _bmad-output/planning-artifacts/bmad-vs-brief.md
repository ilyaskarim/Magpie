# Where BMad and the brief disagreed

The brief (`product-intent.md`) wins every disagreement. Each one is listed
here rather than resolved silently.

| # | BMad said | Brief said | Resolution |
| --- | --- | --- | --- |
| 1 | Install docs lead with v7 (`npx skills add …`, `bmad setup`), a preview alpha | `npx bmad-method install --directory . --modules bmm --tools claude-code --yes` | Used the stable v6.12.4 CLI, which accepts the brief's command as written. |
| 2 | `bmad-help` lists `bmad-prd`, `bmad-create-epics-and-stories` and `bmad-sprint-planning` as required | One proportionate planning pass; don't restart discovery | Used `bmad-spec` (which `bmad-help` names as the lean alternative to a PRD) and `bmad-architecture`. Stages a–g are the epics; `stories.yaml` maps one story per stage. |
| 3 | `bmad-spec` story breakdown asks the user for checkpoints per story | Checkpoints are fixed: after stage a + BMad, and after stage c | Took the checkpoints from the brief instead of asking again. |
| 4 | `bmad-architecture` defaults to a coaching conversation | Decide non-blocking calls yourself and note them | Used the skill's Fast path; decisions are in the spine and DECISIONS.md. |
| 5 | Installer writes the user's name into config files meant to be committed (`_bmad/core/config.yaml`, `_bmad/bmm/config.yaml`) | Leave out anything containing the user's name | Gitignored those files; BMad works without them. |
