import sys
import os
import json
from loguru import logger

# Add root folder to python path to resolve local imports
sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

from src.services.ai_service import AiServiceHandler
from src.generated import prism_pb2

def test_fallback():
    logger.info("Initializing AiServiceHandler for local testing...")
    # Instantiate the gRPC service handler
    handler = AiServiceHandler()

    # 1. Test Health Check RPC
    logger.info("Testing HealthCheck RPC...")
    hc_response = handler.HealthCheck(
        prism_pb2.HealthCheckRequest(client_id="unit-tester"),
        None
    )
    logger.info(f"HealthCheck response: healthy={hc_response.healthy}, status_message='{hc_response.status_message}'")
    assert hc_response.healthy is True
    assert "AI Engine is online" in hc_response.status_message

    # 2. Test ParseIntent Fallback - RAM query
    logger.info("Testing ParseIntent with a RAM optimize query...")
    intent_response_ram = handler.ParseIntent(
        prism_pb2.IntentRequest(
            raw_input="VS Code is slow and taking up memory",
            platform="windows"
        ),
        None
    )
    logger.info(f"Intent resolved: intent='{intent_response_ram.intent}', confidence={intent_response_ram.confidence}, entities={intent_response_ram.entities}")
    assert intent_response_ram.intent == "optimize_ram"
    assert intent_response_ram.entities.get("target_process") == "Code.exe"

    # 3. Test ParseIntent Fallback - Storage clean query
    logger.info("Testing ParseIntent with a junk clean query...")
    intent_response_clean = handler.ParseIntent(
        prism_pb2.IntentRequest(
            raw_input="Please clean temporary junk safely to free 20GB space",
            platform="windows"
        ),
        None
    )
    logger.info(f"Intent resolved: intent='{intent_response_clean.intent}', confidence={intent_response_clean.confidence}, entities={intent_response_clean.entities}")
    assert intent_response_clean.intent == "clean_junk"
    assert intent_response_clean.entities.get("target_free_space") == "20GB"

    # 4. Test AnalyzeRootCause Fallback - RAM optimize
    logger.info("Testing AnalyzeRootCause with RAM optimize intent...")
    mock_metrics = {
        "top_processes": [
            {"pid": 1234, "name": "explorer.exe", "memory_bytes": 100000000},
            {"pid": 5678, "name": "chrome.exe", "memory_bytes": 800000000}, # Heavy process
            {"pid": 9012, "name": "Code.exe", "memory_bytes": 300000000}
        ]
    }
    
    diag_response_ram = handler.AnalyzeRootCause(
        prism_pb2.DiagnosisRequest(
            intent="optimize_ram",
            system_metrics_json=json.dumps(mock_metrics),
            platform="windows"
        ),
        None
    )
    logger.info(f"Diagnosis: root_cause='{diag_response_ram.root_cause}', severity='{diag_response_ram.severity}'")
    for task in diag_response_ram.action_plan:
        logger.info(f"Task generated: id='{task.id}', action='{task.action}', parameters={task.parameters}")

    assert diag_response_ram.severity == "high"
    assert len(diag_response_ram.action_plan) == 1
    assert diag_response_ram.action_plan[0].action == "kill_process"
    assert diag_response_ram.action_plan[0].parameters.get("process_name") == "chrome.exe"

    # 5. Test AnalyzeRootCause Fallback - Network reset
    logger.info("Testing AnalyzeRootCause with Network Reset intent...")
    diag_response_net = handler.AnalyzeRootCause(
        prism_pb2.DiagnosisRequest(
            intent="network_reset",
            system_metrics_json="{}",
            platform="windows"
        ),
        None
    )
    logger.info(f"Diagnosis: root_cause='{diag_response_net.root_cause}', severity='{diag_response_net.severity}'")
    for task in diag_response_net.action_plan:
        logger.info(f"Task generated: id='{task.id}', action='{task.action}', parameters={task.parameters}, deps={task.dependencies}")

    assert diag_response_net.severity == "medium"
    assert len(diag_response_net.action_plan) == 2
    assert diag_response_net.action_plan[0].action == "run_command"
    assert diag_response_net.action_plan[1].action == "restart_service"
    assert diag_response_net.action_plan[1].dependencies[0] == "task_flush_dns"

    print("\n=============================================")
    print("  ALL LOCAL FALLBACK TESTS PASSED SUCCESSFULLY!  ")
    print("=============================================\n")

if __name__ == "__main__":
    test_fallback()
