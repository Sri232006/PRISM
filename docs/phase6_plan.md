# Phase 6 Plan: Dry Run Simulator & Safety Guards

## 1. Objective

The goal of Phase 6 is to implement a **Dry Run Simulator & Safety Guard** inside our `scheduler` crate and integrate it into the execution loop. 

We will:
- Modify `execute_task` and `execute_action_plan` to accept a `dry_run: bool` flag.
- Build simulated behaviors for each task type:
  - **`kill_process`**: Verifies process existence and logs a simulated termination report.
  - **`delete_file`**: Traverses directories to calculate simulated reclaimed storage space without deleting files.
  - **`run_command`**: Runs a safety regex check against forbidden commands (e.g. `rm -rf`, `format`, `erase`) and logs simulated shell executions.
  - **`restart_service`**: Validates service configuration existence on Windows.
- Update the `terminal` client to perform a dry-run simulation first, reporting expected system impact (freed storage, killed processes) and safety audits, before initiating execution.

---

## 2. Safety Audit Filters

To prevent autonomous model execution of destructive commands, we implement a syntax guard blocklist:

| Pattern | Risk Category | Action |
| --- | --- | --- |
| `rm -rf`, `rmdir /s` | Unrestricted path deletion | Block execution |
| `format `, `mkfs` | Disk formatting | Block execution |
| `erase `, `del /f /s` | Wildcard file deletion | Block execution |
| `shutdown`, `reboot` | System denial of service | Block execution |

If any command contains these patterns, the scheduler returns a `CommandError` and halts the plan.

---

## 3. Simulated Space Calculation

When simulating `delete_file` for directories, we traverse the target path recursively using `std::fs::read_dir`, aggregating:
1. Total files that would be deleted.
2. Total bytes that would be reclaimed.

This metadata is presented to the user as a recovery prediction.
