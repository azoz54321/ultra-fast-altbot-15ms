use crate::data_feed::TradeTick;

/// RawTick structure with C-compatible layout
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawTick {
    pub symbol_id: u32,
    pub px_e8: u64,
    pub ts_unix_ms: u64,
}

impl RawTick {
    /// Create a new RawTick (for testing)
    #[allow(dead_code)]
    pub fn new(symbol_id: u32, px_e8: u64, ts_unix_ms: u64) -> Self {
        Self {
            symbol_id,
            px_e8,
            ts_unix_ms,
        }
    }
}

extern "C" {
    /// FFI binding to C SBE decoder
    /// Returns: 1 on success, 0 if no more data, -1 on error
    fn sbe_decode_next(out: *mut RawTick) -> i32;
}

/// Safe Rust wrapper for SBE decoder
pub struct SbeDecoderFfi;

impl SbeDecoderFfi {
    /// Create a new SBE decoder
    pub fn new() -> Self {
        Self
    }

    /// Decode next tick into TradeTick
    /// Returns true if a tick was decoded, false if no more data
    pub fn decode_into(&mut self, tick: &mut TradeTick) -> bool {
        let mut raw_tick = RawTick {
            symbol_id: 0,
            px_e8: 0,
            ts_unix_ms: 0,
        };

        // SAFETY: raw_tick is a valid pointer to RawTick with C-compatible layout
        let result = unsafe { sbe_decode_next(&mut raw_tick as *mut RawTick) };

        match result {
            1 => {
                // Success: copy data to TradeTick
                tick.symbol_id = raw_tick.symbol_id;
                tick.px_e8 = raw_tick.px_e8;
                tick.ts_unix_ms = raw_tick.ts_unix_ms;
                true
            }
            0 => {
                // No more data
                false
            }
            _ => {
                // Error
                false
            }
        }
    }
}

impl Default for SbeDecoderFfi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_tick_layout() {
        // Verify RawTick has C-compatible layout
        // Size: u32 (4 bytes) + padding (4 bytes) + u64 (8 bytes) + u64 (8 bytes) = 24 bytes
        assert_eq!(std::mem::size_of::<RawTick>(), 24);
        assert_eq!(std::mem::align_of::<RawTick>(), 8);
    }

    #[test]
    fn test_sbe_decoder_decode() {
        let mut decoder = SbeDecoderFfi::new();
        let mut tick = TradeTick::new(0, 0, 0);
        
        // Should be able to decode at least one tick (stub returns synthetic data)
        let success = decoder.decode_into(&mut tick);
        assert!(success);
        
        // Check that tick has reasonable values
        assert!(tick.px_e8 > 0);
        assert!(tick.ts_unix_ms > 0);
    }

    #[test]
    fn test_sbe_decoder_multiple_decodes() {
        let mut decoder = SbeDecoderFfi::new();
        
        // Decode multiple ticks and verify they're different
        let mut tick1 = TradeTick::new(0, 0, 0);
        let mut tick2 = TradeTick::new(0, 0, 0);
        
        assert!(decoder.decode_into(&mut tick1));
        assert!(decoder.decode_into(&mut tick2));
        
        // Ticks should be different (timestamps at minimum)
        assert_ne!(tick1.ts_unix_ms, tick2.ts_unix_ms);
    }
}

