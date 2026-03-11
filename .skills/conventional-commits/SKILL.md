---
name: conventional-commits
description: Formats git commit messages using the conventional commits format. Use when creating or modifying a git commit message.
---

When creating or modifying a git commit message, use the **Conventional Commits** format to support release automation. All commit messages should follow this format:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Commit Types

- **feat:** A new feature (triggers minor version bump)
- **fix:** A bug fix (triggers patch version bump)
- **docs:** Documentation only changes
- **style:** Code style changes (formatting, missing semicolons, etc.)
- **refactor:** Code changes that neither fix bugs nor add features
- **perf:** Performance improvements
- **test:** Adding or updating tests
- **chore:** Maintenance tasks, dependency updates, etc.
- **ci:** CI/CD configuration changes
- **build:** Build system or external dependency changes

### Breaking Changes

Add `!` after the type to indicate breaking changes (triggers major version bump):

```
feat!: change API to use async traits
```

Also include `BREAKING CHANGE:` in the footer with a summary:

```
feat: redesign conductor protocol

BREAKING CHANGE: conductor now requires explicit capability registration
```

### Examples

```
feat(conductor): add support for dynamic proxy chains
fix(acp): resolve deadlock in message routing
docs: update README with installation instructions
chore: bump @agentclientprotocol/sdk to 0.12.0
```

### Scope Guidelines

Scope names should generally match the name of the main npm package, Rust
crate, etc that was modified in the commit.
