use prism::ai_service_client::AiServiceClient;
use prism::{
    IntentRequest, IntentResponse,
    HealthCheckRequest, HealthCheckResponse,
    DiagnosisRequest, DiagnosisResponse,
    LearningRequest, LearningResponse
};
use tonic::transport::Channel;
use tracing::{info, error};

// Include the generated proto definitions
pub mod prism {
    tonic::include_proto!("prism");
}

/// A high-level client wrapping the gRPC communication with the Python ai-engine service.
pub struct PrismClient {
    client: AiServiceClient<Channel>,
}

impl PrismClient {
    /// Connects to the Python AI service at the specified address (e.g., http://[::1]:50051)
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: std::convert::TryInto<tonic::transport::Endpoint>,
        D::Error: Into<std::boxed::Box<dyn std::error::Error + std::marker::Send + std::marker::Sync>>,
    {
        info!("Attempting connection to AI Service...");
        let client = AiServiceClient::connect(dst).await?;
        info!("Successfully established connection to AI Service.");
        Ok(Self { client })
    }

    /// Verifies the connection and service health of the AI Service
    pub async fn health_check(&mut self, client_id: &str) -> Result<HealthCheckResponse, tonic::Status> {
        let request = tonic::Request::new(HealthCheckRequest {
            client_id: client_id.to_string(),
        });

        info!(client_id = %client_id, "Sending HealthCheckRequest to AI Service");
        match self.client.health_check(request).await {
            Ok(response) => {
                let inner = response.into_inner();
                info!(healthy = %inner.healthy, status = %inner.status_message, "Received HealthCheckResponse");
                Ok(inner)
            }
            Err(status) => {
                error!(error_code = ?status.code(), error_msg = %status.message(), "HealthCheck request failed");
                Err(status)
            }
        }
    }

    /// Parses a raw user query into structured intent and entities
    pub async fn parse_intent(&mut self, raw_input: &str, platform: &str) -> Result<IntentResponse, tonic::Status> {
        let request = tonic::Request::new(IntentRequest {
            raw_input: raw_input.to_string(),
            platform: platform.to_string(),
        });

        info!(raw_input = %raw_input, platform = %platform, "Sending ParseIntent request");
        match self.client.parse_intent(request).await {
            Ok(response) => {
                let inner = response.into_inner();
                info!(intent = %inner.intent, confidence = %inner.confidence, "Received ParseIntent response");
                Ok(inner)
            }
            Err(status) => {
                error!(error_code = ?status.code(), error_msg = %status.message(), "ParseIntent request failed");
                Err(status)
            }
        }
    }

    /// Invokes the Reasoning Engine to diagnose the root cause and generate a corrective action plan
    pub async fn analyze_root_cause(
        &mut self,
        intent: &str,
        system_metrics_json: &str,
        platform: &str
    ) -> Result<DiagnosisResponse, tonic::Status> {
        let request = tonic::Request::new(DiagnosisRequest {
            intent: intent.to_string(),
            system_metrics_json: system_metrics_json.to_string(),
            platform: platform.to_string(),
        });

        info!(intent = %intent, platform = %platform, "Sending AnalyzeRootCause request to AI Service");
        match self.client.analyze_root_cause(request).await {
            Ok(response) => {
                let inner = response.into_inner();
                info!(
                    root_cause = %inner.root_cause,
                    severity = %inner.severity,
                    task_count = %inner.action_plan.len(),
                    "Received DiagnosisResponse"
                );
                Ok(inner)
            }
            Err(status) => {
                error!(
                    error_code = ?status.code(),
                    error_msg = %status.message(),
                    "AnalyzeRootCause request failed"
                );
                Err(status)
            }
        }
    }

    /// Sends task execution results back to the AI reasoning engine so it can learn from successes/failures
    pub async fn learn_from_repair(
        &mut self,
        raw_input: &str,
        intent: &str,
        executed_tasks: Vec<prism::Task>,
        success: bool,
        error_message: Option<&str>,
        user_feedback: Option<&str>,
    ) -> Result<LearningResponse, tonic::Status> {
        let request = tonic::Request::new(LearningRequest {
            raw_input: raw_input.to_string(),
            intent: intent.to_string(),
            executed_tasks,
            success,
            error_message: error_message.unwrap_or("").to_string(),
            user_feedback: user_feedback.unwrap_or("").to_string(),
        });

        info!(intent = %intent, success = %success, "Sending LearnFromRepair request to AI Service");
        match self.client.learn_from_repair(request).await {
            Ok(response) => {
                let inner = response.into_inner();
                info!(updated = %inner.updated, memory_id = %inner.memory_id, "Received LearningResponse");
                Ok(inner)
            }
            Err(status) => {
                error!(
                    error_code = ?status.code(),
                    error_msg = %status.message(),
                    "LearnFromRepair request failed"
                );
                Err(status)
            }
        }
    }
}
