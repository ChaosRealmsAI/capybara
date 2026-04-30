# Reference Repositories

This directory records external source repositories used only as local design
and architecture references for Capybara.

## Layout

```text
reference-repos/
  README.md
  MANIFEST.md
  github/
```

`README.md` and `MANIFEST.md` are tracked. `github/` is ignored and contains
local shallow clones.

## Rules

- Treat every repository under `github/` as external source material, not
  Capybara code.
- Do not vendor, copy, or port code without a separate license and architecture
  review.
- Keep clones shallow unless a deeper history is needed for a specific
  investigation.
- When adding or refreshing a repository, update `MANIFEST.md` with the source
  URL, pinned commit, local path, and why Capybara should study it.
- Prefer studying architecture, state models, interaction contracts, CLI shape,
  verification flow, provider boundaries, and product tradeoffs.
- If a reference affects Capybara implementation decisions, record the decision
  in `spec/devlog/` or the relevant version spec.

## Refresh

From the repository root:

```bash
git -C reference-repos/github/<owner>__<repo> fetch --depth=1 origin
git -C reference-repos/github/<owner>__<repo> reset --hard origin/HEAD
git -C reference-repos/github/<owner>__<repo> rev-parse HEAD
```

Then update `MANIFEST.md`.
