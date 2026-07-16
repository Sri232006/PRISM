# Phase 3 Plan: System Inspector Crate Implementation

## 1. Objective

The goal of Phase 3 is to implement the **System Inspector** (`diagnostics` crate) in Rust. The Reasoning Engine (Python AI service) needs real-time system metrics to diagnose problems. 

We will:
- Add `sysinfo` and `serde` dependencies to the `diagnostics` crate.
- Implement data structures representing CPU, RAM, OS version, Disks, and Processes.
- Implement the system collector which aggregates this data, sorts the running processes, and returns the **Top N** resource-consuming processes to prevent data bloat.
- Serialize the aggregated metrics into JSON.
- Expose the collection interface to the other Rust workspace crates.
- Write unit tests verifying that metrics are retrieved successfully.

---

## 2. Why `sysinfo` and `serde`?

### Why `sysinfo`?
- **Cross-Platform Compatibility**: It provides a unified Rust API to query system information across Windows, macOS, and Linux. This is crucial for PRISM's cross-platform architecture.
- **Performance**: It accesses operating system APIs directly (via native sys calls or registry/registry queries in Windows) rather than executing shell commands like `wmic` or `ps`, making it extremely fast.

### Why `serde` & `serde_json`?
- **Type-Safe Serialization**: Serde is the de facto standard serialization framework in Rust. We need to serialize our collected system statistics to JSON so they can be passed as arguments over gRPC to the Python NLU/Reasoning engine.

---

## 3. High-Performance Process Filtering

 A typical developer machine has anywhere from **150 to 500 running processes**. Sending all process names, PIDs, memory values, and CPU ticks across gRPC would create significant serialization and network overhead.

To solve this, the `diagnostics` crate will:
1. Fetch all active processes.
2. Filter out zero-resource processes.
3. Sort them descending by memory consumption.
4. Take the **Top 15** processes.
5. Sort them descending by CPU usage and take the **Top 15** processes.
6. Merge and de-duplicate these lists, returning a clean array of the most resource-intensive processes.

---

## 4. Implementation Details

We will configure:
1. `diagnostics/Cargo.toml` with dependencies.
2. `diagnostics/src/lib.rs` with the collection structs and the main `collect_metrics` function.
3. Unit tests inside the file to run via `cargo test -p diagnostics`.
