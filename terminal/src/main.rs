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

    // Test 2: Intent Parsing
    let test_query = "My laptop is running extremely slow and VS Code uses too much RAM.";
    info!(query = %test_query, "Executing Intent Parsing RPC...");
    match client.parse_intent(test_query, "windows").await {
        Ok(res) => {
            println!("\n=================================");
            println!("      gRPC INTENT PARSING        ");
            println!("=================================");
            println!("Raw Query:  \"{}\"", test_query);
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
        }
        Err(status) => {
            tracing::error!("Intent Parsing RPC returned an error: Code = {:?}, Message = '{}'", status.code(), status.message());
        }
    }

    Ok(())
}
