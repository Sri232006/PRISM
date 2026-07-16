import time
from loguru import logger
import grpc

from src.generated import prism_pb2, prism_pb2_grpc

class AiServiceHandler(prism_pb2_grpc.AiServiceServicer):
    """
    Implements the gRPC service defined by the prism.proto schema.
    Provides Natural Language Understanding (NLU), diagnosis suggestion, and learning loops.
    """

    def HealthCheck(self, request, context):
        logger.info(f"HealthCheck invoked by client: {request.client_id}")
        return prism_pb2.HealthCheckResponse(
            healthy=True,
            version="0.1.0",
            status_message="AI Engine is online and ready."
        )

    def ParseIntent(self, request, context):
        raw_input = request.raw_input.strip()
        platform = request.platform.strip()
        logger.info(f"ParseIntent invoked with input='{raw_input}', platform='{platform}'")

        # Heuristic/mock intent parser for verifying gRPC roundtrip
        intent = "unknown"
        confidence = 0.0
        entities = {}
        reasoning = "Rule-based parser matched keywords."

        lowered = raw_input.lower()
        if "slow" in lowered or "ram" in lowered or "memory" in lowered:
            intent = "optimize_ram"
            confidence = 0.90
            reasoning = "Detected 'slow' or 'ram' or 'memory' keyword in query. Suggesting RAM optimization."
            if "vs code" in lowered:
                entities["target_process"] = "Code.exe" if platform == "windows" else "code"
        elif "wifi" in lowered or "wi-fi" in lowered or "disconnect" in lowered or "network" in lowered:
            intent = "network_reset"
            confidence = 0.85
            reasoning = "Detected network issues in query. Suggesting a network stack reset."
        elif "junk" in lowered or "clean" in lowered or "temp" in lowered or "storage" in lowered or "free" in lowered:
            intent = "clean_junk"
            confidence = 0.95
            reasoning = "Detected storage cleaning keywords. Suggesting clearing of temporary directories."
            if "20gb" in lowered:
                entities["target_free_space"] = "20GB"

        # Simulating minimal processing delay to mimic local LLM inference latency
        time.sleep(0.05)

        logger.info(f"Resolved intent={intent} with confidence={confidence}")
        return prism_pb2.IntentResponse(
            intent=intent,
            confidence=confidence,
            entities=entities,
            reasoning=reasoning
        )

    def AnalyzeRootCause(self, request, context):
        logger.info(f"AnalyzeRootCause invoked for intent={request.intent}")
        return prism_pb2.DiagnosisResponse(
            root_cause="Heuristic analysis placeholder.",
            severity="medium",
            action_plan=[],
            explanation="Reasoning Engine will be integrated in a later phase."
        )

    def LearnFromRepair(self, request, context):
        logger.info(f"LearnFromRepair invoked for intent={request.intent}, success={request.success}")
        return prism_pb2.LearningResponse(
            updated=True,
            memory_id="00000000-0000-0000-0000-000000000000"
        )
