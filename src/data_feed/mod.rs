/// Trade tick data structure with zero-allocation design
#[derive(Debug, Clone, Copy)]
pub struct TradeTick {
    /// Symbol ID (integer representation)
    pub symbol_id: u32,
    /// Price in fixed-point (scaled by 1e8)
    pub px_e8: u64,
    /// Unix timestamp in milliseconds
    pub ts_unix_ms: u64,
}

impl TradeTick {
    /// Create a new trade tick
    pub fn new(symbol_id: u32, px_e8: u64, ts_unix_ms: u64) -> Self {
        Self {
            symbol_id,
            px_e8,
            ts_unix_ms,
        }
    }

    /// Get price as f64
    #[allow(dead_code)]
    pub fn price(&self) -> f64 {
        self.px_e8 as f64 / 1e8
    }
}

/// Synthetic tick generator for benchmarking
pub struct TickGenerator {
    num_symbols: u32,
    num_ticks: usize,
    base_ts: u64,
    base_prices: Vec<u64>,
}

impl TickGenerator {
    /// Create a new tick generator
    pub fn new(num_symbols: u32, num_ticks: usize) -> Self {
        let base_ts = 1700000000000; // Nov 2023 timestamp
        let base_prices: Vec<u64> = (0..num_symbols)
            .map(|i| {
                // Generate realistic base prices (e.g., 10-1000 USDT)
                let base = 10u64 + (i as u64 * 13) % 990;
                base * 100_000_000 // Convert to e8 format
            })
            .collect();

        Self {
            num_symbols,
            num_ticks,
            base_ts,
            base_prices,
        }
    }

    /// Generate synthetic ticks with realistic price movements
    pub fn generate(&self) -> Vec<TradeTick> {
        let mut ticks = Vec::with_capacity(self.num_ticks);
        let mut rng_state = 12345u64; // Simple LCG for reproducibility

        for i in 0..self.num_ticks {
            // Simple LCG random number generator
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let symbol_id = (rng_state % self.num_symbols as u64) as u32;

            // Generate price variation (-2% to +8% from base, biased upward)
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let price_var_pct = ((rng_state % 1000) as i64 - 200) as f64 / 100.0;
            
            let base_price = self.base_prices[symbol_id as usize];
            let varied_price = (base_price as f64 * (1.0 + price_var_pct / 100.0)) as u64;

            // Timestamp increases linearly (1ms per tick on average)
            let ts_unix_ms = self.base_ts + i as u64;

            ticks.push(TradeTick::new(symbol_id, varied_price, ts_unix_ms));
        }

        ticks
    }
}
