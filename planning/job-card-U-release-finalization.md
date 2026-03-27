# Job Card U — Release Finalization

## Objective

Finish Awo Console as a local-first release candidate with clear docs, trustworthy workflows, and a realistic validation story.

## Scope

### In Scope
- final docs sweep
- help text cleanup
- manual scenario refresh
- release checklist execution
- known-limitations documentation
- stable smoke matrix for local workflows

### Out Of Scope
- large new features unrelated to local release quality

## Deliverables

### 1. Documentation
- align roadmap/dev-plan docs with actual shipped behavior
- refresh operator docs
- refresh automation docs
- refresh platform notes

### 2. CLI/TUI Help Quality
- add missing help descriptions
- ensure keybindings and workflow docs match reality

### 3. Validation Matrix
- define release smoke scenarios
- run them on real repositories
- keep a smaller release checklist separate from the full scenario catalog

### 4. Open-Source Hygiene
- ensure examples remain generic and public-safe
- keep contributor-facing docs accurate

## Likely Files

- `README.md`
- `MANUAL_TEST_SCENARIOS.md`
- `docs/open-source-release-checklist.md`
- `docs/v1-roadmap.md`
- `planning/2026-03-22-development-plan.md`
- CLI/TUI help-related files

## Risks

- doc drift can make a good product feel unreliable
- release prep often uncovers lingering workflow mismatches late

## Mitigations

- treat docs as product surface, not decoration
- validate docs against real runs, not assumptions

## Verification

- documentation spot checks against real command behavior
- release smoke matrix on real repositories
- full format/lint/test pass before declaring release-ready

## Definition Of Done

- docs reflect reality
- help text is useful
- release smoke workflows are stable
- the local product can be presented honestly as finished for its intended scope
