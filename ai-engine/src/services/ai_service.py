import os
import time
import json
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
        intent = request.intent.strip()
        system_metrics_json = request.system_metrics_json.strip()
        platform = request.platform.strip()
        logger.info(f"AnalyzeRootCause invoked with intent='{intent}', platform='{platform}'")

        # Try local LLM (Ollama) first
        if self.ollama.is_online():
            try:
                result = self.ollama.analyze_root_cause_with_llm(intent, system_metrics_json, platform)
                root_cause = result.get("root_cause", "Unknown root cause resolved by LLM.")
                severity = result.get("severity", "medium")
                action_plan_raw = result.get("action_plan", [])
                explanation = result.get("explanation", "Reasoning generated via local LLM.")

                action_plan = []
                for t in action_plan_raw:
                    task_id = t.get("id", "task_unknown")
                    action = t.get("action", "")
                    parameters = t.get("parameters", {})
                    dependencies = t.get("dependencies", [])

                    flat_params = {}
                    for k, v in parameters.items():
                        if v is not None:
                            flat_params[str(k)] = str(v)

                    action_plan.append(prism_pb2.Task(
                        id=task_id,
                        action=action,
                        parameters=flat_params,
                        dependencies=[str(d) for d in dependencies]
                    ))

                logger.info(f"Successfully generated diagnosis via LLM with {len(action_plan)} tasks.")
                return prism_pb2.DiagnosisResponse(
                    root_cause=root_cause,
                    severity=severity,
                    action_plan=action_plan,
                    explanation=explanation
                )
            except Exception as e:
                logger.warning(f"Failed to analyze root cause via local LLM: {e}. Invoking fallback heuristics.")
        else:
            logger.warning("Ollama is offline. Analyzing root cause via programmatic heuristics.")

        # Fallback Heuristics
        root_cause = "Heuristic rule matching user intent."
        severity = "medium"
        action_plan = []
        explanation = "Programmatic analysis triggered due to local LLM offline status."

        # Parse metrics safely to perform heuristic checks
        metrics = {}
        try:
            if system_metrics_json:
                metrics = json.loads(system_metrics_json)
        except Exception as parse_err:
            logger.warning(f"Failed to parse system metrics JSON: {parse_err}")

        if intent == "optimize_ram":
            root_cause = "High RAM usage detected. System responsiveness is impaired."
            severity = "high"

            # Find the top resource-consuming process from metrics
            top_processes = metrics.get("top_processes", [])
            target_proc = "Code.exe" if platform == "windows" else "code"
            target_pid = "0"
            max_mem = 0

            for proc in top_processes:
                p_name = proc.get("name", "")
                p_mem = proc.get("memory_bytes", 0)
                # Ignore system processes/explorer to avoid killing the OS
                if p_name.lower() not in ["explorer.exe", "system", "idle", "taskmgr.exe"]:
                    if p_mem > max_mem:
                        max_mem = p_mem
                        target_proc = p_name
                        target_pid = str(proc.get("pid", 0))

            action_plan.append(prism_pb2.Task(
                id="task_kill_heavy_process",
                action="kill_process",
                parameters={
                    "process_name": target_proc,
                    "pid": target_pid
                },
                dependencies=[]
            ))
            explanation = f"Terminating resource-heavy process '{target_proc}' (PID: {target_pid}) to immediately reclaim physical RAM."

        elif intent == "clean_junk":
            root_cause = "Temporary directory storage buildup leading to diminished free space."
            severity = "low"

            target_path = "C:\\Windows\\Temp" if platform == "windows" else "/tmp"

            action_plan.append(prism_pb2.Task(
                id="task_delete_temp_files",
                action="delete_file",
                parameters={
                    "directory_path": target_path,
                    "safe_cleanup": "true"
                },
                dependencies=[]
            ))
            explanation = f"Clearing files located inside temporary directory '{target_path}' to reclaim disk space."

        elif intent == "network_reset":
            root_cause = "Network adapter or DNS resolver failure."
            severity = "medium"

            cmd = "ipconfig /flushdns" if platform == "windows" else "systemd-resolve --flush-caches"

            action_plan.append(prism_pb2.Task(
                id="task_flush_dns",
                action="run_command",
                parameters={
                    "command_line": cmd
                },
                dependencies=[]
            ))

            if platform == "windows":
                # Add a service restart as dependency
                action_plan.append(prism_pb2.Task(
                    id="task_restart_wlan",
                    action="restart_service",
                    parameters={
                        "service_name": "WlanSvc"
                    },
                    dependencies=["task_flush_dns"]
                ))

            explanation = "Flushing DNS resolver cache and restarting network interfaces to re-establish connections."

        else:
            root_cause = "Unrecognized intent. Manual inspection recommended."
            severity = "low"
            explanation = "No automatic remedy plan generated for unknown category."

        logger.info(f"Generated heuristic diagnosis: root_cause='{root_cause}', tasks={len(action_plan)}")
        return prism_pb2.DiagnosisResponse(
            root_cause=root_cause,
            severity=severity,
            action_plan=action_plan,
            explanation=explanation
        )

    def LearnFromRepair(self, request, context):
        logger.info(f"LearnFromRepair invoked for intent={request.intent}, success={request.success}")
        for task in request.executed_tasks:
            logger.info(f"  - Task {task.id}: action={task.action}")
        if request.error_message:
            logger.warning(f"  - Error reported: {request.error_message}")
        if request.user_feedback:
            logger.info(f"  - User Feedback: {request.user_feedback}")

        # Simulate writing to a local telemetry log file inside the workspace
        import uuid
        memory_id = str(uuid.uuid4())
        try:
            log_entry = {
                "timestamp": time.time(),
                "raw_input": request.raw_input,
                "intent": request.intent,
                "success": request.success,
                "error_message": request.error_message,
                "user_feedback": request.user_feedback,
                "memory_id": memory_id,
                "tasks": [{"id": t.id, "action": t.action} for t in request.executed_tasks]
            }
            # Append log entry to a JSON lines file
            telemetry_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "telemetry_learning.jsonl"))
            with open(telemetry_path, "a") as f:
                f.write(json.dumps(log_entry) + "\n")
            logger.info(f"Telemetry log written successfully to: {telemetry_path}")
        except Exception as e:
            logger.error(f"Failed to write local telemetry log: {e}")

        return prism_pb2.LearningResponse(
            updated=True,
            memory_id=memory_id
        )
