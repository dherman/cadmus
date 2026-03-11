/**
 * Schema migrations.
 *
 * Each migration transforms a ProseMirror JSON document from one schema version
 * to the next. Migrations are run sequentially on document load if the stored
 * version is behind SCHEMA_VERSION.
 *
 * Rules:
 * - Adding a new node type: usually a no-op migration (existing docs don't contain it).
 * - Adding an attribute to an existing type: walk the tree, add default values.
 * - Removing a type: walk the tree, convert or strip instances.
 * - Never modify attribute semantics without a migration.
 */

import type { JSONContent } from '@tiptap/core';

export type Migration = {
  fromVersion: number;
  toVersion: number;
  migrate: (doc: JSONContent) => JSONContent;
};

/**
 * Registry of all migrations, in order.
 * Add new entries here as the schema evolves.
 */
export const migrations: Migration[] = [
  // Example (commented out) — will be real when we add tables in v2:
  //
  // {
  //   fromVersion: 1,
  //   toVersion: 2,
  //   migrate: (doc) => {
  //     // Tables are additive — no existing documents contain table nodes.
  //     // This is a no-op migration; it exists for structural consistency.
  //     return doc
  //   },
  // },
];

/**
 * Apply all necessary migrations to bring a document from `fromVersion`
 * to `toVersion`.
 */
export function migrateDocument(
  doc: JSONContent,
  fromVersion: number,
  toVersion: number,
): JSONContent {
  let current = doc;
  let currentVersion = fromVersion;

  for (const migration of migrations) {
    if (migration.fromVersion === currentVersion && migration.toVersion <= toVersion) {
      current = migration.migrate(current);
      currentVersion = migration.toVersion;
    }
  }

  if (currentVersion !== toVersion) {
    throw new Error(
      `Migration gap: could not migrate from v${fromVersion} to v${toVersion}. ` +
        `Reached v${currentVersion}.`,
    );
  }

  return current;
}
