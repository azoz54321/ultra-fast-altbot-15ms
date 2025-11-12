#include "decoder.h"
#include <string.h>

// Static state for stub implementation
static int decoder_initialized = 0;
static int tick_counter = 0;

/**
 * Initialize the SBE decoder
 */
int sbe_decoder_init(void) {
    decoder_initialized = 1;
    tick_counter = 0;
    return 0;
}

/**
 * Cleanup the SBE decoder resources
 */
void sbe_decoder_cleanup(void) {
    decoder_initialized = 0;
    tick_counter = 0;
}

/**
 * Decode next SBE message into RawTick
 * 
 * This is a STUB implementation that generates synthetic ticks.
 * In production, this would decode actual SBE-encoded market data.
 */
int sbe_decode_next(struct RawTick* out) {
    if (!decoder_initialized || out == NULL) {
        return -1;
    }

    // Stub: generate synthetic tick data
    // In production, this would decode from actual SBE buffer
    
    // Simple LCG for reproducible random numbers
    static uint64_t rng_state = 42;
    rng_state = rng_state * 1103515245 + 12345;
    
    // Generate symbol_id (0-299 for 300 symbols)
    out->symbol_id = (uint32_t)(rng_state % 300);
    
    // Generate price (base price with variation)
    uint64_t base_price = (10 + (out->symbol_id * 13) % 990) * 100000000ULL;
    rng_state = rng_state * 1103515245 + 12345;
    int64_t price_var_pct = ((int64_t)(rng_state % 1000) - 200);
    out->px_e8 = (uint64_t)((double)base_price * (1.0 + (double)price_var_pct / 10000.0));
    
    // Generate timestamp (incrementing)
    out->ts_unix_ms = 1700000000000ULL + tick_counter;
    
    tick_counter++;
    
    // Return 1 for success (stub always succeeds)
    return 1;
}
