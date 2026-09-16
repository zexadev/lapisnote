use crate::db;
use tauri_plugin_sql::{Migration, MigrationKind};

pub fn all() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create initial tables",
            sql: db::get_init_sql(),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "add conversations and chat enhancements",
            sql: include_str!("../migrations/002_conversations.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "remove role check constraint for tool calls",
            sql: include_str!("../migrations/003_remove_role_check.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "add uuid for multi-device sync",
            sql: include_str!("../migrations/004_sync.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "add base snapshot and conflict flag for 3-way merge",
            sql: include_str!("../migrations/005_sync_merge.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "add is_private flag to exclude notes from sync",
            sql: include_str!("../migrations/006_private.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "add deleted_notes tombstone to prevent resurrection on sync",
            sql: include_str!("../migrations/007_deleted_tombstone.sql"),
            kind: MigrationKind::Up,
        },
    ]
}
