# Cadmus

A collaborative document editor backed by markdown, built for real-time editing, AI agent integration, and local tooling workflows.

## Key Design Principles

- **Markdown as source of truth.** Documents are stored and exported as canonical markdown. The editor uses a rich ProseMirror/Tiptap document model internally, but markdown is the portable interchange format.
- **Real-time collaboration via CRDTs.** Built on Yjs (client) and Yrs (server) for conflict-free concurrent editing without operational transforms.
- **Agent-first API design.** First-class support for AI agents as document collaborators — authenticate with scoped tokens, read/write via REST or WebSocket, plug in any agent you control.
- **Local tool integration.** CLI-based checkout/push workflow for editing documents with local tools (IDEs, terminal agents, scripts), with structured merge back to the server.
- **Enterprise-ready architecture.** Granular permissions, organization/workspace hierarchy, admin controls for agent integrations, audit logging.

## Architecture Overview

The system consists of five main components:

| Component      | Language                    | Purpose                                                                                                     |
| -------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **Server**     | Rust (Axum)                 | WebSocket sync (Yrs), REST API, auth, permissions, persistence                                              |
| **Web Client** | TypeScript (React + Tiptap) | Browser-based collaborative editor                                                                          |
| **Doc Schema** | TypeScript                  | Shared Tiptap/ProseMirror schema — single source of truth for document structure and markdown serialization |
| **Sidecar**    | TypeScript (Node)           | Stateless markdown↔JSON conversion service, co-deployed with the server                                     |
| **CLI**        | TypeScript                  | Command-line tool for checkout/push workflows and agent scripting                                           |

For detailed design documentation, see [docs/architecture/](docs/architecture/).

For the implementation roadmap, see [docs/roadmap.md](docs/roadmap.md).

## Repository Structure

```
cadmus/
├── docs/
│   ├── architecture/       # Design decision documents
│   └── roadmap.md          # Implementation milestones
├── packages/
│   ├── doc-schema/         # Shared document schema (TypeScript)
│   ├── server/             # Rust backend (Axum + Yrs)
│   ├── sidecar/            # Node markdown conversion service
│   ├── web/                # React frontend
│   └── cli/                # Command-line tool
├── package.json            # Workspace root
└── README.md
```

## Development

### Prerequisites

- Node.js >= 20
- pnpm >= 9
- Rust >= 1.75
- Docker (for Postgres and LocalStack)

### Getting Started

```bash
# Install JS dependencies
pnpm install

# Build the shared schema package
pnpm -F @cadmus/doc-schema build

# Copy and (optionally) edit environment variables
cp .env.example .env
```

### Running Everything

```bash
# Start all services (Docker infra + Rust server + sidecar + web)
pnpm dev
```

This runs all four services concurrently with color-coded output.

### Running Services Individually

```bash
pnpm dev:infra    # Docker: Postgres (port 5433) + LocalStack/S3 (port 4566)
pnpm dev:server   # Rust server (port 8080)
pnpm dev:sidecar  # Node sidecar for markdown conversion (port 3001)
pnpm dev:web      # Vite dev server (port 5173)
```

### Ports

| Service    | Port |
| ---------- | ---- |
| Postgres   | 5433 |
| LocalStack | 4566 |
| Server     | 8080 |
| Sidecar    | 3001 |
| Web        | 5173 |

## License

MIT
