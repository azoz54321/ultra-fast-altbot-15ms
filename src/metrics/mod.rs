use hdrhistogram::serialization::Serializer;
use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Execution metrics for Phase 3
#[derive(Debug, Clone, Copy)]
pub struct ExecutionMetrics {
    pub emitted_intents: u64,
    pub dropped_intents: u64,
    pub ack_count: u64,
    pub fill_count: u64,
    pub gate_block_count: u64,
    pub cooldown_block_count: u64,
}

/// Histogram summary for JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramSummary {
    pub count: u64,
    pub min: u64,
    pub max: u64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub p99_9: u64,
    pub throughput_avg: f64,
    // Phase 3 additions
    pub emitted_intents: u64,
    pub dropped_intents: u64,
    pub ack_count: u64,
    pub fill_count: u64,
    pub gate_block_count: u64,
    pub cooldown_block_count: u64,
}

/// Metrics collector using HDR histogram for latency tracking
pub struct MetricsCollector {
    histogram: Histogram<u64>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    /// max_value: maximum latency in microseconds to track (e.g., 100_000 = 100ms)
    /// significant_figures: precision (3 = 0.1% precision)
    pub fn new(max_value: u64, significant_figures: u8) -> Result<Self, String> {
        let histogram = Histogram::new_with_max(max_value, significant_figures)
            .map_err(|e| format!("Failed to create histogram: {}", e))?;

        Ok(Self { histogram })
    }

    /// Record a latency measurement in microseconds (hot-path compatible)
    pub fn record(&mut self, latency_micros: u64) -> Result<(), String> {
        self.histogram
            .record(latency_micros)
            .map_err(|e| format!("Failed to record latency: {}", e))
    }

    /// Get percentile value in microseconds
    pub fn percentile(&self, percentile: f64) -> u64 {
        self.histogram.value_at_quantile(percentile)
    }

    /// Get total count of recorded samples
    pub fn count(&self) -> u64 {
        self.histogram.len()
    }

    /// Print summary statistics
    pub fn print_summary(&self) {
        println!("=== Latency Summary ===");
        println!("Total samples: {}", self.count());
        println!(
            "p50: {} µs ({:.2} ms)",
            self.percentile(0.50),
            self.percentile(0.50) as f64 / 1000.0
        );
        println!(
            "p95: {} µs ({:.2} ms)",
            self.percentile(0.95),
            self.percentile(0.95) as f64 / 1000.0
        );
        println!(
            "p99: {} µs ({:.2} ms)",
            self.percentile(0.99),
            self.percentile(0.99) as f64 / 1000.0
        );
        println!(
            "p99.9: {} µs ({:.2} ms)",
            self.percentile(0.999),
            self.percentile(0.999) as f64 / 1000.0
        );
        println!(
            "max: {} µs ({:.2} ms)",
            self.histogram.max(),
            self.histogram.max() as f64 / 1000.0
        );
        println!(
            "min: {} µs ({:.2} ms)",
            self.histogram.min(),
            self.histogram.min() as f64 / 1000.0
        );
    }

    /// Write histogram to file in HDR histogram format
    pub fn write_to_file(&self, path: &Path) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let mut file = File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;

        // Write HDR histogram in text format
        let mut serializer = hdrhistogram::serialization::V2Serializer::new();
        let mut output = Vec::new();
        serializer
            .serialize(&self.histogram, &mut output)
            .map_err(|e| format!("Failed to serialize histogram: {}", e))?;

        file.write_all(&output)
            .map_err(|e| format!("Failed to write histogram: {}", e))?;

        Ok(())
    }

    /// Generate histogram summary with execution metrics
    pub fn generate_summary(
        &self,
        duration_secs: f64,
        exec_metrics: &ExecutionMetrics,
    ) -> HistogramSummary {
        let throughput_avg = if duration_secs > 0.0 {
            self.count() as f64 / duration_secs
        } else {
            0.0
        };

        HistogramSummary {
            count: self.count(),
            min: self.histogram.min(),
            max: self.histogram.max(),
            p50: self.percentile(0.50),
            p95: self.percentile(0.95),
            p99: self.percentile(0.99),
            p99_9: self.percentile(0.999),
            throughput_avg,
            emitted_intents: exec_metrics.emitted_intents,
            dropped_intents: exec_metrics.dropped_intents,
            ack_count: exec_metrics.ack_count,
            fill_count: exec_metrics.fill_count,
            gate_block_count: exec_metrics.gate_block_count,
            cooldown_block_count: exec_metrics.cooldown_block_count,
        }
    }

    /// Write histogram summary to JSON file with execution metrics
    pub fn write_summary_json(
        &self,
        path: &Path,
        duration_secs: f64,
        exec_metrics: &ExecutionMetrics,
    ) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let summary = self.generate_summary(duration_secs, exec_metrics);
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

        fs::write(path, json).map_err(|e| format!("Failed to write JSON file: {}", e))?;

        Ok(())
    }

    /// Write human-readable text summary
    pub fn write_text_summary(
        &self,
        path: &Path,
        duration_secs: f64,
        num_ticks: usize,
        trigger_count: usize,
        exec_metrics: &ExecutionMetrics,
    ) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let throughput = if duration_secs > 0.0 {
            num_ticks as f64 / duration_secs
        } else {
            0.0
        };

        let p95_us = self.percentile(0.95);
        let p95_ms = p95_us as f64 / 1000.0;
        let pass_status = if p95_us <= 15000 { "PASS" } else { "WARN" };

        let mut output = String::new();
        output.push_str("=== Ultra-Fast Altbot Shadow Benchmark Summary ===\n\n");
        output.push_str(&format!("Benchmark Duration: {:.2}s\n", duration_secs));
        output.push_str(&format!("Total Ticks Processed: {}\n", num_ticks));
        output.push_str(&format!("Throughput: {:.0} ticks/sec\n", throughput));
        output.push_str(&format!("Triggers: {}\n\n", trigger_count));

        output.push_str("=== Latency Statistics ===\n");
        output.push_str(&format!("  Total samples: {}\n", self.count()));
        output.push_str(&format!("  p50:  {} µs ({:.2} ms)\n", self.percentile(0.50), self.percentile(0.50) as f64 / 1000.0));
        output.push_str(&format!("  p95:  {} µs ({:.2} ms) [{}]\n", p95_us, p95_ms, pass_status));
        output.push_str(&format!("  p99:  {} µs ({:.2} ms)\n", self.percentile(0.99), self.percentile(0.99) as f64 / 1000.0));
        output.push_str(&format!("  p99.9: {} µs ({:.2} ms)\n", self.percentile(0.999), self.percentile(0.999) as f64 / 1000.0));
        output.push_str(&format!("  max:  {} µs ({:.2} ms)\n", self.histogram.max(), self.histogram.max() as f64 / 1000.0));
        output.push_str(&format!("  min:  {} µs ({:.2} ms)\n\n", self.histogram.min(), self.histogram.min() as f64 / 1000.0));

        output.push_str("=== Execution Mock Metrics ===\n");
        output.push_str(&format!("  Emitted Intents: {}\n", exec_metrics.emitted_intents));
        output.push_str(&format!("  Dropped Intents: {}\n", exec_metrics.dropped_intents));
        output.push_str(&format!("  Acks Received: {}\n", exec_metrics.ack_count));
        output.push_str(&format!("  Fills Received: {}\n", exec_metrics.fill_count));
        output.push_str(&format!("  Gate Blocks: {}\n", exec_metrics.gate_block_count));
        output.push_str(&format!("  Cooldown Blocks: {}\n\n", exec_metrics.cooldown_block_count));

        output.push_str("=== Consistency Check ===\n");
        let consistency = if exec_metrics.fill_count <= exec_metrics.ack_count && exec_metrics.ack_count <= exec_metrics.emitted_intents {
            "✓ PASS (fills <= acks <= emitted_intents)"
        } else {
            "✗ FAIL (inconsistent metrics)"
        };
        output.push_str(&format!("  {}\n\n", consistency));

        output.push_str("=== Soft Gating Result ===\n");
        if p95_us <= 15000 {
            output.push_str(&format!("  ✓ PASS: p95 latency ({:.2} ms) <= 15.00 ms target\n", p95_ms));
        } else {
            output.push_str(&format!("  ⚠ WARN: p95 latency ({:.2} ms) > 15.00 ms target\n", p95_ms));
            output.push_str("  (benchmark still exits 0 for non-failing gate)\n");
        }

        fs::write(path, output).map_err(|e| format!("Failed to write text summary: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new(100_000, 3).unwrap();

        // Record some sample latencies
        collector.record(100).unwrap();
        collector.record(200).unwrap();
        collector.record(500).unwrap();
        collector.record(1000).unwrap();

        assert_eq!(collector.count(), 4);
        assert!(collector.percentile(0.5) >= 100);
    }
}
