import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Interface definitions matching Rust serialization targets
interface HostInfo {
  hostname: string;
  os_name: string;
  os_version: string;
}

interface CpuInfo {
  usage_percent: number;
  physical_cores: number;
  logical_cores: number;
}

interface MemoryInfo {
  total_bytes: number;
  used_bytes: number;
  usage_percent: number;
}

interface DiskInfo {
  name: string;
  mount_point: string;
  total_bytes: number;
  free_bytes: number;
  usage_percent: number;
}

interface ProcessInfo {
  pid: number;
  name: string;
  cpu_usage_percent: number;
  memory_bytes: number;
}

interface SystemMetrics {
  host: HostInfo;
  cpu: CpuInfo;
  memory: MemoryInfo;
  disks: DiskInfo[];
  top_processes: ProcessInfo[];
}

interface Task {
  id: string;
  action: string;
  parameters: Record<string, string>;
  dependencies: string[];
  requires_privilege: boolean;
}

interface DiagnosisResult {
  intent: string;
  confidence: number;
  root_cause: string;
  severity: string;
  explanation: string;
  action_plan: Task[];
}

export default function App() {
  const [activeTab, setActiveTab] = useState<"dashboard" | "diagnose" | "logs">("dashboard");
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [metricsError, setMetricsError] = useState<string | null>(null);
  const [healthStatus, setHealthStatus] = useState<"online" | "offline">("online");

  // NLU Search & Diagnosis State
  const [queryText, setQueryText] = useState("");
  const [diagnosing, setDiagnosing] = useState(false);
  const [diagnosis, setDiagnosis] = useState<DiagnosisResult | null>(null);
  const [diagnosisError, setDiagnosisError] = useState<string | null>(null);

  // Execution State
  const [executing, setExecuting] = useState(false);
  const [execStatus, setExecStatus] = useState<string | null>(null);
  const [taskStatuses, setTaskStatuses] = useState<Record<string, "pending" | "running" | "success" | "failed" | "skipped">>({});
  const [recentFailures, setRecentFailures] = useState<string[]>([]);

  // Periodic Telemetry Poll
  useEffect(() => {
    fetchMetrics();
    const interval = setInterval(fetchMetrics, 3000);
    return () => clearInterval(interval);
  }, []);

  // Fetch metrics whenever active tab changes to logs to load recent failure profiles
  useEffect(() => {
    if (activeTab === "logs" && diagnosis?.intent) {
      loadRecentFailures(diagnosis.intent);
    }
  }, [activeTab, diagnosis]);

  const fetchMetrics = async () => {
    try {
      const data = await invoke<SystemMetrics>("get_system_telemetry");
      setMetrics(data);
      setMetricsError(null);
      setHealthStatus("online");
    } catch (err: any) {
      setMetricsError(err.toString());
      setHealthStatus("offline");
    }
  };

  const handleDiagnose = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!queryText.trim()) return;

    setDiagnosing(true);
    setDiagnosis(null);
    setDiagnosisError(null);
    setExecStatus(null);
    setTaskStatuses({});

    try {
      const result = await invoke<DiagnosisResult>("diagnose_query", { query: queryText });
      setDiagnosis(result);

      // Initialize all tasks in the action plan to 'pending' state
      const initialStatuses: Record<string, "pending"> = {};
      result.action_plan.forEach((task) => {
        initialStatuses[task.id] = "pending";
      });
      setTaskStatuses(initialStatuses);
      
      // Load recent failure telemetry logs for this intent category
      loadRecentFailures(result.intent);
    } catch (err: any) {
      setDiagnosisError(err.toString());
    } finally {
      setDiagnosing(false);
    }
  };

  const loadRecentFailures = async (intent: string) => {
    try {
      const failures = await invoke<string[]>("get_recent_failures", { intent });
      setRecentFailures(failures);
    } catch (err) {
      console.error("Failed to load recent failures:", err);
    }
  };

  const runSimulation = async () => {
    if (!diagnosis) return;
    setExecuting(true);
    setExecStatus("Initiating safe dry-run simulation...");

    try {
      const msg = await invoke<string>("run_repair", {
        tasks: diagnosis.action_plan,
        dryRun: true,
        rawInput: queryText,
        intent: diagnosis.intent,
      });
      setExecStatus(`[SIMULATION SUCCESS] ${msg}`);
    } catch (err: any) {
      setExecStatus(`[SIMULATION BLOCKED] Safety Guard warning: ${err}`);
    } finally {
      setExecuting(false);
    }
  };

  const executeRepair = async () => {
    if (!diagnosis) return;
    setExecuting(true);
    setExecStatus("Executing repairs sequentially...");

    // Set all tasks status to running for visual feedback
    const statuses = { ...taskStatuses };
    diagnosis.action_plan.forEach((task) => {
      statuses[task.id] = "running";
    });
    setTaskStatuses(statuses);

    try {
      const msg = await invoke<string>("run_repair", {
        tasks: diagnosis.action_plan,
        dryRun: false,
        rawInput: queryText,
        intent: diagnosis.intent,
      });

      // Update all tasks status to successful
      const successStatuses: Record<string, "success"> = {};
      diagnosis.action_plan.forEach((task) => {
        successStatuses[task.id] = "success";
      });
      setTaskStatuses(successStatuses);
      setExecStatus(`[REPAIR COMPLETE] ${msg}`);
    } catch (err: any) {
      // Set tasks status to failed/skipped
      const errStatuses: Record<string, "failed" | "skipped"> = {};
      diagnosis.action_plan.forEach((task, index) => {
        if (index === 0) {
          errStatuses[task.id] = "failed";
        } else {
          errStatuses[task.id] = "skipped";
        }
      });
      setTaskStatuses(errStatuses);
      setExecStatus(`[REPAIR FAILED] ${err}`);
    } finally {
      setExecuting(false);
      loadRecentFailures(diagnosis.intent);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  return (
    <div className="app-container">
      {/* Sidebar Section */}
      <aside className="sidebar">
        <div className="logo-section">
          <svg className="logo-icon" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
          </svg>
          <span className="logo-text">PRISM</span>
        </div>

        <nav className="sidebar-nav">
          <div className={`nav-item ${activeTab === "dashboard" ? "active" : ""}`} onClick={() => setActiveTab("dashboard")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="3" y="3" width="7" height="9" />
              <rect x="14" y="3" width="7" height="5" />
              <rect x="14" y="12" width="7" height="9" />
              <rect x="3" y="16" width="7" height="5" />
            </svg>
            System Monitor
          </div>

          <div className={`nav-item ${activeTab === "diagnose" ? "active" : ""}`} onClick={() => setActiveTab("diagnose")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
              <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
              <line x1="12" y1="22.08" x2="12" y2="12" />
            </svg>
            AI Diagnose Studio
          </div>

          <div className={`nav-item ${activeTab === "logs" ? "active" : ""}`} onClick={() => setActiveTab("logs")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="16" y1="13" x2="8" y2="13" />
              <line x1="16" y1="17" x2="8" y2="17" />
              <polyline points="10 9 9 9 8 9" />
            </svg>
            History & Logs
          </div>
        </nav>

        <div className="sidebar-footer">
          <div className="health-status">
            <span className={`status-dot ${healthStatus}`} />
            <span>AI Core: {healthStatus.toUpperCase()}</span>
          </div>
        </div>
      </aside>

      {/* Main Panel Section */}
      <main className="main-content">
        {activeTab === "dashboard" && (
          <>
            <header className="page-header">
              <h1 className="page-title">System telemetry</h1>
              <p className="page-subtitle">Real-time resource logs and hardware status diagnostics</p>
            </header>

            {metricsError && (
              <div className="panel-card" style={{ borderColor: "var(--color-danger)", marginBottom: "20px" }}>
                <p style={{ color: "var(--color-danger)" }}>Error collecting system stats: {metricsError}</p>
              </div>
            )}

            {metrics && (
              <>
                <div className="telemetry-grid">
                  {/* CPU usage */}
                  <div className="metric-card">
                    <div className="metric-header">
                      <span className="metric-title">Processor (CPU)</span>
                      <svg className="metric-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="4" y="4" width="16" height="16" rx="2" />
                        <rect x="9" y="9" width="6" height="6" />
                        <line x1="9" y1="1" x2="9" y2="4" />
                        <line x1="15" y1="1" x2="15" y2="4" />
                        <line x1="9" y1="20" x2="9" y2="23" />
                        <line x1="15" y1="20" x2="15" y2="23" />
                        <line x1="20" y1="9" x2="23" y2="9" />
                        <line x1="20" y1="15" x2="23" y2="15" />
                        <line x1="1" y1="9" x2="4" y2="9" />
                        <line x1="1" y1="15" x2="4" y2="15" />
                      </svg>
                    </div>
                    <div className="metric-body">
                      <span className="metric-value">{metrics.cpu.usage_percent.toFixed(1)}%</span>
                      <div className="progress-bar-container">
                        <div
                          className="progress-bar"
                          style={{
                            width: `${metrics.cpu.usage_percent}%`,
                            backgroundColor: metrics.cpu.usage_percent > 85 ? "var(--color-danger)" : "var(--color-primary)",
                          }}
                        />
                      </div>
                      <span className="metric-meta">
                        {metrics.cpu.physical_cores} Cores / {metrics.cpu.logical_cores} Threads
                      </span>
                    </div>
                  </div>

                  {/* RAM usage */}
                  <div className="metric-card">
                    <div className="metric-header">
                      <span className="metric-title">System Memory (RAM)</span>
                      <svg className="metric-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M6 19v-3h12v3c0 .55-.45 1-1 1H7c-.55 0-1-.45-1-1zm0-7V9c0-.55.45-1 1-1h10c.55 0 1 .45 1 1v3H6z" />
                        <line x1="10" y1="4" x2="14" y2="4" />
                        <line x1="12" y1="4" x2="12" y2="8" />
                      </svg>
                    </div>
                    <div className="metric-body">
                      <span className="metric-value">{metrics.memory.usage_percent.toFixed(1)}%</span>
                      <div className="progress-bar-container">
                        <div
                          className="progress-bar"
                          style={{
                            width: `${metrics.memory.usage_percent}%`,
                            backgroundColor: metrics.memory.usage_percent > 85 ? "var(--color-danger)" : "var(--color-primary)",
                          }}
                        />
                      </div>
                      <span className="metric-meta">
                        {formatBytes(metrics.memory.used_bytes)} used / {formatBytes(metrics.memory.total_bytes)} total
                      </span>
                    </div>
                  </div>

                  {/* Storage usage */}
                  {metrics.disks.length > 0 && (
                    <div className="metric-card">
                      <div className="metric-header">
                        <span className="metric-title">Storage Drive ({metrics.disks[0].name})</span>
                        <svg className="metric-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
                          <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
                          <line x1="6" y1="6" x2="6.01" y2="6" />
                          <line x1="6" y1="18" x2="6.01" y2="18" />
                        </svg>
                      </div>
                      <div className="metric-body">
                        <span className="metric-value">{metrics.disks[0].usage_percent.toFixed(1)}%</span>
                        <div className="progress-bar-container">
                          <div
                            className="progress-bar"
                            style={{
                              width: `${metrics.disks[0].usage_percent}%`,
                              backgroundColor: metrics.disks[0].usage_percent > 90 ? "var(--color-danger)" : "var(--color-primary)",
                            }}
                          />
                        </div>
                        <span className="metric-meta">
                          {formatBytes(metrics.disks[0].free_bytes)} free / {formatBytes(metrics.disks[0].total_bytes)} total
                        </span>
                      </div>
                    </div>
                  )}
                </div>

                <div className="info-section-grid">
                  {/* Heavy Resource Consuming Processes */}
                  <div className="panel-card">
                    <h3 className="panel-title">
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M12 2v20M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
                      </svg>
                      Resource Consuming Processes
                    </h3>
                    <table className="process-table">
                      <thead>
                        <tr>
                          <th>PID</th>
                          <th>Process Name</th>
                          <th>CPU Usage</th>
                          <th>Memory RAM</th>
                        </tr>
                      </thead>
                      <tbody>
                        {metrics.top_processes.slice(0, 6).map((proc) => (
                          <tr key={proc.pid}>
                            <td>{proc.pid}</td>
                            <td style={{ fontWeight: 600 }}>{proc.name}</td>
                            <td>{proc.cpu_usage_percent.toFixed(1)}%</td>
                            <td>{formatBytes(proc.memory_bytes)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>

                  {/* Device Information */}
                  <div className="panel-card">
                    <h3 className="panel-title">
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" />
                        <line x1="7" y1="2" x2="7" y2="22" />
                        <line x1="17" y1="2" x2="17" y2="22" />
                        <line x1="2" y1="12" x2="22" y2="12" />
                        <line x1="2" y1="7" x2="22" y2="7" />
                        <line x1="2" y1="17" x2="22" y2="17" />
                      </svg>
                      Device Profile
                    </h3>
                    <div className="host-info-list">
                      <div className="info-row">
                        <span className="info-label">Hostname</span>
                        <span className="info-value">{metrics.host.hostname}</span>
                      </div>
                      <div className="info-row">
                        <span className="info-label">Platform OS</span>
                        <span className="info-value">{metrics.host.os_name}</span>
                      </div>
                      <div className="info-row">
                        <span className="info-label">OS Kernel Version</span>
                        <span className="info-value">{metrics.host.os_version}</span>
                      </div>
                    </div>
                  </div>
                </div>
              </>
            )}
          </>
        )}

        {activeTab === "diagnose" && (
          <>
            <header className="page-header">
              <h1 className="page-title">AI Diagnosis Studio</h1>
              <p className="page-subtitle">State system issues in natural language to run autonomous inspections</p>
            </header>

            {/* Input Search Console */}
            <form onSubmit={handleDiagnose} className="search-console">
              <input
                type="text"
                className="search-input"
                placeholder='Describe the problem (e.g. "My laptop is running slow", "Free up 20GB space")...'
                value={queryText}
                onChange={(e) => setQueryText(e.target.value)}
                disabled={diagnosing || executing}
              />
              <button type="submit" className="btn-primary" disabled={diagnosing || executing || !queryText.trim()}>
                {diagnosing ? (
                  <>
                    <svg className="spinner" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <line x1="12" y1="2" x2="12" y2="6" />
                      <line x1="12" y1="18" x2="12" y2="22" />
                      <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
                      <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
                      <line x1="2" y1="12" x2="6" y2="12" />
                      <line x1="18" y1="12" x2="22" y2="12" />
                      <line x1="6.34" y1="17.66" x2="8.46" y2="15.54" />
                      <line x1="15.54" y1="8.46" x2="17.66" y2="6.34" />
                    </svg>
                    Inspecting...
                  </>
                ) : (
                  <>
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <circle cx="11" cy="11" r="8" />
                      <line x1="21" y1="21" x2="16.65" y2="16.65" />
                    </svg>
                    Diagnose
                  </>
                )}
              </button>
            </form>

            {diagnosisError && (
              <div className="panel-card" style={{ borderColor: "var(--color-danger)", marginBottom: "20px" }}>
                <p style={{ color: "var(--color-danger)", fontWeight: 500 }}>Diagnosis Engine Error:</p>
                <p style={{ color: "var(--text-secondary)", marginTop: "4px" }}>{diagnosisError}</p>
              </div>
            )}

            {/* Diagnosis results Display */}
            {diagnosis && (
              <div className="diagnosis-container">
                <div className="diag-summary-card">
                  <div className="diag-title-row">
                    <div className="diag-intent-group">
                      <span className="diag-intent-label">Resolved Intent Category</span>
                      <span className="diag-intent-val">
                        {diagnosis.intent.toUpperCase().replace("_", " ")} ({Math.round(diagnosis.confidence * 100)}% Match)
                      </span>
                    </div>
                    <span className={`severity-badge ${diagnosis.severity.toLowerCase()}`}>
                      Severity: {diagnosis.severity}
                    </span>
                  </div>

                  <div className="diag-root-cause">
                    <strong>Root Cause Diagnosis:</strong> {diagnosis.root_cause}
                  </div>
                  <p className="diag-explanation">{diagnosis.explanation}</p>
                </div>

                <div className="panel-card">
                  <h3 className="action-plan-title">Suggested Remediation Action Plan</h3>

                  <div className="task-list">
                    {diagnosis.action_plan.map((task) => (
                      <div key={task.id} className="task-item">
                        <div className="task-left">
                          <span className="task-action">{task.action.toUpperCase()}</span>
                          <span className="task-params">
                            {Object.entries(task.parameters)
                              .map(([k, v]) => `${k}: ${v}`)
                              .join(" | ")}
                          </span>
                        </div>
                        <div className={`task-status-icon ${taskStatuses[task.id]}`}>
                          {taskStatuses[task.id] === "pending" && (
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <circle cx="12" cy="12" r="10" />
                              <line x1="12" y1="8" x2="12" y2="12" />
                              <line x1="12" y1="16" x2="12.01" y2="16" />
                            </svg>
                          )}
                          {taskStatuses[task.id] === "running" && (
                            <svg className="spinner" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <line x1="12" y1="2" x2="12" y2="6" />
                              <line x1="12" y1="18" x2="12" y2="22" />
                              <line x1="2" y1="12" x2="6" y2="12" />
                              <line x1="18" y1="12" x2="22" y2="12" />
                            </svg>
                          )}
                          {taskStatuses[task.id] === "success" && (
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                              <polyline points="22 4 12 14.01 9 11.01" />
                            </svg>
                          )}
                          {taskStatuses[task.id] === "failed" && (
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <circle cx="12" cy="12" r="10" />
                              <line x1="15" y1="9" x2="9" y2="15" />
                              <line x1="9" y1="9" x2="15" y2="15" />
                            </svg>
                          )}
                          {taskStatuses[task.id] === "skipped" && (
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <circle cx="12" cy="12" r="10" />
                              <line x1="8" y1="12" x2="16" y2="12" />
                            </svg>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>

                  {execStatus && (
                    <div
                      className="info-row"
                      style={{
                        marginTop: "16px",
                        padding: "12px",
                        backgroundColor: "rgba(255, 255, 255, 0.02)",
                        border: "1px solid var(--border-color)",
                        borderRadius: "6px",
                        color: execStatus.includes("FAILED") || execStatus.includes("BLOCKED") ? "var(--color-danger)" : "var(--text-primary)",
                      }}
                    >
                      {execStatus}
                    </div>
                  )}

                  <div className="action-bar">
                    <button className="btn-secondary" onClick={runSimulation} disabled={executing}>
                      Validate Safely (Dry-Run)
                    </button>
                    <button className="btn-primary" onClick={executeRepair} disabled={executing}>
                      Execute Repair Actions
                    </button>
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {activeTab === "logs" && (
          <>
            <header className="page-header">
              <h1 className="page-title">Diagnostics Telemetry & History Logs</h1>
              <p className="page-subtitle">Retrieve recent task execution records from the SQLite database</p>
            </header>

            <div className="panel-card">
              <div className="history-header">
                <h3 className="panel-title">Recent SQLite Failure Log History</h3>
                <button
                  className="btn-secondary"
                  onClick={() => diagnosis?.intent && loadRecentFailures(diagnosis.intent)}
                  style={{ padding: "6px 14px", fontSize: "12px" }}
                >
                  Refresh Logs
                </button>
              </div>

              <div className="history-table-container">
                {recentFailures.length === 0 ? (
                  <div style={{ padding: "32px", textAlign: "center", color: "var(--text-muted)" }}>
                    No system failure logs found for the current intent query session.
                  </div>
                ) : (
                  <table className="history-table">
                    <thead>
                      <tr>
                        <th>#</th>
                        <th>Error Message Details</th>
                        <th>Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {recentFailures.map((failure, index) => (
                        <tr key={index}>
                          <td style={{ width: "50px", color: "var(--text-muted)" }}>{index + 1}</td>
                          <td style={{ fontFamily: "monospace", color: "var(--color-danger)" }}>{failure}</td>
                          <td style={{ width: "120px" }}>
                            <span className="status-badge failed">FAILED RECORD</span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </div>
          </>
        )}
      </main>
    </div>
  );
}
