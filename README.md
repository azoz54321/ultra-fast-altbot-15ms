# Ultra-Fast Altcoin Bot (≤15ms tick→BUY+LIMIT p95)

هدف المشروع: إرسال BUY ثم LIMIT (TP +10%) خلال ≤15ms (p95) من لحظة وصول صفقة (trade tick) على جانب التطبيق، مع شاردات، وبدون تخصيصات على المسار الحار. يبدأ في وضع SHADOW (اختبار بدون أوامر حقيقية).

## المراحل
- **Phase 1: ✅ COMPLETED** - Hot Path skeleton with zero-allocation design
  - Modules: data_feed, sbe_decoder_ffi (placeholder), hotpath, metrics
  - TradeTick structure and synthetic tick generation
  - 60-second price ring buffer with Arc/arc-swap for lock-free reads
  - Trigger logic: ret_60s ≥ 5.0% with can_buy flag
  - HDR histogram latency tracking (p50/p95/p99)
  - Shadow benchmark harness (100K ticks, 300 symbols)
  - CI pipelines for build/test and latency benchmarking
- **Phase 2: ✅ COMPLETED** - Enhanced hot path, SBE decoder FFI stub, and metrics improvements
  - AtomicBool can_buy for lock-free risk gate control
  - Pre-allocated 60s ring buffer per symbol with immutable snapshot reads
  - Off-hot-path maintenance tasks for 15m and 1h aggregate computation
  - C SBE decoder FFI stub with #[repr(C)] RawTick struct
  - SPSC channel integration for tick processing pipeline
  - JSON histogram summary with detailed statistics
  - Enhanced CLI flags: --symbols-per-shard, --hist-out
- Phase 3: Alt Market Gate (AHI).
- Phase 4: Risk (Slots A/B، freeze، bans).
- Phase 5: Ops Hardening (reconnect، token-bucket، singleflight، filter refresh).
- Phase 6: Benchmark CI (p50/p95/p99 histograms، إسقاط التريجر فوق الميزانية).
- Phase 7: LIVE enable (يتطلب Secrets وبيئة محمية).

## Quickstart

### Build
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### Run Tests
```bash
cargo test
```

### Run Shadow Benchmark
```bash
# Default: 100,000 ticks across 300 symbols
cargo run --release -- --bench-shadow

# Custom parameters
cargo run --release -- --bench-shadow --num-ticks 50000 --num-symbols 100

# With sharding and custom output path
cargo run --release -- --bench-shadow \
  --num-ticks 100000 \
  --num-symbols 300 \
  --symbols-per-shard 50 \
  --hist-out target/custom_bench.hdr
```

The benchmark will:
- Generate synthetic tick data with realistic price movements
- Process ticks through the hot-path with latency measurement
- Record triggers when ret_60s ≥ 5.0%
- Output p50/p95/p99/p99.9 latency statistics
- Write HDR histogram to specified path (default: `target/shadow_bench/hdr_histogram.hdr`)
- Write JSON summary to `histogram_summary.json` with detailed metrics

### Example Output
```
=== Benchmark Complete ===
Total time: 0.30s
Throughput: 338681 ticks/sec
Triggers: 11516

=== Latency Summary ===
Total samples: 100000
p50: 2 µs (0.00 ms)
p95: 3 µs (0.00 ms)
p99: 4 µs (0.00 ms)
p99.9: 13 µs (0.01 ms)
max: 49 µs (0.05 ms)

Histogram written to: target/shadow_bench/hdr_histogram.hdr
JSON summary written to: target/shadow_bench/histogram_summary.json
```

The JSON summary includes:
```json
{
  "count": 100000,
  "min": 2,
  "max": 34,
  "p50": 2,
  "p95": 3,
  "p99": 4,
  "p99_9": 14,
  "throughput_avg": 322222.31
}
```

## Architecture

### Zero-Allocation Hot Path
- Fixed-size ring buffers pre-allocated per symbol
- Arc-swap for lock-free snapshot reads
- No heap allocations in the critical path
- Single-threaded processing for predictable latency

### Data Flow
```
TradeTick → HotPath.process_tick() → Trigger Decision
     ↓
  Update Snapshot (off hot-path)
     ↓
  60s Price Ring Buffer
```

### Modules
- **config**: Configuration management (target latency, thresholds)
- **data_feed**: TradeTick structure, synthetic tick generation, and SPSC channel integration
- **sbe_decoder_ffi**: C FFI bindings for SBE decoder with #[repr(C)] RawTick struct
- **hotpath**: Core trigger logic with zero-allocation design and AtomicBool can_buy flag
- **metrics**: HDR histogram latency tracking with JSON summary output

### Phase 2 Features
- **Enhanced Hot Path**:
  - AtomicBool can_buy for lock-free risk gate control
  - Pre-allocated 60s ring buffer with immutable snapshot reads
  - Off-hot-path maintenance tasks for 15m/1h aggregates (prepping for AHI)
  - Single-threaded processing with zero allocations
- **SBE Decoder FFI**:
  - C stub implementation with proper FFI bindings
  - Safe Rust wrapper with decode_into() API
  - SPSC channel for tick distribution
  - Integration tests for end-to-end pipeline
- **Enhanced Metrics**:
  - JSON histogram summary with count, min, max, percentiles, throughput
  - Configurable histogram output path via --hist-out
  - Sharding support via --symbols-per-shard flag

## CI

### Build & Test
Runs on every push and PR:
- Cargo build (debug and release)
- Cargo test
- Clippy linting
- Format checking

### Latency Benchmark
Manual workflow dispatch to run shadow benchmark:
- Configurable tick count and symbol count
- Uploads histogram and logs as artifacts
- Displays summary in GitHub Actions

## Configuration

Default configuration in `src/config/mod.rs`:
- Target p95 latency: 15ms
- Shadow mode: enabled
- Return threshold: 5.0%
- Max symbols: 300
- Price window: 60 seconds

## Performance Notes

Current implementation achieves:
- **~340K ticks/sec** throughput
- **p95 < 5µs** tick-to-trigger latency
- **p99 < 5µs** on typical hardware
- Zero allocations in hot path

This exceeds the 15ms target by orders of magnitude, providing headroom for future phases (SBE decoding, network I/O, order execution).

## ملاحظات
- Phase 1 + Phase 2 توفر البنية الأساسية الكاملة للمسار الحار مع دعم FFI
- يستخدم بيانات صناعية حالياً؛ SBE decoder جاهز للاتصال ببيانات حقيقية
- لا اتصال شبكي أو تنفيذ أوامر حقيقية (وضع SHADOW)
- تصميم بدون تخصيصات ديناميكية على المسار الحار لضمان أقل زمن استجابة
- دعم كامل لـ SPSC channel للتواصل بين data feed و hotpath
