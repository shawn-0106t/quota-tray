## What does this PR do?

<!-- What and why, in a few lines. Link related issues with "Fixes #123". -->

## How was it tested?

<!-- Tick only what you actually ran; keep aligned with ci.yml. -->

- [ ] `cargo fmt --check` + `cargo clippy --all-targets --locked -- -D warnings` + `cargo test --locked`
- [ ] `npx tsc --noEmit` + `npm ci && npm run build`
- [ ] Relevant `--test-*` self-checks against the release exe
- [ ] Not applicable (docs only)

## Docs updated?

<!-- Behaviour changes must sync the contract in the same PR. Tick N/A where it
     genuinely does not apply. -->

- [ ] `PLAN.md` updated — or N/A: no behaviour/design change
- [ ] `AGENTS.md` updated — or N/A: no new trap or convention
- [ ] `CHANGELOG.md` (English + 中文) updated — or N/A: not user-visible
- [ ] `README.md` (English + 中文) updated — or N/A: no user-facing change

## UI changes

- [ ] Regenerated `demo/ui-demo.html` and `docs/screenshot-*.png`, and `demo/check_contrast.py` still passes
- [ ] N/A: no visual change

## Checklist

- [ ] Conventional commit subject (`feat:` / `fix:` / `docs:` / `chore(deps):`)
- [ ] No secrets, credentials or machine-specific paths in the diff
