use std::sync::Arc;
use std::time::Instant;
use arc_swap::ArcSwap;
use crate::data_feed::TradeTick;

/// Price snapshot for a symbol (60-second window)
#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    /// Ring buffer of prices (fixed size, pre-allocated)
    prices: Vec<PricePoint>,
    /// Current write index in the ring
    write_idx: usize,
    /// Number of valid entries
    count: usize,
    /// Window duration in milliseconds
    window_ms: u64,
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
            prices: vec![PricePoint { px_e8: 0, ts_unix_ms: 0 }; capacity],
            write_idx: 0,
            count: 0,
            window_ms: window_secs * 1000,
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

    /// Compute 60-second return (hot-path read-only)
    pub fn compute_return_60s(&self, current_ts_ms: u64) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        // Find oldest valid price within window
        let cutoff_ts = current_ts_ms.saturating_sub(self.window_ms);
        let mut oldest_price: Option<u64> = None;
        let mut newest_price: Option<u64> = None;

        // Scan ring buffer for valid prices
        for i in 0..self.count {
            let point = &self.prices[i];
            if point.ts_unix_ms >= cutoff_ts {
                if oldest_price.is_none() || point.ts_unix_ms < newest_price.unwrap_or(u64::MAX) {
                    if oldest_price.is_none() {
                        oldest_price = Some(point.px_e8);
                    }
                }
                if newest_price.is_none() || point.ts_unix_ms > cutoff_ts {
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

/// Hot-path processor for tick-to-trigger logic
pub struct HotPath {
    /// Global flag to enable/disable buying
    can_buy: bool,
    /// Return threshold for triggering
    threshold_pct: f64,
    /// Price snapshots per symbol (Arc-swapped for lock-free reads)
    snapshots: Vec<ArcSwap<PriceSnapshot>>,
    /// Maximum symbols
    max_symbols: usize,
}

impl HotPath {
    /// Create a new hot-path processor
    pub fn new(max_symbols: usize, threshold_pct: f64, window_secs: u64) -> Self {
        let snapshots: Vec<ArcSwap<PriceSnapshot>> = (0..max_symbols)
            .map(|_| ArcSwap::new(Arc::new(PriceSnapshot::new(window_secs))))
            .collect();

        Self {
            can_buy: true,
            threshold_pct,
            snapshots,
            max_symbols,
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

    /// Process a tick on the hot-path (zero allocations)
    pub fn process_tick(&self, tick: &TradeTick) -> Option<TriggerEvent> {
        if !self.can_buy || (tick.symbol_id as usize) >= self.max_symbols {
            return None;
        }

        // Load snapshot (lock-free read via arc-swap)
        let snapshot = self.snapshots[tick.symbol_id as usize].load();

        // Compute 60s return
        if let Some(ret_60s) = snapshot.compute_return_60s(tick.ts_unix_ms) {
            // Check trigger condition
            if ret_60s >= self.threshold_pct {
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

    /// Set global can_buy flag
    #[allow(dead_code)]
    pub fn set_can_buy(&mut self, can_buy: bool) {
        self.can_buy = can_buy;
    }
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
