use crate::StoreError;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

/// Applies the migrations not yet applied, tracked by `PRAGMA user_version`.
pub(crate) fn run(conn: &mut Connection) -> Result<(), StoreError> {
    let applied: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS
        .iter()
        .enumerate()
        .skip(usize::try_from(applied).unwrap_or(0))
    {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
