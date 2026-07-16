import json
import requests
from loguru import logger

class OllamaClient:
    """
    Client wrapper for interacting with a local Ollama service.
    Uses system prompts and the JSON format option to extract structured NLU intents
    and generate structured root cause diagnoses and repair action plans.
    """

    def __init__(self, endpoint: str = "http://127.0.0.1:11434", model: str = "qwen2.5:3b"):
        self.endpoint = endpoint
        self.model = model
        logger.info(f"OllamaClient initialized with endpoint={self.endpoint}, model={self.model}")

    def is_online(self) -> bool:
        """
        Pings the local Ollama service tags endpoint to see if it is running.
        Returns True if online, False otherwise.
        """
        try:
            response = requests.get(f"{self.endpoint}/api/tags", timeout=1.0)
            return response.status_code == 200
        except Exception as e:
            logger.debug(f"Ollama health check failed: {e}")
            return False

    def parse_intent_with_llm(self, query: str, platform: str) -> dict:
        """
        Executes NLU inference via local Ollama.
        Forces the output to be a valid JSON object matching the required schema.
        Raises an exception if the connection fails or parsing fails.
        """
        url = f"{self.endpoint}/api/generate"

        system_prompt = (
            "You are the natural language understanding (NLU) core of PRISM, an autonomous OS management engine.\n"
            "Your task is to parse the user's query and categorize their request into a system intent, extract entities, and provide a reasoning.\n\n"
            "Supported Intents:\n"
            "- optimize_ram: User mentions memory issues, lag, high RAM usage, or specific processes eating RAM.\n"
            "- network_reset: User mentions Wi-Fi, Internet, network dropouts, DNS, or connectivity.\n"
            "- clean_junk: User mentions temporary files, junk, disk space, storage cleanup, or freeing gigabytes.\n"
            "- unknown: Any query that does not fall into the above categories.\n\n"
            "Output MUST be a valid JSON object matching the following structure:\n"
            "{\n"
            '  "intent": "intent_string",\n'
            '  "confidence": 0.0-1.0,\n'
            '  "entities": {\n'
            '     "target_process": "process name if mentioned",\n'
            '     "target_disk": "drive letter if mentioned",\n'
            '     "target_free_space": "size mentioned, e.g. 20GB",\n'
            '     "target_service": "service name if mentioned"\n'
            "  },\n"
            '  "reasoning": "Brief explanation."\n'
            "}\n"
            "Do not return any conversational text outside the JSON block."
        )

        prompt = f"Platform: {platform}\nUser Query: {query}\n\nJSON Output:"

        payload = {
            "model": self.model,
            "prompt": prompt,
            "system": system_prompt,
            "stream": False,
            "format": "json",  # Force structured JSON generation
            "options": {
                "temperature": 0.0,  # Zero temperature for deterministic classification
            }
        }

        logger.info(f"Sending ParseIntent prompt to local LLM model={self.model}...")
        response = requests.post(url, json=payload, timeout=10.0)

        if response.status_code != 200:
            raise RuntimeError(f"Ollama returned status code {response.status_code}: {response.text}")

        result_json = response.json()
        response_text = result_json.get("response", "").strip()

        logger.debug(f"Raw response from Ollama: {response_text}")

        # Parse text response as JSON
        parsed_response = json.loads(response_text)
        return parsed_response

    def analyze_root_cause_with_llm(self, intent: str, system_metrics_json: str, platform: str) -> dict:
        """
        Sends the intent, host system metrics, and platform to local LLM.
        Instructs it to output a JSON-formatted root cause diagnosis and action plan.
        """
        url = f"{self.endpoint}/api/generate"

        system_prompt = (
            "You are the reasoning and repair planning core of PRISM, an autonomous OS management engine.\n"
            "Your task is to analyze the detected user intent, review the collected host system metrics JSON, "
            "and construct a detailed diagnosis and step-by-step action plan to safely resolve the issue.\n\n"
            "Available Actions you can generate:\n"
            "1. kill_process: Terminates a process. Parameters: 'process_name' (e.g. 'Code.exe') or 'pid'.\n"
            "2. delete_file: Deletes a directory/file safely. Parameters: 'file_path' or 'directory_path'.\n"
            "3. restart_service: Restarts a system service (Windows only). Parameters: 'service_name'.\n"
            "4. run_command: Runs a safe command. Parameters: 'command_line'.\n\n"
            "Output MUST be a valid JSON object matching the following structure:\n"
            "{\n"
            '  "root_cause": "Detailed description of what is causing the issue based on the intent and metrics.",\n'
            '  "severity": "low" | "medium" | "high",\n'
            '  "action_plan": [\n'
            "    {\n"
            '      "id": "task_id",\n'
            '      "action": "kill_process" | "delete_file" | "restart_service" | "run_command",\n'
            '      "parameters": {\n'
            '        "param_key": "param_value"\n'
            "      },\n"
            '      "dependencies": ["dependency_task_id"]\n'
            "    }\n"
            "  ],\n"
            '  "explanation": "Why this action plan was chosen and how it safely fixes the system."\n'
            "}\n"
            "Do not return any conversational text outside the JSON block."
        )

        prompt = (
            f"Platform: {platform}\n"
            f"Detected Intent: {intent}\n"
            f"System Metrics JSON: {system_metrics_json}\n\n"
            f"JSON Output:"
        )

        payload = {
            "model": self.model,
            "prompt": prompt,
            "system": system_prompt,
            "stream": False,
            "format": "json",
            "options": {
                "temperature": 0.0,
            }
        }

        logger.info("Sending AnalyzeRootCause prompt to local LLM...")
        response = requests.post(url, json=payload, timeout=15.0)

        if response.status_code != 200:
            raise RuntimeError(f"Ollama returned status code {response.status_code}: {response.text}")

        result_json = response.json()
        response_text = result_json.get("response", "").strip()

        logger.debug(f"Raw diagnosis response from Ollama: {response_text}")
        parsed_response = json.loads(response_text)
        return parsed_response
