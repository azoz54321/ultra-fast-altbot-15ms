mod config;
mod data_feed;
mod execution;
mod hotpath;
mod metrics;
mod sbe_decoder_ffi;

use clap::Parser;
use config::Config;
use data_feed::TickGenerator;
use hotpath::{HotPath, LatencyMeasurement};
use metrics::{ExecutionMetrics, MetricsCollector};
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
    use execution::ExecutionMock;
    use std::thread;

    let num_ticks = args.num_ticks;
    let num_symbols = args.num_symbols;

    println!(
        "Generating {} ticks across {} symbols...",
        num_ticks, num_symbols
    );

    if let Some(shard_size) = args.symbols_per_shard {
        let num_shards = (num_symbols as usize).div_ceil(shard_size);
        println!(
            "Using {} symbols per shard ({} shards total)",
            shard_size, num_shards
        );
    }

    let config = Config::default();

    // Generate synthetic ticks with enhanced realism
    println!("Generating synthetic ticks with variable rates and micro-bursts...");
    let generator = TickGenerator::new(num_symbols, num_ticks);
    let ticks = generator.generate();
    println!("Generated {} ticks", ticks.len());

    // Create metrics collector
    let mut metrics =
        MetricsCollector::new(100_000, 3).expect("Failed to create metrics collector");

    // Create execution mock with SPSC channel
    // Queue capacity: 1000 intents, Ack delay: 50us, Fill delay: 100us
    let (exec_mock, intent_tx, event_rx) = ExecutionMock::new(1000, 50, 100);
    let (_submitted_counter, ack_counter, fill_counter) = exec_mock.get_counters();

    // Spawn execution mock thread (off hot-path)
    let exec_handle = thread::spawn(move || {
        exec_mock.run();
    });

    // Create hot-path processor with gates and cooldowns
    let mut hotpath = HotPath::with_config(
        config.max_symbols,
        config.return_threshold_pct,
        config.price_window_secs,
        10,   // max_open_intents
        500,  // cooldown_ms
        1000, // initial_budget
    );

    // Set intent sender for hot path
    hotpath.set_intent_sender(intent_tx.clone());

    // Pre-populate price snapshots to ensure we have history for return calculation
    println!("Pre-populating price snapshots...");
    for tick in ticks.iter().take(1000) {
        hotpath.update_snapshot(tick.symbol_id, tick.px_e8, tick.ts_unix_ms);
    }

    // Spawn thread to consume order events and decrement open_intents on fills
    let hotpath_clone = Arc::new(hotpath);
    let hotpath_for_events = Arc::clone(&hotpath_clone);
    let event_handle = thread::spawn(move || {
        loop {
            match event_rx.try_recv() {
                Ok(event) => {
                    // Decrement open_intents when we receive a Fill event
                    if matches!(event.kind, execution::OrderEventKind::Fill) {
                        hotpath_for_events.decrement_open_intents();
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // No events, continue polling
                    // Note: This is a busy-wait. Consider adding thread::yield_now()
                    // or a small sleep if CPU usage is a concern in production.
                    continue;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    // Channel closed
                    break;
                }
            }
        }
    });

    // Process ticks and measure latency
    println!("Processing ticks...");
    let mut trigger_count = 0;
    let bench_start = Instant::now();

    for (idx, tick) in ticks.iter().enumerate() {
        let mut measurement = LatencyMeasurement::new();

        // Start timing
        measurement.start();

        // Update snapshot (in real system, this happens off hot-path)
        hotpath_clone.update_snapshot(tick.symbol_id, tick.px_e8, tick.ts_unix_ms);

        // Process tick on hot-path (may emit OrderIntent)
        if let Some(trigger) = hotpath_clone.process_tick(tick) {
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

    // Drop intent sender to signal completion
    drop(intent_tx);

    // Wait briefly for execution mock to process remaining intents
    thread::sleep(std::time::Duration::from_millis(200));

    // Get gate metrics
    let gate_metrics = hotpath_clone.get_gate_metrics();
    let ack_count = ack_counter.load(std::sync::atomic::Ordering::Relaxed);
    let fill_count = fill_counter.load(std::sync::atomic::Ordering::Relaxed);

    println!("\n=== Benchmark Complete ===");
    println!("Total time: {:.2}s", duration_secs);
    println!(
        "Throughput: {:.0} ticks/sec",
        num_ticks as f64 / duration_secs
    );
    println!("Triggers: {}", trigger_count);
    println!("\n=== Execution Mock Stats ===");
    println!("Emitted Intents: {}", gate_metrics.emitted_intents);
    println!("Dropped Intents: {}", gate_metrics.dropped_intents);
    println!("Acks Received: {}", ack_count);
    println!("Fills Received: {}", fill_count);
    println!("Gate Blocks: {}", gate_metrics.gate_block_count);
    println!("Cooldown Blocks: {}", gate_metrics.cooldown_block_count);
    println!();

    // Print metrics summary
    metrics.print_summary();

    // Soft gating check
    let p95_us = metrics.percentile(0.95);
    let p95_ms = p95_us as f64 / 1000.0;
    println!("\n=== Soft Gating Check ===");
    if p95_us <= 15000 {
        println!("✓ PASS: p95 latency ({:.2} ms) <= 15.00 ms target", p95_ms);
    } else {
        println!("⚠ WARN: p95 latency ({:.2} ms) > 15.00 ms target", p95_ms);
        println!("(benchmark exits 0 for non-failing gate)");
    }

    // Write histogram to file
    match metrics.write_to_file(&args.hist_out) {
        Ok(_) => println!("\nHistogram written to: {}", args.hist_out.display()),
        Err(e) => eprintln!("Failed to write histogram: {}", e),
    }

    // Create execution metrics struct
    let exec_metrics = ExecutionMetrics {
        emitted_intents: gate_metrics.emitted_intents,
        dropped_intents: gate_metrics.dropped_intents,
        ack_count,
        fill_count,
        gate_block_count: gate_metrics.gate_block_count,
        cooldown_block_count: gate_metrics.cooldown_block_count,
    };

    // Write JSON summary with execution metrics
    let json_path = args.hist_out.with_file_name("histogram_summary.json");
    match metrics.write_summary_json(&json_path, duration_secs, &exec_metrics) {
        Ok(_) => println!("JSON summary written to: {}", json_path.display()),
        Err(e) => eprintln!("Failed to write JSON summary: {}", e),
    }

    // Write text summary
    let txt_path = args.hist_out.with_file_name("summary.txt");
    match metrics.write_text_summary(
        &txt_path,
        duration_secs,
        num_ticks,
        trigger_count,
        &exec_metrics,
    ) {
        Ok(_) => println!("Text summary written to: {}", txt_path.display()),
        Err(e) => eprintln!("Failed to write text summary: {}", e),
    }

    println!("\n✓ Shadow benchmark completed successfully");
    
    // Wait for threads to complete and handle panics
    if let Err(e) = event_handle.join() {
        eprintln!("Event consumer thread panicked: {:?}", e);
    }
    if let Err(e) = exec_handle.join() {
        eprintln!("Execution mock thread panicked: {:?}", e);
    }

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

    #[test]
    fn test_data_feed_hotpath_integration() {
        use data_feed::DataFeed;
        use hotpath::HotPath;
        use std::sync::Arc;

        // Create data feed with SBE decoder
        let mut feed = DataFeed::new(true, 1000);
        let rx = feed.get_receiver().unwrap();

        // Create hot path
        let hotpath = Arc::new(HotPath::new(100, 5.0, 60));

        // Decode some ticks
        let decoded = feed.decode_and_send(100);
        assert!(decoded > 0);

        // Process ticks from channel through hot path
        let mut processed = 0;
        while let Ok(tick) = rx.try_recv() {
            // Update snapshot (off hot-path in real system)
            hotpath.update_snapshot(tick.symbol_id, tick.px_e8, tick.ts_unix_ms);

            // Process on hot path
            let _trigger = hotpath.process_tick(&tick);
            processed += 1;
        }

        assert_eq!(processed, decoded);
    }
}
