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
- Rust >= 1.75
- PostgreSQL >= 15
- Docker (for local development)

### Getting Started

```bash
# Install JS dependencies
npm install

# Build the shared schema package
npm run build -w packages/doc-schema

# Start the Rust server (from packages/server/)
cd packages/server && cargo run

# Start the sidecar (from packages/sidecar/)
cd packages/sidecar && npm run dev

# Start the web client (from packages/web/)
cd packages/web && npm run dev
```

## License

MIT
