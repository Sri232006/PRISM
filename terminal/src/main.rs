use core_engine::PrismClient;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up tracing logging for production-grade visibility
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting PRISM Terminal Test Client...");

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
            println!("Entities:");
            if res.entities.is_empty() {
                println!("  None extracted.");
            } else {
                for (k, v) in &res.entities {
                    println!("  - {}: {}", k, v);
                }
            }
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
        let metrics = diagnostics::collect_metrics();
        println!("\n=================================");
        println!("    LOCAL SYSTEM TELEMETRY       ");
        println!("=================================");
        println!("Hostname:       {}", metrics.host.hostname);
        println!("OS Name:        {} ({})", metrics.host.os_name, metrics.host.os_version);
        println!("CPU Cores:      {} logical ({} physical)", metrics.cpu.logical_cores, metrics.cpu.physical_cores);
        println!("CPU Usage:      {:.2}%", metrics.cpu.usage_percent);
        println!("RAM Usage:      {:.2}% ({:.2} GB / {:.2} GB)",
            metrics.memory.usage_percent,
            metrics.memory.used_bytes as f64 / 1_073_741_824.0,
            metrics.memory.total_bytes as f64 / 1_073_741_824.0
        );
        println!("Top 3 Resource Consuming Processes:");
        let mut procs = metrics.top_processes.clone();
        procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        for (i, p) in procs.iter().take(3).enumerate() {
            println!("  {}. {} (PID: {}) - RAM: {:.2} MB, CPU: {:.2}%",
                i + 1,
                p.name,
                p.pid,
                p.memory_bytes as f64 / 1_048_576.0,
                p.cpu_usage_percent
            );
        }
        println!("=================================\n");

        let metrics_json = diagnostics::get_metrics_json(false)?;

        info!("Executing AnalyzeRootCause RPC...");
        match client.analyze_root_cause(&parsed.intent, &metrics_json, "windows").await {
            Ok(diag) => {
                println!("\n=================================");
                println!("    gRPC REASONING ENGINE        ");
                println!("=================================");
                println!("Root Cause:  {}", diag.root_cause);
                println!("Severity:    {}", diag.severity);
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

                // Execute the repair tasks
                println!(">>> COMMENCING REPAIR EXECUTION...");
                match scheduler::execute_action_plan(&diag.action_plan).await {
                    Ok(_) => {
                        println!("\n[PRISM CLI] SUCCESS: All repair actions completed successfully!\n");
                    }
                    Err(e) => {
                        println!("\n[PRISM CLI] ERROR: Repair execution failed: {}\n", e);
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
            println!("Reasoning:  {}", res.reasoning);
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
                println!("Severity:    {}", diag.severity);
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

                // Execute the repair tasks
                println!(">>> COMMENCING REPAIR EXECUTION...");
                match scheduler::execute_action_plan(&diag.action_plan).await {
                    Ok(_) => {
                        println!("\n[PRISM CLI] SUCCESS: Network reset actions completed successfully!\n");
                    }
                    Err(e) => {
                        println!("\n[PRISM CLI] ERROR: Network reset execution failed: {}\n", e);
                    }
                }
            }
            Err(status) => {
                tracing::error!("AnalyzeRootCause RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
            }
        }
    }

    Ok(())
}
