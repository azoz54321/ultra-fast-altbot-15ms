use crate::data_feed::TradeTick;

/// Raw tick structure matching C ABI
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawTick {
    pub symbol_id: u32,
    pub px_e8: u64,
    pub ts_unix_ms: u64,
}

extern "C" {
    /// Initialize SBE decoder
    fn sbe_decoder_init() -> i32;
    
    /// Decode next tick from SBE stream
    fn sbe_decode_next(out: *mut RawTick) -> i32;
    
    /// Cleanup SBE decoder
    fn sbe_decoder_cleanup();
}

/// Safe wrapper for SBE decoder FFI
pub struct SbeDecoderFfi {
    initialized: bool,
}

impl SbeDecoderFfi {
    /// Create and initialize a new SBE decoder
    pub fn new() -> Result<Self, String> {
        unsafe {
            let ret = sbe_decoder_init();
            if ret != 0 {
                return Err(format!("Failed to initialize SBE decoder: {}", ret));
            }
        }
        Ok(Self { initialized: true })
    }

    /// Decode next tick into TradeTick structure
    /// Returns true if tick was decoded, false on end-of-stream
    pub fn decode_into(&mut self, tick: &mut TradeTick) -> bool {
        if !self.initialized {
            return false;
        }

        let mut raw_tick = RawTick {
            symbol_id: 0,
            px_e8: 0,
            ts_unix_ms: 0,
        };

        unsafe {
            let ret = sbe_decode_next(&mut raw_tick as *mut RawTick);
            if ret == 1 {
                // Success: map RawTick to TradeTick
                tick.symbol_id = raw_tick.symbol_id;
                tick.px_e8 = raw_tick.px_e8;
                tick.ts_unix_ms = raw_tick.ts_unix_ms;
                true
            } else {
                // End of stream or error
                false
            }
        }
    }
}

impl Default for SbeDecoderFfi {
    fn default() -> Self {
        Self::new().unwrap_or(Self { initialized: false })
    }
}

impl Drop for SbeDecoderFfi {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sbe_decoder_cleanup();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbe_decoder_init() {
        let decoder = SbeDecoderFfi::new();
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_decode_into() {
        let mut decoder = SbeDecoderFfi::new().unwrap();
        let mut tick = TradeTick::new(0, 0, 0);
        
        // Should successfully decode a tick (stub always succeeds)
        let result = decoder.decode_into(&mut tick);
        assert!(result);
        
        // Verify tick has been populated with non-zero values
        assert!(tick.px_e8 > 0);
    }
}
