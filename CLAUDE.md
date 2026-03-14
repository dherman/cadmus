# Cadmus Project Instructions

## Dev Environment

### Quick Start (all services)

```bash
pnpm dev
```

This starts Docker infra, Rust server, sidecar, and web frontend concurrently.

### Individual Services

```bash
pnpm dev:infra    # Docker: Postgres (port 5433) + LocalStack/S3 (port 4566)
pnpm dev:server   # Rust server (port 8080)
pnpm dev:sidecar  # Node sidecar for markdown conversion (port 3001)
pnpm dev:web      # Vite dev server (port 5173)
```

### Environment Variables

All env vars have sensible dev defaults. See `.env.example` for the full list. Key vars:

- `DATABASE_URL` — Postgres connection string (default: `postgres://postgres:postgres@localhost:5433/cadmus`)
- `S3_ENDPOINT` — LocalStack S3 endpoint (default: `http://localhost:4566`)
- `JWT_SECRET` — JWT signing key (default: `dev-secret-change-in-production`)
- `VITE_API_URL` — Server URL for the web client (default: `http://localhost:8080`)
- `VITE_WS_URL` — WebSocket URL for the web client (default: `ws://localhost:8080/api/docs`)

### First-Time Setup

```bash
pnpm install
pnpm -F @cadmus/doc-schema build
```

## Formatting

Before pushing to a PR branch, always run `pnpm run format:check` and fix any issues with `pnpm run format` (which runs Prettier). This applies to all file types Prettier covers, including markdown docs.

## Commits

Always use the conventional-commits skill (`.skills/conventional-commits/SKILL.md`) when writing commit messages.

## Task Tracking

When completing tasks from planning documents (implementation plans, milestone checklists, etc.), always check off the corresponding checkbox (`- [ ]` → `- [x]`) immediately after the task is done. Don't wait until the end — mark each task as completed as you go.

## Green Tree Policy

Always keep the tree green. If any test is failing, the task is not done until the test is fixed. Never dismiss a test failure as irrelevant — investigate and resolve it before considering work complete.

## Docs Structure

- `docs/architecture/*.md` — architecture design documents
- `docs/roadmap.md` — main roadmap listing all milestones
- `docs/milestones/` — active milestones, each in its own subdirectory:
- `docs/history/` — completed milestones, same subdirectory structure as `docs/milestones/`; each contains:
  - `README.md` — describes the milestone and its success criteria
  - `prs/` — plans for the PRs needed to implement the milestone, sequenced in implementation order; each PR has:
    - `design.md` — design for the PR
    - `implementation-plan.md` — step-by-step implementation plan
