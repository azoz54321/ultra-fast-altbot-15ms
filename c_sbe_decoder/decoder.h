#ifndef SBE_DECODER_H
#define SBE_DECODER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Raw tick data structure matching Rust #[repr(C)]
 */
struct RawTick {
    uint32_t symbol_id;   // Symbol identifier
    uint64_t px_e8;       // Price in fixed-point (scaled by 1e8)
    uint64_t ts_unix_ms;  // Unix timestamp in milliseconds
};

/**
 * Decode next SBE message into RawTick
 * Returns 1 on success, 0 on end-of-stream, -1 on error
 * 
 * @param out Pointer to RawTick structure to fill
 * @return 1 on success, 0 on end-of-stream, -1 on error
 */
int sbe_decode_next(struct RawTick* out);

/**
 * Initialize the SBE decoder (placeholder for future implementation)
 * Returns 0 on success, -1 on error
 */
int sbe_decoder_init(void);

/**
 * Cleanup the SBE decoder resources
 */
void sbe_decoder_cleanup(void);

#ifdef __cplusplus
}
#endif

#endif // SBE_DECODER_H
