mod config;
mod data_feed;
mod hotpath;
mod metrics;
mod sbe_decoder_ffi;

use clap::Parser;
use config::Config;
use data_feed::TickGenerator;
use hotpath::{HotPath, LatencyMeasurement};
use metrics::MetricsCollector;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Ultra-fast altcoin trading bot
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in shadow benchmark mode
    #[arg(long)]
    bench_shadow: bool,

    /// Number of ticks to generate in benchmark mode
    #[arg(long, default_value = "100000")]
    num_ticks: usize,

    /// Number of symbols to simulate
    #[arg(long, default_value = "300")]
    num_symbols: u32,

    /// Number of symbols per shard (for future sharding support)
    #[arg(long)]
    symbols_per_shard: Option<usize>,

    /// Path to write HDR histogram output
    #[arg(long, default_value = "target/shadow_bench/hdr_histogram.hdr")]
    hist_out: PathBuf,
}

fn main() {
    let args = Args::parse();

    if args.bench_shadow {
        println!("Running in shadow benchmark mode...");
        run_shadow_benchmark(&args);
    } else {
        println!("Running in normal mode (shadow mode enabled by default)...");
        run_normal_mode();
    }
}

/// Run shadow benchmark harness
fn run_shadow_benchmark(args: &Args) {
    let num_ticks = args.num_ticks;
    let num_symbols = args.num_symbols;
    
    println!("Generating {} ticks across {} symbols...", num_ticks, num_symbols);
    
    if let Some(shard_size) = args.symbols_per_shard {
        let num_shards = (num_symbols as usize + shard_size - 1) / shard_size;
        println!("Using {} symbols per shard ({} shards total)", shard_size, num_shards);
    }

    let config = Config::default();
    
    // Generate synthetic ticks
    let generator = TickGenerator::new(num_symbols, num_ticks);
    let ticks = generator.generate();
    println!("Generated {} ticks", ticks.len());

    // Create metrics collector
    let mut metrics = MetricsCollector::new(100_000, 3)
        .expect("Failed to create metrics collector");

    // Create hot-path processor
    let hotpath = Arc::new(HotPath::new(
        config.max_symbols,
        config.return_threshold_pct,
        config.price_window_secs,
    ));

    // Pre-populate price snapshots to ensure we have history for return calculation
    println!("Pre-populating price snapshots...");
    for tick in ticks.iter().take(1000) {
        hotpath.update_snapshot(tick.symbol_id, tick.px_e8, tick.ts_unix_ms);
    }

    // Process ticks and measure latency
    println!("Processing ticks...");
    let mut trigger_count = 0;
    let bench_start = Instant::now();

    for (idx, tick) in ticks.iter().enumerate() {
        let mut measurement = LatencyMeasurement::new();
        
        // Start timing
        measurement.start();

        // Update snapshot (in real system, this happens off hot-path)
        hotpath.update_snapshot(tick.symbol_id, tick.px_e8, tick.ts_unix_ms);

        // Process tick on hot-path
        if let Some(trigger) = hotpath.process_tick(tick) {
            trigger_count += 1;
            if trigger_count <= 10 {
                println!(
                    "Trigger #{}: symbol={} return={:.2}% price={}",
                    trigger_count,
                    trigger.symbol_id,
                    trigger.return_pct,
                    trigger.price_e8 as f64 / 1e8
                );
            }
        }

        // End timing
        measurement.end();

        // Record latency
        if let Err(e) = metrics.record(measurement.duration_micros()) {
            eprintln!("Failed to record metric: {}", e);
        }

        // Progress update
        if (idx + 1) % 10_000 == 0 {
            println!("Processed {}/{} ticks...", idx + 1, num_ticks);
        }
    }

    let bench_duration = bench_start.elapsed();
    let duration_secs = bench_duration.as_secs_f64();
    
    println!("\n=== Benchmark Complete ===");
    println!("Total time: {:.2}s", duration_secs);
    println!("Throughput: {:.0} ticks/sec", num_ticks as f64 / duration_secs);
    println!("Triggers: {}", trigger_count);
    println!();

    // Print metrics summary
    metrics.print_summary();

    // Write histogram to file
    match metrics.write_to_file(&args.hist_out) {
        Ok(_) => println!("\nHistogram written to: {}", args.hist_out.display()),
        Err(e) => eprintln!("Failed to write histogram: {}", e),
    }

    // Write JSON summary
    let json_path = args.hist_out.with_file_name("histogram_summary.json");
    match metrics.write_summary_json(&json_path, duration_secs) {
        Ok(_) => println!("JSON summary written to: {}", json_path.display()),
        Err(e) => eprintln!("Failed to write JSON summary: {}", e),
    }

    println!("\n✓ Shadow benchmark completed successfully");
    std::process::exit(0);
}

/// Run in normal mode (future: connect to real data feed)
fn run_normal_mode() {
    let config = Config::default();
    println!("Configuration: {:?}", config);
    println!();
    println!("Normal mode is not yet implemented.");
    println!("Use --bench-shadow to run the benchmark harness.");
    println!();
    println!("Example:");
    println!("  cargo run --release -- --bench-shadow");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.target_p95_ms, 15);
        assert!(config.shadow_mode);
        assert_eq!(config.return_threshold_pct, 5.0);
    }

    #[test]
    fn test_tick_generation() {
        let generator = TickGenerator::new(10, 100);
        let ticks = generator.generate();
        assert_eq!(ticks.len(), 100);
    }
}
