import os
import time
from loguru import logger
import grpc

from src.generated import prism_pb2, prism_pb2_grpc
from src.services.ollama_client import OllamaClient

class AiServiceHandler(prism_pb2_grpc.AiServiceServicer):
    """
    Implements the gRPC service defined by the prism.proto schema.
    Provides Natural Language Understanding (NLU), diagnosis suggestion, and learning loops.
    """

    def __init__(self):
        # Allow configuring model name and endpoint via environment variables, with defaults
        model_name = os.environ.get("PRISM_LLM_MODEL", "qwen2.5:3b")
        ollama_endpoint = os.environ.get("PRISM_OLLAMA_ENDPOINT", "http://127.0.0.1:11434")
        self.ollama = OllamaClient(endpoint=ollama_endpoint, model=model_name)

    def HealthCheck(self, request, context):
        logger.info(f"HealthCheck invoked by client: {request.client_id}")
        ollama_online = self.ollama.is_online()
        ollama_status = "connected" if ollama_online else "offline"

        status_msg = f"AI Engine is online. Local LLM ({self.ollama.model}) is {ollama_status}."
        logger.info(f"HealthCheck result: {status_msg}")

        return prism_pb2.HealthCheckResponse(
            healthy=True,
            version="0.1.0",
            status_message=status_msg
        )

    def ParseIntent(self, request, context):
        raw_input = request.raw_input.strip()
        platform = request.platform.strip()
        logger.info(f"ParseIntent invoked with input='{raw_input}', platform='{platform}'")

        # Attempt to run NLP reasoning via local Ollama
        if self.ollama.is_online():
            try:
                result = self.ollama.parse_intent_with_llm(raw_input, platform)
                intent = result.get("intent", "unknown")
                confidence = float(result.get("confidence", 0.0))
                entities = result.get("entities", {})
                reasoning = result.get("reasoning", "Parsed by local LLM.")

                # Ensure all entity values are cast to strings for protobuf map compliance
                flat_entities = {}
                for k, v in entities.items():
                    if v is not None:
                        flat_entities[str(k)] = str(v)

                logger.info(f"Successfully resolved intent via LLM: intent={intent}, confidence={confidence}")
                return prism_pb2.IntentResponse(
                    intent=intent,
                    confidence=confidence,
                    entities=flat_entities,
                    reasoning=reasoning
                )
            except Exception as e:
                logger.warning(f"Failed parsing intent with local LLM: {e}. Falling back to heuristics.")
        else:
            logger.warning("Ollama is offline or unreachable. Falling back to keyword heuristics.")

        # Heuristic keyword fallback
        intent = "unknown"
        confidence = 0.0
        entities = {}
        reasoning = "Rule-based parser fallback (Ollama is offline)."

        lowered = raw_input.lower()
        if "slow" in lowered or "ram" in lowered or "memory" in lowered:
            intent = "optimize_ram"
            confidence = 0.80
            reasoning = "Heuristic matched RAM keywords."
            if "vs code" in lowered:
                entities["target_process"] = "Code.exe" if platform == "windows" else "code"
        elif "wifi" in lowered or "wi-fi" in lowered or "disconnect" in lowered or "network" in lowered:
            intent = "network_reset"
            confidence = 0.80
            reasoning = "Heuristic matched network keywords."
        elif "junk" in lowered or "clean" in lowered or "temp" in lowered or "storage" in lowered or "free" in lowered:
            intent = "clean_junk"
            confidence = 0.80
            reasoning = "Heuristic matched storage keywords."
            if "20gb" in lowered:
                entities["target_free_space"] = "20GB"

        logger.info(f"Resolved intent via fallback: intent={intent}")
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
