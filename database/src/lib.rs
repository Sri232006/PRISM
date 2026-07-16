//! PRISM SQLite History Database Crate
//! Persists repair outcomes, task executions, and telemetry diagnostics locally.

use rusqlite::{params, Connection, Result};
use std::path::Path;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, debug};

pub struct DbEngine {
    conn: Connection,
}

impl DbEngine {
    /// Initializes a local SQLite database file, creating all tables if they do not exist
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        info!("Initializing SQLite database at: {:?}", path.as_ref());
        let conn = Connection::open(path)?;

        // Enable foreign keys for relational safety
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        let mut db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    /// Initializes a temporary in-memory SQLite database (excellent for isolated testing)
    pub fn new_in_memory() -> Result<Self> {
        info!("Initializing in-memory SQLite database...");
        let conn = Connection::open_in_memory()?;
        let mut db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    fn create_tables(&mut self) -> Result<()> {
        debug!("Creating database schemas if not present...");

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS repair_history (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                raw_input TEXT NOT NULL,
                intent TEXT NOT NULL,
                success INTEGER NOT NULL,
                error_message TEXT,
                user_feedback TEXT
            );",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS task_history (
                id TEXT PRIMARY KEY,
                repair_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                action TEXT NOT NULL,
                parameters TEXT NOT NULL,
                success INTEGER NOT NULL,
                FOREIGN KEY(repair_id) REFERENCES repair_history(id)
            );",
            [],
        )?;

        debug!("Database schemas initialized successfully.");
        Ok(())
    }

    /// Logs a complete repair session outcome
    pub fn log_repair(
        &self,
        raw_input: &str,
        intent: &str,
        success: bool,
        error_message: Option<&str>,
        user_feedback: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();
        let success_val = if success { 1 } else { 0 };

        info!(repair_id = %id, intent = %intent, success = %success, "Logging repair history to database...");

        self.conn.execute(
            "INSERT INTO repair_history (id, timestamp, raw_input, intent, success, error_message, user_feedback)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![id, timestamp, raw_input, intent, success_val, error_message, user_feedback],
        )?;

        Ok(id)
    }

    /// Logs a single task execution outcome linked to a repair session
    pub fn log_task(
        &self,
        repair_id: &str,
        task_id: &str,
        action: &str,
        parameters: &std::collections::HashMap<String, String>,
        success: bool,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let success_val = if success { 1 } else { 0 };
        let params_json = serde_json::to_string(parameters).unwrap_or_else(|_| "{}".to_string());

        debug!(task_id = %task_id, repair_id = %repair_id, "Logging task run...");

        self.conn.execute(
            "INSERT INTO task_history (id, repair_id, task_id, action, parameters, success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![id, repair_id, task_id, action, params_json, success_val],
        )?;

        Ok(id)
    }

    /// Retrieves failure logs for a given intent to detect persistent system blocks
    pub fn get_recent_failures(&self, intent: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT error_message FROM repair_history 
             WHERE intent = ?1 AND success = 0 AND error_message IS NOT NULL
             ORDER BY timestamp DESC LIMIT ?2;"
        )?;

        let rows = stmt.query_map(params![intent, limit], |row| {
            row.get::<_, String>(0)
        })?;

        let mut failures = Vec::new();
        for r in rows {
            if let Ok(msg) = r {
                failures.push(msg);
            }
        }
        Ok(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_db_initialization_and_logging() {
        let db = DbEngine::new_in_memory().unwrap();

        // Log repair
        let rep_id = db.log_repair(
            "My laptop is slow",
            "optimize_ram",
            true,
            None,
            Some("Feels much faster!"),
        ).unwrap();

        assert!(!rep_id.is_empty());

        // Log task
        let mut params = HashMap::new();
        params.insert("process_name".to_string(), "Code.exe".to_string());

        let task_run_id = db.log_task(
            &rep_id,
            "task_kill_code",
            "kill_process",
            &params,
            true,
        ).unwrap();

        assert!(!task_run_id.is_empty());
    }

    #[test]
    fn test_failures_recovery() {
        let db = DbEngine::new_in_memory().unwrap();

        db.log_repair(
            "Reset network connection",
            "network_reset",
            false,
            Some("Access denied restarting service WlanSvc"),
            None,
        ).unwrap();

        let failures = db.get_recent_failures("network_reset", 5).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0], "Access denied restarting service WlanSvc");
    }
}
