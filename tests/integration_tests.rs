use ultra_fast_altbot::*;
use std::sync::atomic::Ordering;

#[test]
fn test_phase2_hotpath_atomic_can_buy() {
    use std::sync::Arc;
    
    let hotpath = Arc::new(hotpath::HotPath::new(100, 5.0, 60));
    
    // Test atomic can_buy flag
    assert!(hotpath.can_buy_flag().load(Ordering::Relaxed));
    
    hotpath.set_can_buy(false);
    assert!(!hotpath.can_buy_flag().load(Ordering::Relaxed));
    
    hotpath.set_can_buy(true);
    assert!(hotpath.can_buy_flag().load(Ordering::Relaxed));
}

#[test]
fn test_phase2_sbe_decoder_ffi() {
    use sbe_decoder_ffi::SbeDecoderFfi;
    use data_feed::TradeTick;

    let mut decoder = SbeDecoderFfi::new().expect("Failed to create decoder");
    let mut tick = TradeTick::new(0, 0, 0);
    
    // Decode a few ticks from the stub
    assert!(decoder.decode_into(&mut tick));
    assert!(tick.px_e8 > 0);
    assert!(tick.ts_unix_ms > 0);
}

#[test]
fn test_phase2_metrics_json_output() {
    use metrics::MetricsCollector;
    use std::path::PathBuf;
    use std::fs;

    let mut collector = MetricsCollector::new(100_000, 3).unwrap();
    
    // Record some latencies
    for i in 1..=100 {
        collector.record(i).unwrap();
    }
    
    collector.set_duration(1.0);
    
    let summary = collector.summary();
    assert_eq!(summary.count, 100);
    assert!(summary.throughput_avg > 0.0);
    
    // Test JSON output
    let json_path = PathBuf::from("/tmp/test_metrics_summary.json");
    collector.write_json_summary(&json_path).expect("Failed to write JSON");
    
    let json_content = fs::read_to_string(&json_path).expect("Failed to read JSON");
    assert!(json_content.contains("\"count\""));
    assert!(json_content.contains("100"));
    assert!(json_content.contains("\"throughput_avg\""));
    
    // Cleanup
    fs::remove_file(&json_path).ok();
}

#[test]
fn test_phase2_aggregate_rings() {
    use std::sync::Arc;
    
    let hotpath = Arc::new(hotpath::HotPath::new(100, 5.0, 60));
    
    // Add some ticks to a symbol
    let symbol_id = 10;
    for i in 0..100 {
        hotpath.update_snapshot(symbol_id, 100_000_000 + i * 1000, 1700000000000 + i);
    }
    
    // Maintain aggregate rings (off hot-path operation)
    hotpath.maintain_aggregate_rings(symbol_id);
    
    // This tests that maintain_aggregate_rings doesn't panic and completes successfully
}
