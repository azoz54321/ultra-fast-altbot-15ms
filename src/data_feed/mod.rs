use crate::sbe_decoder_ffi::SbeDecoderFfi;
use crossbeam_channel::{bounded, Receiver, Sender};

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

/// Data feed manager that can use SBE decoder or synthetic data
pub struct DataFeed {
    decoder: Option<SbeDecoderFfi>,
    tx: Option<Sender<TradeTick>>,
    rx: Option<Receiver<TradeTick>>,
}

impl DataFeed {
    /// Create a new data feed with SPSC channel
    pub fn new(use_sbe_decoder: bool, channel_capacity: usize) -> Self {
        let (tx, rx) = bounded(channel_capacity);
        let decoder = if use_sbe_decoder {
            Some(SbeDecoderFfi::new())
        } else {
            None
        };

        Self {
            decoder,
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    /// Get the receiver end of the channel
    pub fn get_receiver(&mut self) -> Option<Receiver<TradeTick>> {
        self.rx.take()
    }

    /// Decode and send ticks through the channel
    /// Returns number of ticks decoded
    pub fn decode_and_send(&mut self, max_ticks: usize) -> usize {
        if let (Some(decoder), Some(tx)) = (&mut self.decoder, &self.tx) {
            let mut count = 0;
            let mut tick = TradeTick::new(0, 0, 0);

            while count < max_ticks {
                if decoder.decode_into(&mut tick) {
                    if tx.send(tick).is_err() {
                        // Channel closed
                        break;
                    }
                    count += 1;
                } else {
                    // No more data
                    break;
                }
            }

            count
        } else {
            0
        }
    }
}

/// Synthetic tick generator for benchmarking

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

    /// Generate synthetic ticks with enhanced realism:
    /// - Variable per-symbol tick rates (Poisson-distributed 50-150 tps)
    /// - Micro-bursts (short periods with 5x rate)
    /// - Mild mean-reverting drift to force triggers
    pub fn generate(&self) -> Vec<TradeTick> {
        let mut ticks = Vec::with_capacity(self.num_ticks);
        let mut rng_state = 12345u64; // Simple LCG for reproducibility
        
        // Track current price per symbol for mean-reverting drift
        let mut current_prices: Vec<u64> = self.base_prices.clone();
        
        // Poisson-distributed tick rates per symbol (50-150 tps base)
        let symbol_tick_rates: Vec<f64> = (0..self.num_symbols)
            .map(|i| {
                let seed = 54321u64.wrapping_add(i as u64 * 9973);
                let rate_offset = ((seed % 100) as f64 - 50.0) / 100.0;
                100.0 + rate_offset * 50.0 // Range: 50-150 tps
            })
            .collect();
        
        let mut current_ts = self.base_ts;
        let mut burst_mode_until = 0u64;
        
        for i in 0..self.num_ticks {
            // Simple LCG random number generator
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let symbol_id = (rng_state % self.num_symbols as u64) as u32;
            let symbol_idx = symbol_id as usize;
            
            // Determine if we should enter burst mode (5% chance every 1000 ticks)
            if i % 1000 == 0 {
                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                if (rng_state % 100) < 5 {
                    // Enter burst mode for 50 ticks
                    burst_mode_until = current_ts + 50;
                }
            }
            
            // Variable inter-tick delay based on symbol rate and burst mode
            let base_rate = symbol_tick_rates[symbol_idx];
            let effective_rate = if current_ts < burst_mode_until {
                base_rate * 5.0 // 5x rate during burst
            } else {
                base_rate
            };
            
            // Average delay in ms between ticks (1000ms / tps)
            let avg_delay_ms = 1000.0 / effective_rate;
            
            // Add some jitter (±50%)
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let jitter_factor = 0.5 + ((rng_state % 100) as f64 / 100.0);
            let delay_ms = (avg_delay_ms * jitter_factor).max(1.0) as u64;
            
            current_ts += delay_ms;
            
            // Mild mean-reverting drift for price
            let base_price = self.base_prices[symbol_idx];
            let current_price = current_prices[symbol_idx];
            
            // Calculate drift towards base price (mean reversion)
            let deviation_pct = ((current_price as f64 - base_price as f64) / base_price as f64) * 100.0;
            
            // Generate price variation with mean reversion
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let raw_var = ((rng_state % 2000) as i64 - 1000) as f64 / 100.0; // -10% to +10%
            
            // Apply mean reversion: if price is too high, bias downward; if too low, bias upward
            let mean_revert_bias = -deviation_pct * 0.2; // 20% reversion strength
            let price_var_pct = raw_var + mean_revert_bias;
            
            // Occasionally inject strong upward moves to trigger conditions
            let inject_trigger = (i % 500 == 0) && ((rng_state % 10) < 2);
            let final_var_pct = if inject_trigger {
                5.0 + ((rng_state % 300) as f64 / 100.0) // +5% to +8% spike
            } else {
                price_var_pct.max(-5.0).min(8.0) // Clamp to reasonable range
            };
            
            let new_price = (current_price as f64 * (1.0 + final_var_pct / 100.0)).max(1.0) as u64;
            current_prices[symbol_idx] = new_price;
            
            ticks.push(TradeTick::new(symbol_id, new_price, current_ts));
        }
        
        ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_feed_with_sbe_decoder() {
        let mut feed = DataFeed::new(true, 100);
        let rx = feed.get_receiver().unwrap();

        // Decode some ticks in the background
        let decoded = feed.decode_and_send(10);
        assert_eq!(decoded, 10);

        // Receive the ticks
        let mut received = 0;
        while let Ok(_tick) = rx.try_recv() {
            received += 1;
        }
        assert_eq!(received, 10);
    }

    #[test]
    fn test_data_feed_channel() {
        let mut feed = DataFeed::new(true, 50);
        let rx = feed.get_receiver().unwrap();

        // Send ticks through the channel
        let decoded = feed.decode_and_send(5);
        assert!(decoded > 0);

        // Verify we can receive
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, decoded);
    }
}
