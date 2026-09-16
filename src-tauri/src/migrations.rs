use crate::db;
use tauri_plugin_sql::{Migration, MigrationKind};

// sqlx 按 SQL 原始字节算校验和。仓库里这些文件是 LF，但 autocrlf 让本地构建嵌进去的行尾
// 随工作区状态漂（001/004–007 曾是 CRLF、002/003 是 LF），CI 检出则全是 CRLF——
// 同一条迁移两种字节就是两个校验和，升级时比对失败，后面的迁移一条都不跑
fn lf(sql: &'static str) -> &'static str {
    if !sql.contains('\r') {
        return sql;
    }
    Box::leak(sql.replace("\r\n", "\n").into_boxed_str())
}

pub fn all() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create initial tables",
            sql: lf(db::get_init_sql()),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "add conversations and chat enhancements",
            sql: lf(include_str!("../migrations/002_conversations.sql")),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "remove role check constraint for tool calls",
            sql: lf(include_str!("../migrations/003_remove_role_check.sql")),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "add uuid for multi-device sync",
            sql: lf(include_str!("../migrations/004_sync.sql")),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "add base snapshot and conflict flag for 3-way merge",
            sql: lf(include_str!("../migrations/005_sync_merge.sql")),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "add is_private flag to exclude notes from sync",
            sql: lf(include_str!("../migrations/006_private.sql")),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "add deleted_notes tombstone to prevent resurrection on sync",
            sql: lf(include_str!("../migrations/007_deleted_tombstone.sql")),
            kind: MigrationKind::Up,
        },
    ]
}
