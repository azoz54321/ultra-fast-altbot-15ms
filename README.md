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
- Phase 2: Integer sizing + tick/step rounding + rings 60s/15m/1h خارج المسار الحار.
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
```

The benchmark will:
- Generate synthetic tick data with realistic price movements
- Process ticks through the hot-path with latency measurement
- Record triggers when ret_60s ≥ 5.0%
- Output p50/p95/p99/p99.9 latency statistics
- Write HDR histogram to `target/shadow_bench/hdr_histogram.hdr`

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
- **data_feed**: TradeTick structure and synthetic tick generation
- **sbe_decoder_ffi**: Placeholder for SBE decoder (future)
- **hotpath**: Core trigger logic with zero-allocation design
- **metrics**: HDR histogram latency tracking

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
- Phase 1 يوفر البنية الأساسية الكاملة للمسار الحار
- يستخدم بيانات صناعية حالياً؛ سيتم إضافة SBE decoder في المراحل القادمة
- لا اتصال شبكي أو تنفيذ أوامر حقيقية (وضع SHADOW)
- تصميم بدون تخصيصات ديناميكية على المسار الحار لضمان أقل زمن استجابة
