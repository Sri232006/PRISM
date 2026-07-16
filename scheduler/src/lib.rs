//! PRISM Task Scheduler and Repair Execution Engine
//! Sequences repair plans using topological sorting and executes low-level recovery actions.

use std::collections::{HashMap, VecDeque};
use std::process::Command;
use std::path::Path;
use sysinfo::{System, Pid};
use tracing::{info, warn, error, debug};
use thiserror::Error;

use core_engine::client::prism::Task;

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Task dependency cycle detected or invalid dependencies")]
    CycleDetected,
    #[error("Process termination failed: {0}")]
    ProcessError(String),
    #[error("File deletion failed: {0}")]
    FileError(String),
    #[error("Command execution failed: {0}")]
    CommandError(String),
    #[error("Service restart failed: {0}")]
    ServiceError(String),
    #[error("Invalid task action: {0}")]
    InvalidAction(String),
    #[error("Task parameter missing: {0}")]
    MissingParameter(String),
}

/// Performs a topological sort on the list of tasks using Kahn's algorithm.
/// Returns a sorted list of tasks in the order they must be executed,
/// or an error if a dependency cycle is detected.
pub fn topological_sort(tasks: &[Task]) -> Result<Vec<Task>, SchedulerError> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut task_map: HashMap<String, Task> = HashMap::new();

    // 1. Build adjacency list and in-degree counts
    for task in tasks {
        task_map.insert(task.id.clone(), task.clone());
        in_degree.entry(task.id.clone()).or_insert(0);

        for dep in &task.dependencies {
            // dep is a predecessor: dep -> task
            adj.entry(dep.clone()).or_default().push(task.id.clone());
            *in_degree.entry(task.id.clone()).or_insert(0) += 1;
        }
    }

    // 2. Queue nodes with 0 in-degrees (no dependencies)
    let mut queue = VecDeque::new();
    for (id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(id.clone());
        }
    }

    // 3. Process the queue
    let mut order = Vec::new();
    while let Some(u) = queue.pop_front() {
        if let Some(task) = task_map.get(&u) {
            order.push(task.clone());
        }

        if let Some(neighbors) = adj.get(&u) {
            for v in neighbors {
                if let Some(deg) = in_degree.get_mut(v) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(v.clone());
                    }
                }
            }
        }
    }

    // 4. If order length doesn't match total task count, a cycle exists
    if order.len() != tasks.len() {
        error!("Dependency cycle detected in execution plan.");
        return Err(SchedulerError::CycleDetected);
    }

    debug!("Topological sort succeeded.");
    Ok(order)
}

/// Executes a single task using the appropriate platform command or system API.
pub async fn execute_task(task: &Task) -> Result<(), SchedulerError> {
    info!(task_id = %task.id, action = %task.action, "Executing task...");

    match task.action.as_str() {
        "kill_process" => {
            let pid_str = task.parameters.get("pid").cloned().unwrap_or_default();
            let proc_name = task.parameters.get("process_name").cloned().unwrap_or_default();

            if pid_str.is_empty() && proc_name.is_empty() {
                return Err(SchedulerError::MissingParameter("pid or process_name required".to_string()));
            }

            let mut sys = System::new();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);

            let mut killed = false;

            // 1. Try to terminate via PID first
            if !pid_str.is_empty() && pid_str != "0" {
                if let Ok(pid_val) = pid_str.parse::<u32>() {
                    let sys_pid = Pid::from(pid_val as usize);
                    if let Some(process) = sys.process(sys_pid) {
                        info!(pid = %pid_val, name = %process.name().to_string_lossy(), "Killing process via PID");
                        process.kill();
                        killed = true;
                    }
                }
            }

            // 2. Try by process name if PID was not found or wasn't provided
            if !killed && !proc_name.is_empty() {
                let lower_proc_name = proc_name.to_lowercase();
                for (pid, process) in sys.processes() {
                    let name = process.name().to_string_lossy().to_lowercase();
                    if name == lower_proc_name || name.starts_with(&lower_proc_name) {
                        info!(pid = ?pid, name = %process.name().to_string_lossy(), "Killing process via name match");
                        process.kill();
                        killed = true;
                    }
                }
            }

            if killed {
                info!("Process terminated successfully.");
                Ok(())
            } else {
                warn!("Target process was not found running. Assuming already terminated (idempotence).");
                Ok(())
            }
        }

        "delete_file" => {
            let dir_path_str = task.parameters.get("directory_path").cloned();
            let file_path_str = task.parameters.get("file_path").cloned();

            let target = match (&dir_path_str, &file_path_str) {
                (Some(dir), _) => dir,
                (_, Some(file)) => file,
                _ => return Err(SchedulerError::MissingParameter("directory_path or file_path required".to_string())),
            };

            let path = Path::new(target);
            let path_str = target.to_lowercase();

            // Security Check: Enforce a strict "never delete" list for OS critical paths
            let never_delete = [
                "c:\\", "c:\\windows", "c:\\windows\\system32", "c:\\program files",
                "c:\\users", "/", "/etc", "/bin", "/usr", "/var"
            ];
            if never_delete.iter().any(|&p| path_str == p || path_str.starts_with(&(p.to_string() + "\\"))) {
                return Err(SchedulerError::FileError(format!(
                    "Security abort: Refusing to delete system-critical path: {}", target
                )));
            }

            if path.exists() {
                if path.is_dir() {
                    info!(dir = %target, "Cleaning directory contents...");
                    // Delete contents, keeping the base directory intact
                    let entries = std::fs::read_dir(path).map_err(|e| SchedulerError::FileError(e.to_string()))?;
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_file() {
                            let _ = std::fs::remove_file(&entry_path);
                        } else if entry_path.is_dir() {
                            let _ = std::fs::remove_dir_all(&entry_path);
                        }
                    }
                } else {
                    info!(file = %target, "Deleting file...");
                    std::fs::remove_file(path).map_err(|e| SchedulerError::FileError(e.to_string()))?;
                }
            } else {
                warn!("Target path does not exist. Skipping file deletion.");
            }

            info!("File system cleanup operation completed.");
            Ok(())
        }

        "run_command" => {
            let cmd_str = task.parameters.get("command_line").cloned().unwrap_or_default();
            if cmd_str.is_empty() {
                return Err(SchedulerError::MissingParameter("command_line required".to_string()));
            }

            info!(command = %cmd_str, "Spawning system shell command...");

            // Execute using target shell (powershell for Windows, sh for Unix)
            let output = if cfg!(target_os = "windows") {
                Command::new("powershell")
                    .args(&["-Command", &cmd_str])
                    .output()
            } else {
                Command::new("sh")
                    .args(&["-c", &cmd_str])
                    .output()
            }.map_err(|e| SchedulerError::CommandError(e.to_string()))?;

            if output.status.success() {
                info!("Shell command completed successfully.");
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!(stderr = %stderr, "Shell command returned non-zero code");
                Err(SchedulerError::CommandError(stderr.into_owned()))
            }
        }

        "restart_service" => {
            let service_name = task.parameters.get("service_name").cloned().unwrap_or_default();
            if service_name.is_empty() {
                return Err(SchedulerError::MissingParameter("service_name required".to_string()));
            }

            // Platform Guard
            if !cfg!(target_os = "windows") {
                warn!("restart_service is Windows-only. Skipping execution on current host OS.");
                return Ok(());
            }

            info!(service = %service_name, "Restarting Windows Service...");

            // Attempt sc.exe stop
            let _ = Command::new("sc.exe")
                .args(&["stop", &service_name])
                .output();

            // Wait brief transition period
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            // Attempt sc.exe start
            let start_out = Command::new("sc.exe")
                .args(&["start", &service_name])
                .output()
                .map_err(|e| SchedulerError::ServiceError(e.to_string()))?;

            if start_out.status.success() {
                info!("Service start command dispatched successfully.");
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&start_out.stderr);
                error!(stderr = %stderr, "Failed to start service");
                Err(SchedulerError::ServiceError(stderr.into_owned()))
            }
        }

        _ => Err(SchedulerError::InvalidAction(task.action.clone())),
    }
}

/// Helper that topological-sorts the action plan and executes all tasks in sequence.
pub async fn execute_action_plan(tasks: &[Task]) -> Result<(), SchedulerError> {
    let sorted_tasks = topological_sort(tasks)?;
    info!(task_count = %sorted_tasks.len(), "Action plan sequenced. Commencing execution.");

    for task in &sorted_tasks {
        execute_task(task).await?;
    }

    info!("All sequenced repair tasks completed successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_topological_sort() {
        let tasks = vec![
            Task {
                id: "task_2".to_string(),
                action: "run_command".to_string(),
                parameters: HashMap::new(),
                dependencies: vec!["task_1".to_string()],
                requires_privilege: false,
            },
            Task {
                id: "task_1".to_string(),
                action: "run_command".to_string(),
                parameters: HashMap::new(),
                dependencies: vec![],
                requires_privilege: false,
            },
        ];

        let sorted = topological_sort(&tasks).unwrap();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].id, "task_1");
        assert_eq!(sorted[1].id, "task_2");
    }

    #[test]
    fn test_topological_sort_cycle() {
        let tasks = vec![
            Task {
                id: "task_1".to_string(),
                action: "run_command".to_string(),
                parameters: HashMap::new(),
                dependencies: vec!["task_2".to_string()],
                requires_privilege: false,
            },
            Task {
                id: "task_2".to_string(),
                action: "run_command".to_string(),
                parameters: HashMap::new(),
                dependencies: vec!["task_1".to_string()],
                requires_privilege: false,
            },
        ];

        let sorted = topological_sort(&tasks);
        assert!(sorted.is_err());
    }
}
