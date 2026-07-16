//! Diagnostics Inspector Crate
//! Gathers system telemetry (CPU, memory, disks, processes) in a platform-independent way.

use serde::{Serialize, Deserialize};
use sysinfo::{System, Disks, CpuRefreshKind, RefreshKind};
use std::collections::HashSet;
use std::thread;
use std::time::Duration;
use tracing::{info, debug};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CpuInfo {
    pub usage_percent: f32,
    pub logical_cores: usize,
    pub physical_cores: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_bytes: u64,
    pub cpu_usage_percent: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemMetrics {
    pub host: HostInfo,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub top_processes: Vec<ProcessInfo>,
}

/// Gathers the complete SystemMetrics snapshot.
pub fn collect_metrics() -> SystemMetrics {
    info!("Starting system metrics collection...");

    // 1. Initialize sysinfo system.
    // We only refresh CPU, memory, and processes to keep collection lightweight and fast.
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::new().with_cpu_usage())
            .with_memory(sysinfo::MemoryRefreshKind::new().with_ram())
            .with_processes(sysinfo::ProcessRefreshKind::new().with_cpu().with_memory())
    );

    // QUIRK: To get a accurate non-zero CPU usage reading, we must wait a short duration
    // and refresh CPU information twice, as CPU usage is calculated as a delta over time.
    debug!("Refreshing CPU ticks (Pass 1)");
    sys.refresh_cpu_all();
    thread::sleep(Duration::from_millis(150));
    debug!("Refreshing CPU ticks (Pass 2) and memory/processes");
    sys.refresh_all();

    // 2. Extract Host Information
    let host = HostInfo {
        os_name: System::name().unwrap_or_else(|| "unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
    };
    debug!(os = %host.os_name, version = %host.os_version, "Gathered OS host details");

    // 3. Extract CPU Metrics
    let cpu = CpuInfo {
        usage_percent: sys.global_cpu_usage(),
        logical_cores: sys.cpus().len(),
        physical_cores: sys.physical_core_count().unwrap_or(sys.cpus().len()),
    };
    debug!(usage = %cpu.usage_percent, cores = %cpu.logical_cores, "Gathered CPU utilization");

    // 4. Extract Memory Metrics
    let total_mem = sys.total_memory();
    let free_mem = sys.free_memory();
    let used_mem = sys.used_memory();
    let memory = MemoryInfo {
        total_bytes: total_mem,
        available_bytes: free_mem,
        used_bytes: used_mem,
        usage_percent: if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        },
    };
    debug!(used_pct = %memory.usage_percent, "Gathered Memory allocation");

    // 5. Extract Disk Metrics
    let disks_list = Disks::new_with_refreshed_list();
    let disks = disks_list
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            DiskInfo {
                name: disk.name().to_string_lossy().into_owned(),
                mount_point: disk.mount_point().to_string_lossy().into_owned(),
                total_bytes: total,
                available_bytes: available,
                usage_percent: if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();
    debug!(disk_count = %disks.len(), "Gathered Storage volumes");

    // 6. Extract Top Resource-Consuming Processes (Optimized filtration)
    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, proc)| {
            ProcessInfo {
                pid: pid.as_u32(),
                name: proc.name().to_string_lossy().into_owned(),
                memory_bytes: proc.memory(),
                cpu_usage_percent: proc.cpu_usage(),
            }
        })
        .collect();

    // Take top 15 by memory
    processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
    let mut top_resources = processes.iter().take(15).cloned().collect::<Vec<_>>();

    // Take top 15 by CPU
    processes.sort_by(|a, b| {
        b.cpu_usage_percent
            .partial_cmp(&a.cpu_usage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut top_cpu = processes.iter().take(15).cloned().collect::<Vec<_>>();

    // Merge and de-duplicate by PID
    top_resources.append(&mut top_cpu);
    let mut unique_processes = Vec::new();
    let mut seen_pids = HashSet::new();
    for proc in top_resources {
        if seen_pids.insert(proc.pid) {
            unique_processes.push(proc);
        }
    }

    info!(process_count = %unique_processes.len(), "System metrics collection complete.");
    SystemMetrics {
        host,
        cpu,
        memory,
        disks,
        top_processes: unique_processes,
    }
}

/// Formats the SystemMetrics into a pretty-printed or compact JSON string.
pub fn get_metrics_json(pretty: bool) -> Result<String, serde_json::Error> {
    let metrics = collect_metrics();
    if pretty {
        serde_json::to_string_pretty(&metrics)
    } else {
        serde_json::to_string(&metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_metrics() {
        let metrics = collect_metrics();

        // Host OS assertions
        assert!(!metrics.host.os_name.is_empty(), "OS name should not be empty");
        assert!(!metrics.host.hostname.is_empty(), "Hostname should not be empty");

        // CPU assertions
        assert!(metrics.cpu.logical_cores > 0, "Logical cores should be > 0");
        assert!(metrics.cpu.usage_percent >= 0.0, "CPU usage should be >= 0%");

        // Memory assertions
        assert!(metrics.memory.total_bytes > 0, "Total RAM should be > 0");
        assert!(metrics.memory.usage_percent >= 0.0 && metrics.memory.usage_percent <= 100.0, "RAM usage % must be between 0 and 100");

        // Disk assertions
        // (May be empty on some containerized testing setups, but generally populated on physical devices)
        for disk in &metrics.disks {
            assert!(!disk.mount_point.is_empty(), "Disk mount point should not be empty");
            assert!(disk.total_bytes > 0, "Disk total bytes should be > 0");
        }

        // Process assertions
        assert!(!metrics.top_processes.is_empty(), "Top processes list should not be empty");
        for proc in &metrics.top_processes {
            assert!(proc.pid > 0, "PID should be > 0");
            assert!(!proc.name.is_empty(), "Process name should not be empty");
        }
    }

    #[test]
    fn test_get_metrics_json() {
        let json_result = get_metrics_json(false);
        assert!(json_result.is_ok(), "JSON serialization should succeed");
        let json_str = json_result.unwrap();
        assert!(json_str.contains("\"host\""), "JSON should contain host field");
        assert!(json_str.contains("\"cpu\""), "JSON should contain cpu field");
        assert!(json_str.contains("\"memory\""), "JSON should contain memory field");
    }
}
