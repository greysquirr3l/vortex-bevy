//! Linux process-memory observations for bounded profiling.
//!
//! This adapter reads only `/proc/self/status` and returns aggregate process
//! counters. It does not inspect heap allocations, source identities, open
//! files, environment variables, or other process metadata.

use std::io;

#[cfg(target_os = "linux")]
use std::fs;

use thiserror::Error;

#[cfg(target_os = "linux")]
const STATUS_PATH: &str = "/proc/self/status";
#[cfg(target_os = "linux")]
const BYTES_PER_KIB: u64 = 1024;

/// Aggregate process-memory counters sampled at one instant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessMemorySnapshot {
    resident_bytes: u64,
    peak_resident_bytes: u64,
}

impl ProcessMemorySnapshot {
    /// Returns the resident-set size reported by the operating system in bytes.
    #[must_use]
    pub const fn resident_bytes(self) -> u64 {
        self.resident_bytes
    }

    /// Returns the high-water resident-set size reported by the operating system in bytes.
    #[must_use]
    pub const fn peak_resident_bytes(self) -> u64 {
        self.peak_resident_bytes
    }
}

/// Fail-closed errors while sampling process-memory counters.
#[derive(Debug, Error)]
pub enum ProcessMemoryError {
    /// The current target does not expose the Linux `/proc` process-status interface.
    #[error("Linux process-memory observation is unavailable on this platform")]
    UnsupportedPlatform,
    /// The operating system status record could not be read.
    #[error("Linux process-memory status is unavailable: {source}")]
    StatusUnavailable {
        /// The underlying operating-system error.
        source: io::Error,
    },
    /// A required status metric did not appear exactly once.
    #[error("Linux process-memory status is missing required metric {metric}")]
    MissingMetric {
        /// The required `/proc` key.
        metric: &'static str,
    },
    /// A required status metric did not have a bounded KiB representation.
    #[error("Linux process-memory status has an invalid {metric} metric")]
    InvalidMetric {
        /// The invalid `/proc` key.
        metric: &'static str,
    },
}

/// Linux reader for aggregate resident-set memory observations.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxProcessMemoryReader;

impl LinuxProcessMemoryReader {
    /// Reads the current process RSS and peak RSS from `/proc/self/status`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vortex_core::adapters::secondary::linux_process_memory::LinuxProcessMemoryReader;
    ///
    /// if let Ok(snapshot) = LinuxProcessMemoryReader.snapshot() {
    ///     assert!(snapshot.peak_resident_bytes() >= snapshot.resident_bytes());
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a typed error on unsupported targets, unavailable process status,
    /// absent metrics, malformed metrics, or byte-conversion overflow.
    pub fn snapshot(&self) -> Result<ProcessMemorySnapshot, ProcessMemoryError> {
        read_current_status()
    }
}

#[cfg(target_os = "linux")]
fn read_current_status() -> Result<ProcessMemorySnapshot, ProcessMemoryError> {
    let status = fs::read_to_string(STATUS_PATH)
        .map_err(|source| ProcessMemoryError::StatusUnavailable { source })?;
    parse_status(&status)
}

#[cfg(not(target_os = "linux"))]
const fn read_current_status() -> Result<ProcessMemorySnapshot, ProcessMemoryError> {
    Err(ProcessMemoryError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn parse_status(status: &str) -> Result<ProcessMemorySnapshot, ProcessMemoryError> {
    let resident_bytes = parse_metric(status, "VmRSS:")?;
    let peak_resident_bytes = parse_metric(status, "VmHWM:")?;
    Ok(ProcessMemorySnapshot { resident_bytes, peak_resident_bytes })
}

#[cfg(target_os = "linux")]
fn parse_metric(status: &str, metric: &'static str) -> Result<u64, ProcessMemoryError> {
    let mut value = None;
    for line in status.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some(metric) {
            continue;
        }
        if value.is_some() {
            return Err(ProcessMemoryError::InvalidMetric { metric });
        }
        let number = parts
            .next()
            .ok_or(ProcessMemoryError::InvalidMetric { metric })?
            .parse::<u64>()
            .map_err(|_| ProcessMemoryError::InvalidMetric { metric })?;
        if parts.next() != Some("kB") || parts.next().is_some() {
            return Err(ProcessMemoryError::InvalidMetric { metric });
        }
        value = Some(
            number
                .checked_mul(BYTES_PER_KIB)
                .ok_or(ProcessMemoryError::InvalidMetric { metric })?,
        );
    }
    value.ok_or(ProcessMemoryError::MissingMetric { metric })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{ProcessMemoryError, parse_status};

    #[test]
    fn parses_aggregate_linux_rss_counters_without_retaining_status_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = parse_status("Name:\tvortex\nVmRSS:\t  42 kB\nVmHWM:\t  64 kB\n")?;
        assert_eq!(snapshot.resident_bytes(), 43_008);
        assert_eq!(snapshot.peak_resident_bytes(), 65_536);
        Ok(())
    }

    #[test]
    fn rejects_missing_duplicate_and_malformed_memory_metrics() {
        assert!(matches!(
            parse_status("VmRSS:\t42 kB\n"),
            Err(ProcessMemoryError::MissingMetric { metric: "VmHWM:" })
        ));
        assert!(matches!(
            parse_status("VmRSS:\t42 kB\nVmRSS:\t43 kB\nVmHWM:\t64 kB\n"),
            Err(ProcessMemoryError::InvalidMetric { metric: "VmRSS:" })
        ));
        assert!(matches!(
            parse_status("VmRSS:\t42 bytes\nVmHWM:\t64 kB\n"),
            Err(ProcessMemoryError::InvalidMetric { metric: "VmRSS:" })
        ));
    }
}
