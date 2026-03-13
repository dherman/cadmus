# Milestone: Auth and Permissions

## Goal

Multi-user system with granular document permissions. Users authenticate with email/password, and document access is controlled by per-document roles (Read/Comment/Edit).

Milestone 2 gave us persistent, multi-document storage — but anyone can access any document. This milestone adds the user model, authentication, and permission enforcement across both REST and WebSocket paths. After this milestone, Cadmus is a real multi-user application where documents are private by default and shared explicitly.

## Success Criteria

- Users can register with email/password and log in to receive a JWT.
- All REST endpoints require authentication. Unauthenticated requests receive 401.
- Document creators automatically get Edit permission. Other users have no access until explicitly invited.
- The WebSocket connection flow uses short-lived tokens: `POST /api/auth/ws-token` → JWT validated on upgrade.
- A Read-role user can view a document but cannot send Yrs Update messages (edits are rejected server-side).
- A Comment-role user can view and will be able to comment (M5) but cannot edit document content.
- An Edit-role user has full editing access.
- Document owners can invite other users by email and assign roles via a sharing UI.
- Token refresh works for long editing sessions — expired WebSocket connections reconnect seamlessly.
- The frontend shows login/register screens, hides edit controls for read-only users, and provides a document sharing dialog.

## Scope Boundaries

**In scope:** Users table, email/password auth, JWT issuance and validation, auth middleware for REST, WebSocket token flow, `PermissionedProtocol` for Yrs update gating, document sharing (invite by email, assign roles), frontend auth UI (login, register), permission-aware editor UI.

**Out of scope:** OAuth/SSO providers (deferred to M7 Enterprise), agent tokens (M6), organization/workspace hierarchy (M7), password reset flow (can be added later without schema changes), email verification (prototype accepts any email).

## PR Breakdown

The milestone is divided into five PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                      | Depends On                  | Estimated Effort |
| --- | ---------------------------------------------------------- | --------------------------- | ---------------- |
| 1   | [Users & Auth Endpoints](prs/01-users-and-auth-endpoints/) | —                           | 2–3 days         |
| 2   | [JWT Middleware & Auth Extractors](prs/02-jwt-middleware/) | PR 1 (users + JWT issuance) | 1–2 days         |
| 3   | [Permission Enforcement](prs/03-permission-enforcement/)   | PR 2 (auth middleware)      | 2–3 days         |
| 4   | [Frontend Auth UI](prs/04-frontend-auth-ui/)               | PR 1 (auth endpoints)       | 1–2 days         |
| 5   | [Document Sharing UI](prs/05-document-sharing-ui/)         | PR 3 + PR 4                 | 1–2 days         |

PRs 3 and 4 can be developed in parallel since PR 3 is backend-only (permission enforcement) and PR 4 is frontend-only (login/register UI), and they share only the dependency on PR 1/2 for JWT validation.

```
PR 1 (Users & Auth Endpoints)
 └── PR 2 (JWT Middleware)
      ├── PR 3 (Permission Enforcement) ─── can run in parallel
      └── PR 4 (Frontend Auth UI) ──────── can run in parallel
           └── PR 5 (Document Sharing UI) ← also depends on PR 3
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram, technology choices
- [WebSocket Sync Protocol](../../architecture/websocket-protocol.md) — Layer 1 (auth), Layer 4 (permission enforcement), token flow, `PermissionedProtocol`
- [Enterprise](../../architecture/enterprise.md) — future org model and SSO (deferred, but informs schema design)

## Key Technical Decisions for This Milestone

**Email/password with bcrypt.** The prototype uses simple email/password registration. Passwords are hashed with bcrypt (via the `argon2` crate, which also supports bcrypt — or `password-hash` with argon2id). No OAuth yet — SSO/OIDC is deferred to M7 Enterprise, but the JWT-based session layer is designed so adding OAuth providers later only changes the authentication step, not the authorization layer.

**JWT with short-lived access tokens.** Access tokens expire after 15 minutes. A long-lived refresh token (7 days) allows silent renewal. This keeps the attack surface small for stolen tokens while avoiding constant re-authentication during editing sessions.

**WebSocket token flow.** WebSocket connections can't use standard HTTP auth headers after the initial handshake. The architecture doc specifies: client calls `POST /api/auth/ws-token` to get a short-lived (30-second) single-use JWT, then passes it as a query parameter on the WebSocket URL. The server validates it before upgrading the connection. On token expiry during a session, the server closes with code `4401` and the client transparently reconnects with a fresh token.

**PermissionedProtocol wrapping DefaultProtocol.** The Yrs sync protocol trait allows intercepting incoming messages. Our `PermissionedProtocol` wraps `DefaultProtocol` and checks: Read/Comment users cannot send Update messages (document edits). All users can send/receive Awareness updates (presence). This is the server-side enforcement — the frontend also hides edit controls for read-only users, but the server is the authority.

**Document creator gets Edit + owner semantics.** When a user creates a document, they automatically receive an `edit` role permission entry. For sharing purposes, the creator is treated as the owner (can invite others, change roles, delete the document). We track this with a `created_by` column on the documents table rather than a separate ownership concept — simpler for the prototype.

**Permissions table already exists.** The `document_permissions` table was created in M2's initial migration. This milestone adds the `users` table, wires up the FK constraint on `document_permissions.user_id`, and starts enforcing permissions in code.

**No email verification for prototype.** Registration accepts any email without verification. This is acceptable for a prototype; email verification can be added later without schema changes (add a `verified_at` column and a verification token flow).

## Repository Changes (End State of This Milestone)

```
packages/
  server/
    migrations/
      20260312000001_initial.sql        # (unchanged from M2)
      20260312000002_users.sql          # PR 1: users table, FK on permissions
    src/
      main.rs                           # (unchanged)
      lib.rs                            # PR 2: add auth routes, middleware
      config.rs                         # (unchanged — JWT_SECRET already exists)
      db.rs                             # PR 1: user queries, PR 3: permission queries
      errors.rs                         # (unchanged — Unauthorized/Forbidden exist)
      auth/
        mod.rs                          # PR 1: auth module
        handlers.rs                     # PR 1: register, login, refresh, ws-token
        jwt.rs                          # PR 1: JWT creation/validation
        password.rs                     # PR 1: password hashing/verification
        middleware.rs                   # PR 2: auth extractor middleware
      documents/
        mod.rs                          # (unchanged)
        api.rs                          # PR 3: add permission checks to all handlers
        storage.rs                      # (unchanged)
        permissions.rs                  # PR 3: permission check logic, sharing endpoints
      websocket/
        mod.rs                          # (unchanged)
        handler.rs                      # PR 3: token validation on upgrade
        protocol.rs                     # PR 3: PermissionedProtocol
web/
  src/
    main.tsx                            # PR 4: add auth routes
    App.tsx                             # PR 4: auth context provider
    api.ts                              # PR 4: add auth headers to all requests
    auth/
      AuthContext.tsx                   # PR 4: React context for auth state
      LoginPage.tsx                     # PR 4: login form
      RegisterPage.tsx                  # PR 4: registration form
      ProtectedRoute.tsx                # PR 4: route guard component
    collaboration.ts                    # PR 3: pass ws-token on connect
    user-identity.ts                    # PR 4: replace random identity with real user
    Dashboard.tsx                       # PR 5: add share button per document
    EditorPage.tsx                      # PR 3: permission-aware toolbar
    ShareDialog.tsx                     # PR 5: invite users, manage roles
```
