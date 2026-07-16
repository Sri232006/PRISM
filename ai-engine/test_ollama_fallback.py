import sys
import os
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

    print("\n=============================================")
    print("  ALL LOCAL FALLBACK TESTS PASSED SUCCESSFULLY!  ")
    print("=============================================\n")

if __name__ == "__main__":
    test_fallback()
