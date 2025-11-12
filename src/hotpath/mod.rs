use crate::data_feed::TradeTick;
use crate::execution::{OrderIntent, OrderSide};
use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Price snapshot for a symbol (60-second window)
#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    /// Ring buffer of prices (fixed size, pre-allocated)
    prices: Vec<PricePoint>,
    /// Current write index in the ring
    write_idx: usize,
    /// Number of valid entries
    count: usize,
    /// Window duration in milliseconds (60s)
    window_ms: u64,
    /// 15-minute aggregate return (computed off hot-path)
    ret_15m: Option<f64>,
    /// 1-hour aggregate return (computed off hot-path)
    ret_1h: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct PricePoint {
    px_e8: u64,
    ts_unix_ms: u64,
}

impl PriceSnapshot {
    /// Create a new price snapshot with fixed capacity
    pub fn new(window_secs: u64) -> Self {
        let capacity = (window_secs * 100) as usize; // Assume max 100 ticks/sec
        Self {
            prices: vec![
                PricePoint {
                    px_e8: 0,
                    ts_unix_ms: 0
                };
                capacity
            ],
            write_idx: 0,
            count: 0,
            window_ms: window_secs * 1000,
            ret_15m: None,
            ret_1h: None,
        }
    }

    /// Add a new price point (called off hot-path)
    pub fn add(&mut self, px_e8: u64, ts_unix_ms: u64) {
        self.prices[self.write_idx] = PricePoint { px_e8, ts_unix_ms };
        self.write_idx = (self.write_idx + 1) % self.prices.len();
        if self.count < self.prices.len() {
            self.count += 1;
        }
    }

    /// Compute 60-second return (hot-path read-only, zero allocations)
    pub fn compute_return_60s(&self, current_ts_ms: u64) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        // Find oldest and newest valid prices within 60s window
        let cutoff_ts = current_ts_ms.saturating_sub(self.window_ms);
        let mut oldest_price: Option<u64> = None;
        let mut oldest_ts = u64::MAX;
        let mut newest_price: Option<u64> = None;
        let mut newest_ts = 0u64;

        // Scan ring buffer for valid prices (no allocations)
        for i in 0..self.count {
            let point = &self.prices[i];
            if point.ts_unix_ms >= cutoff_ts {
                // Track oldest price in window
                if point.ts_unix_ms < oldest_ts {
                    oldest_ts = point.ts_unix_ms;
                    oldest_price = Some(point.px_e8);
                }
                // Track newest price in window
                if point.ts_unix_ms > newest_ts {
                    newest_ts = point.ts_unix_ms;
                    newest_price = Some(point.px_e8);
                }
            }
        }

        if let (Some(old_px), Some(new_px)) = (oldest_price, newest_price) {
            if old_px > 0 {
                let ret = ((new_px as f64 - old_px as f64) / old_px as f64) * 100.0;
                return Some(ret);
            }
        }

        None
    }

    /// Compute return over a longer window (off hot-path)
    fn compute_return_window(&self, window_secs: u64, current_ts_ms: u64) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        let cutoff_ts = current_ts_ms.saturating_sub(window_secs * 1000);
        let mut oldest_price: Option<u64> = None;
        let mut oldest_ts = u64::MAX;
        let mut newest_price: Option<u64> = None;
        let mut newest_ts = 0u64;

        for i in 0..self.count {
            let point = &self.prices[i];
            if point.ts_unix_ms >= cutoff_ts {
                if point.ts_unix_ms < oldest_ts {
                    oldest_ts = point.ts_unix_ms;
                    oldest_price = Some(point.px_e8);
                }
                if point.ts_unix_ms > newest_ts {
                    newest_ts = point.ts_unix_ms;
                    newest_price = Some(point.px_e8);
                }
            }
        }

        if let (Some(old_px), Some(new_px)) = (oldest_price, newest_price) {
            if old_px > 0 {
                let ret = ((new_px as f64 - old_px as f64) / old_px as f64) * 100.0;
                return Some(ret);
            }
        }

        None
    }

    /// Update aggregate returns (15m and 1h) - called off hot-path
    pub fn update_aggregates(&mut self, current_ts_ms: u64) {
        self.ret_15m = self.compute_return_window(15 * 60, current_ts_ms);
        self.ret_1h = self.compute_return_window(60 * 60, current_ts_ms);
    }

    /// Get 15-minute return (precomputed, hot-path safe)
    #[allow(dead_code)]
    pub fn get_return_15m(&self) -> Option<f64> {
        self.ret_15m
    }

    /// Get 1-hour return (precomputed, hot-path safe)
    #[allow(dead_code)]
    pub fn get_return_1h(&self) -> Option<f64> {
        self.ret_1h
    }
}

/// Trigger event recorded when conditions are met
#[derive(Debug, Clone, Copy)]
pub struct TriggerEvent {
    pub symbol_id: u32,
    #[allow(dead_code)]
    pub ts_unix_ms: u64,
    pub return_pct: f64,
    pub price_e8: u64,
}

/// Hot-path processor for tick-to-trigger logic with execution wiring
pub struct HotPath {
    /// Global flag to enable/disable buying (atomic for lock-free access)
    can_buy: Arc<AtomicBool>,
    /// Return threshold for triggering
    threshold_pct: f64,
    /// Price snapshots per symbol (Arc-swapped for lock-free reads)
    snapshots: Vec<ArcSwap<PriceSnapshot>>,
    /// Maximum symbols
    max_symbols: usize,
    /// Maximum open intents (gate)
    max_open_intents: Arc<AtomicU32>,
    /// Current open intents counter
    open_intents: Arc<AtomicU32>,
    /// Budget counter (decrements on emit, replenished by maintenance)
    budget: Arc<AtomicU64>,
    /// Per-symbol cooldown timestamps (last trigger time in ms)
    cooldowns: Vec<AtomicU64>,
    /// Cooldown duration in milliseconds
    cooldown_ms: u64,
    /// Optional sender for order intents
    intent_tx: Option<Sender<OrderIntent>>,
    /// Dropped intents counter
    dropped_intents: Arc<AtomicU64>,
    /// Emitted intents counter
    emitted_intents: Arc<AtomicU64>,
    /// Gate block counter
    gate_block_count: Arc<AtomicU64>,
    /// Cooldown block counter
    cooldown_block_count: Arc<AtomicU64>,
}

impl HotPath {
    /// Create a new hot-path processor
    pub fn new(max_symbols: usize, threshold_pct: f64, window_secs: u64) -> Self {
        Self::with_config(max_symbols, threshold_pct, window_secs, 10, 500, 1000)
    }

    /// Create with custom configuration
    pub fn with_config(
        max_symbols: usize,
        threshold_pct: f64,
        window_secs: u64,
        max_open_intents: u32,
        cooldown_ms: u64,
        initial_budget: u64,
    ) -> Self {
        let snapshots: Vec<ArcSwap<PriceSnapshot>> = (0..max_symbols)
            .map(|_| ArcSwap::new(Arc::new(PriceSnapshot::new(window_secs))))
            .collect();

        let cooldowns: Vec<AtomicU64> = (0..max_symbols)
            .map(|_| AtomicU64::new(0))
            .collect();

        Self {
            can_buy: Arc::new(AtomicBool::new(true)),
            threshold_pct,
            snapshots,
            max_symbols,
            max_open_intents: Arc::new(AtomicU32::new(max_open_intents)),
            open_intents: Arc::new(AtomicU32::new(0)),
            budget: Arc::new(AtomicU64::new(initial_budget)),
            cooldowns,
            cooldown_ms,
            intent_tx: None,
            dropped_intents: Arc::new(AtomicU64::new(0)),
            emitted_intents: Arc::new(AtomicU64::new(0)),
            gate_block_count: Arc::new(AtomicU64::new(0)),
            cooldown_block_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update snapshot for a symbol (off hot-path)
    pub fn update_snapshot(&self, symbol_id: u32, px_e8: u64, ts_unix_ms: u64) {
        if (symbol_id as usize) < self.max_symbols {
            let current = self.snapshots[symbol_id as usize].load();
            let mut new_snapshot = (**current).clone();
            new_snapshot.add(px_e8, ts_unix_ms);
            self.snapshots[symbol_id as usize].store(Arc::new(new_snapshot));
        }
    }

    /// Process a tick on the hot-path (zero allocations, single-threaded)
    /// Returns TriggerEvent if conditions met, and optionally emits OrderIntent
    pub fn process_tick(&self, tick: &TradeTick) -> Option<TriggerEvent> {
        // Check can_buy flag (atomic load, relaxed ordering for performance)
        if !self.can_buy.load(Ordering::Relaxed) || (tick.symbol_id as usize) >= self.max_symbols {
            return None;
        }

        // Load snapshot (lock-free read via arc-swap, immutable snapshot)
        let snapshot = self.snapshots[tick.symbol_id as usize].load();

        // Compute 60s return (no allocations, read-only operation)
        if let Some(ret_60s) = snapshot.compute_return_60s(tick.ts_unix_ms) {
            // Check trigger condition
            if ret_60s >= self.threshold_pct {
                // Try to emit order intent (with gates and cooldowns)
                self.try_emit_intent(tick);

                return Some(TriggerEvent {
                    symbol_id: tick.symbol_id,
                    ts_unix_ms: tick.ts_unix_ms,
                    return_pct: ret_60s,
                    price_e8: tick.px_e8,
                });
            }
        }

        None
    }

    /// Try to emit order intent (with gates, cooldowns, budget checks)
    fn try_emit_intent(&self, tick: &TradeTick) {
        // If no intent sender, skip
        let intent_tx = match &self.intent_tx {
            Some(tx) => tx,
            None => return,
        };

        let symbol_idx = tick.symbol_id as usize;
        if symbol_idx >= self.max_symbols {
            return;
        }

        // Check cooldown for this symbol
        let last_trigger = self.cooldowns[symbol_idx].load(Ordering::Relaxed);
        if tick.ts_unix_ms < last_trigger + self.cooldown_ms {
            self.cooldown_block_count.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // Check budget
        let budget = self.budget.load(Ordering::Relaxed);
        if budget == 0 {
            self.gate_block_count.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // Check max open intents
        let open = self.open_intents.load(Ordering::Relaxed);
        let max_open = self.max_open_intents.load(Ordering::Relaxed);
        if open >= max_open {
            self.gate_block_count.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // Create order intent
        let intent = OrderIntent::new(
            tick.symbol_id,
            OrderSide::Buy,
            tick.px_e8,
            tick.ts_unix_ms,
        );

        // Try to send (non-blocking)
        match intent_tx.try_send(intent) {
            Ok(_) => {
                // Success: update counters and cooldown
                self.emitted_intents.fetch_add(1, Ordering::Relaxed);
                self.open_intents.fetch_add(1, Ordering::Relaxed);
                self.budget.fetch_sub(1, Ordering::Relaxed);
                self.cooldowns[symbol_idx].store(tick.ts_unix_ms, Ordering::Relaxed);
            }
            Err(_) => {
                // Queue full: drop and record
                self.dropped_intents.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Set global can_buy flag (atomic store, can be called from risk/gate task)
    pub fn set_can_buy(&self, can_buy: bool) {
        self.can_buy.store(can_buy, Ordering::Relaxed);
    }

    /// Get can_buy status
    #[allow(dead_code)]
    pub fn get_can_buy(&self) -> bool {
        self.can_buy.load(Ordering::Relaxed)
    }

    /// Update aggregates for a symbol (off hot-path maintenance task)
    pub fn update_aggregates(&self, symbol_id: u32, current_ts_ms: u64) {
        if (symbol_id as usize) < self.max_symbols {
            let current = self.snapshots[symbol_id as usize].load();
            let mut new_snapshot = (**current).clone();
            new_snapshot.update_aggregates(current_ts_ms);
            self.snapshots[symbol_id as usize].store(Arc::new(new_snapshot));
        }
    }

    /// Set the order intent sender (must be called before processing ticks that emit intents)
    pub fn set_intent_sender(&mut self, sender: Sender<OrderIntent>) {
        self.intent_tx = Some(sender);
    }

    /// Get gate metrics (for reporting)
    pub fn get_gate_metrics(&self) -> GateMetrics {
        GateMetrics {
            emitted_intents: self.emitted_intents.load(Ordering::Relaxed),
            dropped_intents: self.dropped_intents.load(Ordering::Relaxed),
            gate_block_count: self.gate_block_count.load(Ordering::Relaxed),
            cooldown_block_count: self.cooldown_block_count.load(Ordering::Relaxed),
        }
    }

    /// Decrement open intents counter (called when order completes)
    /// Uses saturating subtraction to prevent underflow
    pub fn decrement_open_intents(&self) {
        // Use fetch_max to ensure we don't go below 0
        let prev = self.open_intents.fetch_sub(1, Ordering::Relaxed);
        if prev == 0 {
            // We went negative, add it back
            self.open_intents.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Replenish budget (called by maintenance task)
    pub fn replenish_budget(&self, amount: u64) {
        self.budget.fetch_add(amount, Ordering::Relaxed);
    }

    /// Get current budget
    #[allow(dead_code)]
    pub fn get_budget(&self) -> u64 {
        self.budget.load(Ordering::Relaxed)
    }
}

/// Gate metrics for reporting
#[derive(Debug, Clone, Copy)]
pub struct GateMetrics {
    pub emitted_intents: u64,
    pub dropped_intents: u64,
    pub gate_block_count: u64,
    pub cooldown_block_count: u64,
}

/// Latency measurement for a single tick processing
#[derive(Debug, Clone, Copy)]
pub struct LatencyMeasurement {
    pub start: Instant,
    pub end: Instant,
}

impl LatencyMeasurement {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            end: Instant::now(),
        }
    }

    pub fn start(&mut self) {
        self.start = Instant::now();
    }

    pub fn end(&mut self) {
        self.end = Instant::now();
    }

    pub fn duration_micros(&self) -> u64 {
        self.end.duration_since(self.start).as_micros() as u64
    }
}
