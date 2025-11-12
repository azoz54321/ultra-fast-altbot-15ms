use hdrhistogram::Histogram;
use hdrhistogram::serialization::Serializer;
use serde::{Serialize, Deserialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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
        println!("p50: {} µs ({:.2} ms)", self.percentile(0.50), self.percentile(0.50) as f64 / 1000.0);
        println!("p95: {} µs ({:.2} ms)", self.percentile(0.95), self.percentile(0.95) as f64 / 1000.0);
        println!("p99: {} µs ({:.2} ms)", self.percentile(0.99), self.percentile(0.99) as f64 / 1000.0);
        println!("p99.9: {} µs ({:.2} ms)", self.percentile(0.999), self.percentile(0.999) as f64 / 1000.0);
        println!("max: {} µs ({:.2} ms)", self.histogram.max(), self.histogram.max() as f64 / 1000.0);
        println!("min: {} µs ({:.2} ms)", self.histogram.min(), self.histogram.min() as f64 / 1000.0);
    }

    /// Write histogram to file in HDR histogram format
    pub fn write_to_file(&self, path: &Path) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let mut file = File::create(path)
            .map_err(|e| format!("Failed to create file: {}", e))?;

        // Write HDR histogram in text format
        let mut serializer = hdrhistogram::serialization::V2Serializer::new();
        let mut output = Vec::new();
        serializer.serialize(&self.histogram, &mut output)
            .map_err(|e| format!("Failed to serialize histogram: {}", e))?;

        file.write_all(&output)
            .map_err(|e| format!("Failed to write histogram: {}", e))?;

        Ok(())
    }

    /// Generate histogram summary
    pub fn generate_summary(&self, duration_secs: f64) -> HistogramSummary {
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
        }
    }

    /// Write histogram summary to JSON file
    pub fn write_summary_json(&self, path: &Path, duration_secs: f64) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let summary = self.generate_summary(duration_secs);
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

        fs::write(path, json)
            .map_err(|e| format!("Failed to write JSON file: {}", e))?;

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
