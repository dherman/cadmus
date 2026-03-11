# Enterprise Integration

## Organization Model

```
Organization
  └── Workspaces (logical groupings of documents)
       └── Documents
```

- Users belong to organizations.
- Workspaces provide organizational structure within an org.
- Document permissions can be set at the document level or inherited from workspace-level defaults.
- Org admins can set org-wide default permission levels (e.g., "all org members get Comment access to all workspace documents").

## Agent Controls

Enterprise admins can manage agent usage across their organization:

- **Allowed scopes:** Define which token scopes are permitted (e.g., allow `docs:read` but not `docs:write` for agents).
- **Agent allowlist:** Restrict to specific approved agent integrations by agent_id pattern.
- **Maximum token lifetimes:** Cap how long agent tokens can live (e.g., max 7 days for enterprise).
- **Disable BYO agents:** Prevent individual users from creating their own agent tokens. Only org-provided integrations are available.
- **Default integrations:** Org admin configures agent integrations that are available to all users by default.

These controls are enforced at token creation time and documented in the admin settings UI.

## Audit Logging

Every document mutation is logged:

- **Actor:** User ID, agent token ID (if applicable), IP address.
- **Action:** Document edit (with change summary), permission change, comment action, agent token creation/revocation.
- **Target:** Document ID, comment ID, etc.
- **Timestamp.**

Audit logs are stored in a dedicated Postgres table and are queryable by org admins. Retention policy is configurable per org.

## Encryption at Rest

Prototype: rely on AWS-managed encryption (RDS encryption for Postgres, S3 SSE for object storage). This provides encryption at rest without application-level complexity.

Future: client-side encryption is possible since CRDT updates are opaque binary blobs that can be encrypted before storing. However, this prevents the server from reading document content (needed for search, indexing, and the sidecar's serialization). Client-side encryption would require a fundamental architecture change and is deferred.

## SSO / Identity Federation

Deferred for the prototype. The auth system uses JWT-based sessions. Enterprise SSO (SAML, OIDC) can be added as an authentication provider that produces the same JWT format, without changing the authorization layer.
