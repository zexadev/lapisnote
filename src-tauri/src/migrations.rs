use std::path::Path;

use sha2::{Digest, Sha384};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection};

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

/// 把库里已应用迁移的校验和改写成当前 SQL 的值，返回改写条数。
/// 迁移的身份是版本号，发布过的 SQL 从不改；校验和对不上只可能是嵌入字节的行尾不同，
/// 而 sqlx 会把它当「迁移被改过」拒跑，损失的是后面所有迁移
pub async fn repair_checksums(db_path: &Path, migrations: &[Migration]) -> Result<usize, sqlx::Error> {
    if !db_path.exists() {
        return Ok(0);
    }
    let mut conn = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(db_path)).await?;
    let has_table: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(&mut conn)
    .await?;
    if has_table.is_none() {
        return Ok(0);
    }
    let applied: Vec<(i64, Vec<u8>)> = sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
        .fetch_all(&mut conn)
        .await?;
    let mut repaired = 0;
    for (version, stored) in applied {
        let Some(m) = migrations.iter().find(|m| m.version == version) else {
            continue;
        };
        let expected = Sha384::digest(m.sql.as_bytes());
        if stored.as_slice() == expected.as_slice() {
            continue;
        }
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
            .bind(expected.to_vec())
            .bind(version)
            .execute(&mut conn)
            .await?;
        repaired += 1;
    }
    conn.close().await?;
    Ok(repaired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::migrate::{MigrateError, Migration as SqlxMigration, MigrationType, Migrator};
    use sqlx::Row;
    use std::borrow::Cow;
    use std::path::PathBuf;

    fn migrator(list: &[Migration], crlf: bool) -> Migrator {
        let migrations = list
            .iter()
            .map(|m| {
                let sql = if crlf { m.sql.replace('\n', "\r\n") } else { m.sql.to_string() };
                SqlxMigration::new(m.version, m.description.into(), MigrationType::Simple, sql.into(), false)
            })
            .collect::<Vec<_>>();
        Migrator { migrations: Cow::Owned(migrations), ignore_missing: false, locking: true, no_tx: false }
    }

    async fn connect(path: &Path) -> SqliteConnection {
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).create_if_missing(true))
            .await
            .unwrap()
    }

    fn temp_db() -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("lapis-migrations-{}-{}.db", std::process::id(), nanos))
    }

    #[test]
    fn crlf_checksums_block_upgrade_until_repaired() {
        tauri::async_runtime::block_on(async {
            let path = temp_db();
            let current = all();

            // 旧版本用另一种行尾建的库，只跑到版本 3（还没有 uuid）
            let mut conn = connect(&path).await;
            migrator(&current[..3], true).run(&mut conn).await.unwrap();
            let err = migrator(&current, false).run(&mut conn).await.unwrap_err();
            assert!(matches!(err, MigrateError::VersionMismatch(1)), "{err}");
            conn.close().await.unwrap();

            assert_eq!(repair_checksums(&path, &current).await.unwrap(), 3);

            let mut conn = connect(&path).await;
            migrator(&current, false).run(&mut conn).await.unwrap();
            let columns: Vec<String> = sqlx::query("PRAGMA table_info(notes)")
                .fetch_all(&mut conn)
                .await
                .unwrap()
                .iter()
                .map(|r| r.get("name"))
                .collect();
            assert!(columns.iter().any(|c| c == "uuid"), "{columns:?}");
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(applied, current.len() as i64);
            conn.close().await.unwrap();

            assert_eq!(repair_checksums(&path, &current).await.unwrap(), 0);
            let _ = std::fs::remove_file(&path);
        });
    }

    #[test]
    fn missing_database_is_left_alone() {
        tauri::async_runtime::block_on(async {
            let path = temp_db();
            assert_eq!(repair_checksums(&path, &all()).await.unwrap(), 0);
            assert!(!path.exists());
        });
    }
}
