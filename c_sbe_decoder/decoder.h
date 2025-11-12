#ifndef SBE_DECODER_H
#define SBE_DECODER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * RawTick structure matching Rust's #[repr(C)] layout
 * Represents a decoded trade tick from SBE format
 */
struct RawTick {
    uint32_t symbol_id;      /* Symbol identifier */
    uint64_t px_e8;          /* Price in fixed-point (scaled by 1e8) */
    uint64_t ts_unix_ms;     /* Unix timestamp in milliseconds */
};

/**
 * Decode the next tick from SBE wire format
 * 
 * @param out Pointer to RawTick structure to fill
 * @return 1 if a tick was decoded successfully, 0 if no more data, -1 on error
 */
int sbe_decode_next(struct RawTick* out);

#ifdef __cplusplus
}
#endif

#endif /* SBE_DECODER_H */
