# Phase 4 Plan: Reasoning Engine & Task Planning Implementation

## 1. Objective

The goal of Phase 4 is to implement the **Reasoning Engine** (`AnalyzeRootCause` RPC method) inside the Python `ai-engine` and connect it end-to-end to the Rust Core Orchestrator. 

We will:
- Implement the `AnalyzeRootCause` method in `OllamaClient` to generate structured JSON diagnosis reports.
- Implement the `AnalyzeRootCause` method in the gRPC `AiServiceHandler`, adding a robust rule-based fallback when Ollama is offline.
- Modify the Rust `core-engine/src/client.rs` to expose the `analyze_root_cause` gRPC method.
- Update the Rust `terminal/src/main.rs` client to tie all phases together:
  1. Capture the user's natural language command.
  2. Parse the intent via gRPC (`ParseIntent`).
  3. Collect real-time telemetry using the `diagnostics` crate.
  4. Send the intent and telemetry to the Python reasoning engine via gRPC (`AnalyzeRootCause`).
  5. Print the complete diagnosis response (root cause, severity, action plan tasks, explanation).
- Perform end-to-end integration verification tests.

---

## 2. Dynamic Task Schema

The generated action plan contains a sequence of executable **Tasks**. Each task represents a safe, atomic system operation that can be executed by Rust.

Supported Task Actions:
1. `kill_process`: Terminates a resource-heavy process.
   - Parameters: `process_name` (e.g. `Code.exe` or `chrome.exe`) and/or `pid`.
2. `delete_file`: Cleans up temporary or junk paths.
   - Parameters: `file_path` or `directory_path`.
3. `run_command`: Resets OS network configurations.
   - Parameters: `command_line` (e.g. `ipconfig /flushdns`).
4. `restart_service`: Restarts specific system services (like `WlanSvc` for Wi-Fi).
   - Parameters: `service_name`.

### Dependencies:
Tasks can declare dependencies on other tasks (by `id`). For example, we cannot run a connectivity check task until the network reset command task completes successfully.

---

## 3. Graceful Fallback (Heuristics)

When Ollama is offline, the Reasoning Engine will analyze the `SystemMetrics` payload programmatically:
- **Optimize RAM**: Inspects the metrics' `top_processes` array, identifies the process consuming the most memory bytes (excluding system processes), and creates a `kill_process` task targeting that process.
- **Clean Junk**: Generates a safe file deletion task targeting system temporary folders.
- **Network Reset**: Creates a command execution task to flush DNS and reset the network stack.

This programmatic fallback ensures that PRISM remains completely functional and testable without active LLM daemons.
