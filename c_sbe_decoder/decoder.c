#include "decoder.h"
#include <string.h>

/**
 * Stub implementation of SBE decoder
 * In a real implementation, this would decode from actual SBE wire format
 * For now, it returns synthetic data for testing
 */

static uint64_t call_count = 0;
static const uint32_t NUM_SYMBOLS = 10;
static const uint64_t BASE_TIMESTAMP = 1700000000000ULL;

int sbe_decode_next(struct RawTick* out) {
    if (out == NULL) {
        return -1;  /* Error: null pointer */
    }

    /* Generate synthetic tick data for testing */
    /* In real implementation, this would read from SBE wire buffer */
    
    /* Simple pseudo-random generation for testing */
    call_count++;
    
    /* Generate symbol_id (0 to NUM_SYMBOLS-1) */
    out->symbol_id = (uint32_t)(call_count % NUM_SYMBOLS);
    
    /* Generate price (e.g., between 10 and 100 USDT in e8 format) */
    uint64_t base_price = (10ULL + (call_count * 7) % 90) * 100000000ULL;
    /* Add some variation */
    int64_t variation = ((int64_t)(call_count * 13) % 1000000000LL) - 500000000LL;
    out->px_e8 = (uint64_t)((int64_t)base_price + variation);
    
    /* Generate timestamp (incrementing) */
    out->ts_unix_ms = BASE_TIMESTAMP + call_count;
    
    /* Return 1 to indicate success */
    /* In a real implementation, would return 0 when no more data available */
    return 1;
}
