use anyhow::Context;
use chrono::Utc;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use crate::structures::{Schedule, ScheduleVariant};

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub telegram_id: i64,
    pub group_number: Option<i32>,
}

impl Db {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("failed to connect to sqlite")?;

        let db = Self { pool };
        db.init().await?;
        Ok(db)
    }

    async fn init(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (\
                telegram_id INTEGER PRIMARY KEY,\
                group_number INTEGER NULL,\
                remind_hour_msk INTEGER NULL,\
                created_at INTEGER NOT NULL,\
                updated_at INTEGER NOT NULL\
            )",
        )
        .execute(&self.pool)
        .await
        .context("failed to create users table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schedule_cache (\
                id TEXT PRIMARY KEY,\
                schedule_json TEXT NOT NULL,\
                updated_at INTEGER NOT NULL\
            )",
        )
        .execute(&self.pool)
        .await
        .context("failed to create schedule_cache table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS meta (\
                key TEXT PRIMARY KEY,\
                value TEXT NOT NULL\
            )",
        )
        .execute(&self.pool)
        .await
        .context("failed to create meta table")?;

        Ok(())
    }

    pub async fn ensure_user(&self, telegram_id: i64) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT OR IGNORE INTO users (telegram_id, created_at, updated_at) VALUES (?, ?, ?)",
        )
        .bind(telegram_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("failed to insert user")?;

        Ok(())
    }

    pub async fn get_group(&self, telegram_id: i64) -> anyhow::Result<Option<i32>> {
        self.ensure_user(telegram_id).await?;

        let group = sqlx::query_scalar::<_, Option<i32>>(
            "SELECT group_number FROM users WHERE telegram_id = ?",
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        Ok(group)
    }

    pub async fn set_group(&self, telegram_id: i64, group_number: i32) -> anyhow::Result<()> {
        self.ensure_user(telegram_id).await?;

        let now = Utc::now().timestamp();
        sqlx::query("UPDATE users SET group_number = ?, updated_at = ? WHERE telegram_id = ?")
            .bind(group_number)
            .bind(now)
            .bind(telegram_id)
            .execute(&self.pool)
            .await
            .context("failed to update user group")?;

        Ok(())
    }

    pub async fn replace_schedule_cache(
        &self,
        schedules: &[Schedule],
        last_updated: i64,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await.context("failed to start tx")?;

        sqlx::query("DELETE FROM schedule_cache")
            .execute(&mut *tx)
            .await
            .context("failed to clear schedule_cache")?;

        for schedule in schedules {
            let id = match &schedule.schedule_type {
                ScheduleVariant::Group(group) => format!("group:{}", group.id),
                ScheduleVariant::Teacher(teacher) => format!("teacher:{}", teacher.isu_id),
            };
            let json = serde_json::to_string(schedule).context("failed to serialize schedule")?;
            sqlx::query(
                "INSERT INTO schedule_cache (id, schedule_json, updated_at) VALUES (?, ?, ?)",
            )
            .bind(id)
            .bind(json)
            .bind(last_updated)
            .execute(&mut *tx)
            .await
            .context("failed to insert schedule cache")?;
        }

        sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)")
            .bind("schedule_last_updated")
            .bind(last_updated.to_string())
            .execute(&mut *tx)
            .await
            .context("failed to update schedule_last_updated")?;

        tx.commit().await.context("failed to commit tx")?;
        Ok(())
    }

    pub async fn load_schedule_cache(&self) -> anyhow::Result<Vec<Schedule>> {
        let rows = sqlx::query_scalar::<_, String>("SELECT schedule_json FROM schedule_cache")
            .fetch_all(&self.pool)
            .await
            .context("failed to fetch schedule cache")?;

        let mut schedules = Vec::with_capacity(rows.len());
        for json in rows {
            let schedule: Schedule =
                serde_json::from_str(&json).context("failed to deserialize schedule")?;
            schedules.push(schedule);
        }

        Ok(schedules)
    }

    pub async fn load_schedule_last_updated(&self) -> anyhow::Result<Option<i64>> {
        let value = sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key = ?")
            .bind("schedule_last_updated")
            .fetch_optional(&self.pool)
            .await
            .context("failed to fetch schedule_last_updated")?;

        if let Some(value) = value {
            return Ok(value.parse::<i64>().ok());
        }

        let fallback =
            sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(updated_at) FROM schedule_cache")
                .fetch_one(&self.pool)
                .await
                .context("failed to fetch schedule_cache updated_at")?;

        Ok(fallback)
    }

    pub async fn remove_remind_hour(&self, telegram_id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET remind_hour_msk = NULL WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&self.pool)
            .await
            .context("failed to remove remind hour")?;
        Ok(())
    }

    pub async fn set_remind_hour(&self, telegram_id: i64, hour: u32) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET remind_hour_msk = ? WHERE telegram_id = ?")
            .bind(hour)
            .bind(telegram_id)
            .execute(&self.pool)
            .await
            .context("failed to set remind hour")?;

        Ok(())
    }

    pub async fn get_users_for_hour(&self, hour: i32) -> anyhow::Result<Vec<UserRow>> {
        let rows = sqlx::query_as::<_, (i64, Option<i32>)>(
            "SELECT telegram_id, group_number FROM users WHERE remind_hour_msk = ?",
        )
        .bind(hour)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch users for hour")?;

        Ok(rows
            .into_iter()
            .map(|(telegram_id, group_number)| UserRow {
                telegram_id,
                group_number,
            })
            .collect())
    }
}
