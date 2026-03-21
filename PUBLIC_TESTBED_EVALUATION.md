# Public Testbed Evaluation

## Goal

Pick a public GitHub repository that is safer than a live production repo for the next real end-to-end `awo` trials.

## Selection Criteria

- disposable and low-risk
- active CI and contributor workflow
- enough surface area for parallel agent work
- realistic tests and review loops
- buildable with the toolchains already present on this machine
- not so large that orchestration noise hides product issues

## Candidates

### 1. `sharkdp/bat`

- Type: Rust CLI application
- Why it fits:
  - mature contributor workflow
  - active issues and pull requests
  - clean compiled-language target
  - straightforward `cargo`-based build and test loop
  - enough surface area for real parallel tasks without monorepo overhead
- Why it is attractive for `awo`:
  - matches the current Rust-heavy local environment
  - lets us validate repo registration, slot acquisition, branch isolation, session launch, review, and cleanup without GUI complexity
- Risk:
  - less pressure on TUI or browser-facing workflows than a UI-first project

### 2. `jesseduffield/lazygit`

- Type: Go TUI application
- Why it fits:
  - strong TUI and interaction-heavy surface
  - active project with mature release rhythm
  - good second-stage testbed once basic orchestration feels solid
- Why it is attractive for `awo`:
  - stresses a TUI app in a way that is closer to `awo`'s own UX model
  - good for testing parallel task allocation in a UI-rich but still terminal-native codebase
- Risk:
  - Go stack instead of Rust
  - more UI-state complexity than needed for the first disposable trial

### 3. `Textualize/textual`

- Type: Python TUI/UI framework
- Why it fits:
  - explicitly UI-oriented
  - cross-platform mindset
  - large docs and test surface
- Why it is attractive for `awo`:
  - useful if we want to validate context-pack routing and documentation-heavy tasks
  - good pressure test for repos that mix framework code, docs, and examples
- Risk:
  - framework/library repo rather than a single end-user app
  - may tell us more about docs-heavy contribution flows than orchestration ergonomics

### 4. `astral-sh/uv`

- Type: high-velocity Rust application
- Why it fits:
  - excellent CI and contributor workflow
  - real-world operational complexity
- Why it is attractive for `awo`:
  - strong future stress test for heavy parallel agent workflows
- Risk:
  - likely too large and active for the first disposable validation
  - more likely to bury `awo` product issues under repo-scale complexity

## Recommendation

Choose `sharkdp/bat` as the first public disposable testbed.

Why:
- it is mature enough to be realistic
- small enough to stay legible
- Rust matches the current toolchain and lowers setup friction
- it gives us clean signal on whether `awo`'s slot/session/review workflow feels good before we add UI-heavy noise

## Secondary Recommendation

Use `jesseduffield/lazygit` as the next testbed after `bat` if we want to pressure TUI and interaction-heavy flows.

## Sources

- `sharkdp/bat`: https://github.com/sharkdp/bat
- `jesseduffield/lazygit`: https://github.com/jesseduffield/lazygit
- `Textualize/textual`: https://github.com/Textualize/textual
- `astral-sh/uv`: https://github.com/astral-sh/uv
