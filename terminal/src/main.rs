use core_engine::PrismClient;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use std::collections::HashMap;

/// Helper function to execute task plans, logging individual task states to the local database
async fn execute_and_log(
    db: &database::DbEngine,
    repair_id: &str,
    tasks: &[core_engine::client::prism::Task],
) -> Result<(), String> {
    let sorted = match scheduler::topological_sort(tasks) {
        Ok(s) => s,
        Err(e) => return Err(e.to_string()),
    };

    let mut first_error = None;

    for task in &sorted {
        if first_error.is_none() {
            // Execute the task since no prior errors occurred in the chain
            match scheduler::execute_task(task, false).await {
                Ok(_) => {
                    let _ = db.log_task(repair_id, &task.id, &task.action, &task.parameters, true);
                }
                Err(e) => {
                    first_error = Some(e.to_string());
                    let _ = db.log_task(repair_id, &task.id, &task.action, &task.parameters, false);
                }
            }
        } else {
            // Skip execution of dependent tasks if a predecessor failed, logging them as unsuccessful
            let _ = db.log_task(repair_id, &task.id, &task.action, &task.parameters, false);
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up tracing logging for production-grade visibility
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting PRISM Terminal Test Client...");

    // Connect to local SQLite history database
    let db_path = "prism_history.db";
    let db = database::DbEngine::new(db_path)?;
    info!("SQLite database connection established successfully.");

    // Connect to Python gRPC server running locally on port 50051
    let mut client = match PrismClient::connect("http://127.0.0.1:50051").await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to AI service at http://127.0.0.1:50051: {:?}", e);
            tracing::warn!("Ensure that the Python gRPC server (ai-engine) is started and listening.");
            return Err(e.into());
        }
    };

    // Test 1: System Health Check
    info!("Executing Health Check RPC...");
    match client.health_check("prism-cli-tester").await {
        Ok(res) => {
            println!("\n=================================");
            println!("      gRPC HEALTH CHECK          ");
            println!("=================================");
            println!("Healthy:        {}", res.healthy);
            println!("Engine Version: {}", res.version);
            println!("Status Message: {}", res.status_message);
            println!("=================================\n");
        }
        Err(status) => {
            tracing::error!("Health Check RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
        }
    }

    // -------------------------------------------------------------
    // INTEGRATION SCENARIO 1: RAM Optimization
    // -------------------------------------------------------------
    let ram_query = "My laptop is running extremely slow and VS Code uses too much RAM.";
    println!("\n>>> RUNNING SCENARIO 1: RAM OPTIMIZATION");
    info!(query = %ram_query, "Executing Intent Parsing RPC...");
    let intent_res = match client.parse_intent(ram_query, "windows").await {
        Ok(res) => {
            println!("\n=================================");
            println!("      gRPC INTENT PARSING        ");
            println!("=================================");
            println!("Raw Query:  \"{}\"", ram_query);
            println!("Intent:     {}", res.intent);
            println!("Confidence: {:.2}", res.confidence);
            println!("Reasoning:  {}", res.reasoning);
            println!("=================================\n");
            Some(res)
        }
        Err(status) => {
            tracing::error!("Intent Parsing RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
            None
        }
    };

    if let Some(parsed) = intent_res {
        // Collect telemetry
        info!("Collecting local system telemetry via System Inspector...");
        let _metrics = diagnostics::collect_metrics();
        let metrics_json = diagnostics::get_metrics_json(false)?;

        info!("Executing AnalyzeRootCause RPC...");
        match client.analyze_root_cause(&parsed.intent, &metrics_json, "windows").await {
            Ok(diag) => {
                println!("\n=================================");
                println!("    gRPC REASONING ENGINE        ");
                println!("=================================");
                println!("Root Cause:  {}", diag.root_cause);
                println!("Explanation: {}", diag.explanation);
                println!("Action Plan ({} tasks):", diag.action_plan.len());
                for (i, task) in diag.action_plan.iter().enumerate() {
                    println!("  [{}] Action: '{}' (ID: '{}')", i + 1, task.action, task.id);
                }
                println!("=================================\n");

                // Step A: Dry Run Simulation
                println!(">>> COMMENCING DRY-RUN SIMULATION (SAFETY AUDIT)...");
                match scheduler::execute_action_plan(&diag.action_plan, true).await {
                    Ok(_) => {
                        println!("\n[PRISM CLI] DRY-RUN SUCCESS: Safe path verification complete.\n");
                        
                        // Step B: Actual execution (once dry-run succeeds)
                        println!(">>> COMMENCING ACTUAL REPAIR EXECUTION & DATABASE LOGGING...");
                        
                        // Create a temporary database log entry
                        let rep_id = db.log_repair(
                            ram_query,
                            &parsed.intent,
                            false, // temporary state
                            None,
                            Some("RAM optimization run")
                        )?;

                        match execute_and_log(&db, &rep_id, &diag.action_plan).await {
                            Ok(_) => {
                                println!("\n[PRISM CLI] SUCCESS: All repair actions completed successfully!");
                                // Update database to successful
                                let _ = db.log_repair(ram_query, &parsed.intent, true, None, Some("Reclaimed RAM successfully"));
                                
                                // Step C: Learning Loop RPC
                                info!("Invoking LearnFromRepair RPC to update AI model...");
                                let _ = client.learn_from_repair(
                                    ram_query,
                                    &parsed.intent,
                                    diag.action_plan.clone(),
                                    true,
                                    None,
                                    Some("Reclaimed RAM successfully")
                                ).await;
                            }
                            Err(e) => {
                                println!("\n[PRISM CLI] ERROR: Repair execution failed: {}", e);
                                // Update database with error message
                                let _ = db.log_repair(ram_query, &parsed.intent, false, Some(&e), Some("Reclaimed RAM failed"));
                                
                                info!("Invoking LearnFromRepair RPC to log failure in AI model...");
                                let _ = client.learn_from_repair(
                                    ram_query,
                                    &parsed.intent,
                                    diag.action_plan.clone(),
                                    false,
                                    Some(&e),
                                    Some("Reclaimed RAM failed")
                                ).await;
                            }
                        }
                    }
                    Err(e) => {
                        println!("\n[PRISM CLI] DRY-RUN FAILURE: Action plan is unsafe: {}\n", e);
                    }
                }
            }
            Err(status) => {
                tracing::error!("AnalyzeRootCause RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
            }
        }
    }

    // -------------------------------------------------------------
    // INTEGRATION SCENARIO 2: Network Reset (Multi-stage with dependencies)
    // -------------------------------------------------------------
    let net_query = "My Wi-Fi keeps dropping and I need to reset the connection.";
    println!("\n>>> RUNNING SCENARIO 2: NETWORK RESET (DAG EXECUTION)");
    info!(query = %net_query, "Executing Intent Parsing RPC...");
    let net_intent_res = match client.parse_intent(net_query, "windows").await {
        Ok(res) => {
            println!("\n=================================");
            println!("      gRPC INTENT PARSING        ");
            println!("=================================");
            println!("Raw Query:  \"{}\"", net_query);
            println!("Intent:     {}", res.intent);
            println!("Confidence: {:.2}", res.confidence);
            println!("=================================\n");
            Some(res)
        }
        Err(status) => {
            tracing::error!("Intent Parsing RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
            None
        }
    };

    if let Some(parsed) = net_intent_res {
        info!("Executing AnalyzeRootCause RPC...");
        match client.analyze_root_cause(&parsed.intent, "{}", "windows").await {
            Ok(diag) => {
                println!("\n=================================");
                println!("    gRPC REASONING ENGINE        ");
                println!("=================================");
                println!("Root Cause:  {}", diag.root_cause);
                println!("Explanation: {}", diag.explanation);
                println!("Action Plan ({} tasks):", diag.action_plan.len());
                for (i, task) in diag.action_plan.iter().enumerate() {
                    print!("  [{}] Action: '{}' (ID: '{}')", i + 1, task.action, task.id);
                    if !task.dependencies.is_empty() {
                        print!(" - Depends on: {:?}", task.dependencies);
                    }
                    println!();
                }
                println!("=================================\n");

                // Step A: Dry Run Simulation
                println!(">>> COMMENCING DRY-RUN SIMULATION (SAFETY AUDIT)...");
                match scheduler::execute_action_plan(&diag.action_plan, true).await {
                    Ok(_) => {
                        println!("\n[PRISM CLI] DRY-RUN SUCCESS: Network reset simulation completed safely.\n");

                        // Step B: Actual execution
                        println!(">>> COMMENCING ACTUAL REPAIR EXECUTION & DATABASE LOGGING...");
                        let rep_id = db.log_repair(
                            net_query,
                            &parsed.intent,
                            false,
                            None,
                            Some("Network reset run")
                        )?;

                        match execute_and_log(&db, &rep_id, &diag.action_plan).await {
                            Ok(_) => {
                                println!("\n[PRISM CLI] SUCCESS: Network reset actions completed successfully!\n");
                                let _ = db.log_repair(net_query, &parsed.intent, true, None, Some("Network restored"));

                                info!("Invoking LearnFromRepair RPC to update AI model...");
                                let _ = client.learn_from_repair(
                                    net_query,
                                    &parsed.intent,
                                    diag.action_plan.clone(),
                                    true,
                                    None,
                                    Some("Network restored")
                                ).await;
                            }
                            Err(e) => {
                                println!("\n[PRISM CLI] ERROR: Network reset execution failed: {}\n", e);
                                // Update database with error logs
                                let _ = db.log_repair(net_query, &parsed.intent, false, Some(&e), Some("Network failed"));

                                info!("Invoking LearnFromRepair RPC to log failure in AI model...");
                                let _ = client.learn_from_repair(
                                    net_query,
                                    &parsed.intent,
                                    diag.action_plan.clone(),
                                    false,
                                    Some(&e),
                                    Some("Network failed")
                                ).await;
                            }
                        }
                    }
                    Err(e) => {
                        println!("\n[PRISM CLI] DRY-RUN FAILURE: Network reset simulation blocked: {}\n", e);
                    }
                }
            }
            Err(status) => {
                tracing::error!("AnalyzeRootCause RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
            }
        }
    }

    // -------------------------------------------------------------
    // INTEGRATION SCENARIO 3: Unsafe command safety guard interception
    // -------------------------------------------------------------
    println!("\n>>> RUNNING SCENARIO 3: SAFETY GUARD INTERCEPTION (DESTRUCTIVE COMMAND)");
    let unsafe_task = core_engine::client::prism::Task {
        id: "task_format_disk".to_string(),
        action: "run_command".to_string(),
        parameters: {
            let mut map = HashMap::new();
            map.insert("command_line".to_string(), "format D: /fs:ntfs /q /y".to_string());
            map
        },
        dependencies: vec![],
        requires_privilege: true,
    };

    let unsafe_plan = vec![unsafe_task];

    println!(">>> COMMENCING DRY-RUN SIMULATION ON UNTRUSTED PLAN...");
    match scheduler::execute_action_plan(&unsafe_plan, true).await {
        Ok(_) => {
            println!("\n[PRISM CLI] WARNING: Unsafe plan was not blocked! (Safety Failure)\n");
        }
        Err(e) => {
            println!("\n[PRISM CLI] SUCCESS: Unsafe plan was blocked by Safety Guard! Error: {}\n", e);
        }
    }

    // -------------------------------------------------------------
    // STEP 4: Query Recent Failures to Close the Loop
    // -------------------------------------------------------------
    println!("\n>>> QUERYING LOCAL SQLite DATABASE FOR HISTORICAL FAILURE PATTERNS...");
    match db.get_recent_failures("network_reset", 3) {
        Ok(failures) => {
            println!("\n=================================");
            println!("   SQLite LOCAL HISTORY QUERY    ");
            println!("=================================");
            if failures.is_empty() {
                println!("No recent failures recorded.");
            } else {
                for (i, err) in failures.iter().enumerate() {
                    println!("  {}. Failure Detail: '{}'", i + 1, err);
                }
            }
            println!("=================================\n");
        }
        Err(e) => {
            tracing::error!("Failed to query recent failures from SQLite: {:?}", e);
        }
    }

    Ok(())
}
