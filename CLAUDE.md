# Cadmus Project Instructions

## Formatting

Before pushing to a PR branch, always run `pnpm run format:check` and fix any issues with `pnpm run format` (which runs Prettier). This applies to all file types Prettier covers, including markdown docs.

## Commits

Always use the conventional-commits skill (`.skills/conventional-commits/SKILL.md`) when writing commit messages.

## Task Tracking

When completing tasks from planning documents (implementation plans, milestone checklists, etc.), always check off the corresponding checkbox (`- [ ]` → `- [x]`) immediately after the task is done. Don't wait until the end — mark each task as completed as you go.

## Docs Structure

- `docs/architecture/*.md` — architecture design documents
- `docs/roadmap.md` — main roadmap listing all milestones
- `docs/milestones/` — active milestones, each in its own subdirectory:
- `docs/history/` — completed milestones, same subdirectory structure as `docs/milestones/`; each contains:
  - `README.md` — describes the milestone and its success criteria
  - `prs/` — plans for the PRs needed to implement the milestone, sequenced in implementation order; each PR has:
    - `design.md` — design for the PR
    - `implementation-plan.md` — step-by-step implementation plan
