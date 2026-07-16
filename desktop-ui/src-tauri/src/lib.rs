// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use core_engine::PrismClient;
use tracing::{info, error};

#[derive(serde::Serialize, Clone)]
pub struct DiagnosisResult {
    pub intent: String,
    pub confidence: f64,
    pub root_cause: String,
    pub severity: String,
    pub explanation: String,
    pub action_plan: Vec<core_engine::client::prism::Task>,
}

#[tauri::command]
fn get_system_telemetry() -> Result<diagnostics::SystemMetrics, String> {
    info!("Tauri Command: get_system_telemetry invoked");
    Ok(diagnostics::collect_metrics())
}

#[tauri::command]
async fn diagnose_query(query: String) -> Result<DiagnosisResult, String> {
    info!(query = %query, "Tauri Command: diagnose_query invoked");

    // 1. Establish connection with AI Service
    let mut client = PrismClient::connect("http://127.0.0.1:50051")
        .await
        .map_err(|e| {
            error!(err = ?e, "Failed to connect to local AI gRPC service");
            format!("Failed to connect to local AI service: Ensure python ai-engine is listening. Detail: {}", e)
        })?;

    // 2. Parse User Query Intent
    let parsed = client.parse_intent(&query, "windows")
        .await
        .map_err(|e| {
            error!(err = ?e, "ParseIntent RPC failed");
            format!("Intent parsing failed: {}", e.message())
        })?;

    // 3. Collect Local OS Diagnostics
    let metrics_json = diagnostics::get_metrics_json(false)
        .map_err(|e| format!("Failed to read system inspector metrics: {}", e))?;

    // 4. Generate Diagnosis & Corrective Actions
    let diag = client.analyze_root_cause(&parsed.intent, &metrics_json, "windows")
        .await
        .map_err(|e| {
            error!(err = ?e, "AnalyzeRootCause RPC failed");
            format!("Root cause reasoning failed: {}", e.message())
        })?;

    Ok(DiagnosisResult {
        intent: parsed.intent,
        confidence: parsed.confidence,
        root_cause: diag.root_cause,
        severity: diag.severity,
        explanation: diag.explanation,
        action_plan: diag.action_plan,
    })
}

#[tauri::command]
async fn run_repair(
    tasks: Vec<core_engine::client::prism::Task>,
    dry_run: bool,
    raw_input: String,
    intent: String
) -> Result<String, String> {
    info!(task_count = %tasks.len(), dry_run = %dry_run, "Tauri Command: run_repair invoked");

    // Initialize local SQLite
    let db = database::DbEngine::new("prism_history.db")
        .map_err(|e| format!("SQLite connection failed: {}", e))?;

    if dry_run {
        // Run simulated dry-run verification
        scheduler::execute_action_plan(&tasks, true)
            .await
            .map_err(|e| format!("Dry-run simulation failed: {}", e))?;
        Ok("Dry-run safety simulation complete. All checks passed.".to_string())
    } else {
        // Run full database-tracked execution and AI learning update
        let rep_id = db.log_repair(&raw_input, &intent, false, None, Some("Tauri Desktop UI execution run"))
            .map_err(|e| format!("Failed to initialize database log entry: {}", e))?;

        let mut first_error = None;
        let sorted = scheduler::topological_sort(&tasks)
            .map_err(|e| format!("Topological sorting failed: {}", e))?;

        for task in &sorted {
            if first_error.is_none() {
                match scheduler::execute_task(task, false).await {
                    Ok(_) => {
                        let _ = db.log_task(&rep_id, &task.id, &task.action, &task.parameters, true);
                    }
                    Err(e) => {
                        first_error = Some(e.to_string());
                        let _ = db.log_task(&rep_id, &task.id, &task.action, &task.parameters, false);
                    }
                }
            } else {
                let _ = db.log_task(&rep_id, &task.id, &task.action, &task.parameters, false);
            }
        }

        // Establish connection to Python gRPC server for LearnFromRepair updates
        let mut client_opt = None;
        if let Ok(c) = PrismClient::connect("http://127.0.0.1:50051").await {
            client_opt = Some(c);
        }

        match first_error {
            Some(err) => {
                let _ = db.log_repair(&raw_input, &intent, false, Some(&err), Some("Tauri Desktop UI execution failed"));
                if let Some(mut client) = client_opt {
                    let _ = client.learn_from_repair(
                        &raw_input,
                        &intent,
                        tasks,
                        false,
                        Some(&err),
                        Some("Tauri Desktop UI execution failed")
                    ).await;
                }
                Err(format!("Repair execution failed: {}", err))
            }
            None => {
                let _ = db.log_repair(&raw_input, &intent, true, None, Some("Tauri Desktop UI execution succeeded"));
                if let Some(mut client) = client_opt {
                    let _ = client.learn_from_repair(
                        &raw_input,
                        &intent,
                        tasks,
                        true,
                        None,
                        Some("Tauri Desktop UI execution succeeded")
                    ).await;
                }
                Ok("All repair tasks executed successfully!".to_string())
            }
        }
    }
}

#[tauri::command]
fn get_recent_failures(intent: String) -> Result<Vec<String>, String> {
    info!(intent = %intent, "Tauri Command: get_recent_failures invoked");
    let db = database::DbEngine::new("prism_history.db")
        .map_err(|e| format!("SQLite connection failed: {}", e))?;
    db.get_recent_failures(&intent, 5)
        .map_err(|e| format!("Failed to read database logs: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_system_telemetry,
            diagnose_query,
            run_repair,
            get_recent_failures
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
