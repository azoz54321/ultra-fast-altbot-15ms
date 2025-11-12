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
- **Phase 3: ✅ COMPLETED** - End-to-end tick-to-order flow with execution mock
  - Execution mock module simulating exchange responses (Ack/Fill) without real I/O
  - SPSC channel from hotpath to execution mock (bounded queue, non-blocking emit)
  - Trigger wiring: emit OrderIntent on ret_60s ≥ threshold with can_buy gate
  - Enhanced risk/gate logic: max_open_intents, per-symbol cooldown (500ms), budget counter
  - Improved tick generator: variable rates (Poisson 50-150 tps), micro-bursts, mean-reverting drift
  - Extended metrics: emitted_intents, dropped_intents, ack_count, fill_count, gate/cooldown blocks
  - Soft gating: p95 ≤ 15ms check (WARN if above, exits 0)
  - Human-readable summary.txt artifact with consistency checks
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
- Generate synthetic tick data with enhanced realism (variable rates, micro-bursts, mean-reverting drift)
- Process ticks through the hot-path with latency measurement
- Emit OrderIntents when ret_60s ≥ 5.0% and gates pass (can_buy, budget, max_open_intents, cooldown)
- Simulate exchange responses via execution mock (Submitted → Ack → Fill)
- Record triggers and execution metrics
- Output p50/p95/p99/p99.9 latency statistics
- Write HDR histogram to specified path (default: `target/shadow_bench/hdr_histogram.hdr`)
- Write JSON summary to `histogram_summary.json` with detailed execution metrics
- Write human-readable text summary to `summary.txt`

### Example Output (Phase 3)
```
=== Benchmark Complete ===
Total time: 0.30s
Throughput: 338681 ticks/sec
Triggers: 11516

=== Execution Mock Stats ===
Emitted Intents: 1000
Dropped Intents: 0
Acks Received: 1000
Fills Received: 1000
Gate Blocks: 2081
Cooldown Blocks: 819

=== Latency Summary ===
Total samples: 100000
p50: 2 µs (0.00 ms)
p95: 3 µs (0.00 ms)
p99: 4 µs (0.00 ms)
p99.9: 13 µs (0.01 ms)
max: 49 µs (0.05 ms)

=== Soft Gating Check ===
✓ PASS: p95 latency (0.00 ms) <= 15.00 ms target

Histogram written to: target/shadow_bench/hdr_histogram.hdr
JSON summary written to: target/shadow_bench/histogram_summary.json
Text summary written to: target/shadow_bench/summary.txt
```

The JSON summary includes execution metrics:
```json
{
  "count": 100000,
  "min": 2,
  "max": 34,
  "p50": 2,
  "p95": 3,
  "p99": 4,
  "p99_9": 14,
  "throughput_avg": 322222.31,
  "emitted_intents": 1000,
  "dropped_intents": 0,
  "ack_count": 1000,
  "fill_count": 1000,
  "gate_block_count": 2081,
  "cooldown_block_count": 819
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
Phase 1-2 Flow:
TradeTick → HotPath.process_tick() → Trigger Decision
     ↓
  Update Snapshot (off hot-path)
     ↓
  60s Price Ring Buffer

Phase 3 Flow (End-to-End Tick-to-Order):
DataFeed → SBE FFI → Channel → HotPath → (OrderIntent) → ExecMock → Metrics
                                    ↓
                              Trigger Decision
                                    ↓
                        [Gates: can_buy, budget, max_open_intents, cooldown]
                                    ↓
                            Emit OrderIntent (non-blocking)
                                    ↓
                         SPSC Queue (bounded, drop-on-full)
                                    ↓
                    ExecutionMock (off hot-path thread)
                                    ↓
                    [Submitted → Ack → Fill with deterministic delays]
                                    ↓
                         OrderEvent back to metrics
```

### Modules
- **config**: Configuration management (target latency, thresholds)
- **data_feed**: TradeTick structure, enhanced synthetic tick generation with variable rates and micro-bursts
- **sbe_decoder_ffi**: C FFI bindings for SBE decoder with #[repr(C)] RawTick struct
- **hotpath**: Core trigger logic with zero-allocation design, risk gates, cooldowns, and order intent emission
- **execution**: Execution mock module simulating exchange responses without real I/O
- **metrics**: HDR histogram latency tracking with JSON and text summary output

### Phase 3 Features
- **Execution Mock**:
  - OrderIntent structure (symbol_id, side, px_e8, ts_unix_ms)
  - OrderEvent structure (kind: Submitted/Ack/Fill, symbol_id, px_e8, ts_unix_ms)
  - SPSC channel from hotpath to execution mock (bounded queue, capacity 1000)
  - Deterministic delays (50µs for Ack, 100µs for Fill) without syscalls
  - Runs off hot-path in separate thread
- **Trigger Wiring**:
  - HotPath emits OrderIntent when ret_60s ≥ threshold and gates pass
  - Non-blocking emit with drop-on-full behavior (tracked in dropped_intents)
  - Per-symbol cooldown (500ms) to avoid repeated triggers
- **Enhanced Risk/Gate Logic**:
  - Global can_buy flag (existing, AtomicBool)
  - max_open_intents gate (default: 10)
  - Per-symbol cooldown store (pre-allocated array of u64 timestamps)
  - Budget counter (AtomicU64) with replenishment capability
  - Metrics: gate_block_count, cooldown_block_count
- **Improved Tick Generator**:
  - Variable per-symbol tick rates (Poisson-distributed 50-150 tps)
  - Micro-burst mode (5x rate for short periods, triggered randomly)
  - Mild mean-reverting drift to force trigger conditions
  - Maintains backward compatibility with existing flags
- **Extended Metrics & Gating**:
  - JSON summary includes: emitted_intents, dropped_intents, ack_count, fill_count, gate/cooldown blocks
  - Soft gating check: p95 ≤ 15000µs (15ms) - prints WARN if above but exits 0
  - Human-readable summary.txt with consistency check (fills ≤ acks ≤ emitted_intents)
  - CI uploads all artifacts: hdr_histogram.hdr, histogram_summary.json, summary.txt, bench logs

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
