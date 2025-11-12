/// Configuration for the ultra-fast altbot
#[derive(Debug, Clone)]
pub struct Config {
    /// Target p95 latency in milliseconds
    pub target_p95_ms: u64,
    /// Shadow mode enabled (no real orders)
    pub shadow_mode: bool,
    /// Return threshold for triggering (e.g., 5.0 = 5%)
    pub return_threshold_pct: f64,
    /// Maximum number of symbols to track
    pub max_symbols: usize,
    /// Price ring buffer duration in seconds
    pub price_window_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_p95_ms: 15,
            shadow_mode: true,
            return_threshold_pct: 5.0,
            max_symbols: 300,
            price_window_secs: 60,
        }
    }
}
