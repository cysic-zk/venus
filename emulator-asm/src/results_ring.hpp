#ifndef EMULATOR_ASM_RESULTS_RING_HPP
#define EMULATOR_ASM_RESULTS_RING_HPP

#include <stdint.h>
#include <stdbool.h>

// Precompile and free-call results handed from the minimal-trace emulator to the memory-ops one.
//
// Both emulators run the same program on the same input, so they make the same precompile and
// fcall calls in the same order. The minimal-trace (MT) service computes every result and
// appends it to a ring in shared memory; the memory-ops (MO) service, the slower of the two,
// copies each result from the ring instead of computing it again. A record is
// { job, op << 32 | bytes, payload }, the payload padded to 8 bytes.
//
// The MO service checks the job and op of every record and that the run's job is still the
// ring's after copying it. If the ring has no record for it in time, or holds another run's, MO
// computes that result and every later one itself: the ring saves work, it never decides one.

// Ring data bytes; a power of two. A run's results fit many times over (about 30 MB for a
// 60M-step block), so the producer only waits for room behind a consumer that has stalled.
#define RESULTS_RING_DATA_SIZE (256ull << 20)
#define RESULTS_RING_HEADER_SIZE 4096ull
#define RESULTS_RING_SHM_SIZE (RESULTS_RING_HEADER_SIZE + RESULTS_RING_DATA_SIZE)

// Record ops: one per precompile entry point, so a misordered replay is caught at once.
typedef enum {
    RESULTS_OP_KECCAK = 1,
    RESULTS_OP_SHA256,
    RESULTS_OP_BLAKE2B,
    RESULTS_OP_BLAKE3,
    RESULTS_OP_BLAKE2S,
    RESULTS_OP_POSEIDON2_KOALABEAR,
    RESULTS_OP_POSEIDON2,
    RESULTS_OP_POSEIDON1,
    RESULTS_OP_ARITH256,
    RESULTS_OP_ARITH256_MOD,
    RESULTS_OP_ARITH384_MOD,
    RESULTS_OP_SECP256K1_ADD,
    RESULTS_OP_SECP256K1_DBL,
    RESULTS_OP_SECP256R1_ADD,
    RESULTS_OP_SECP256R1_DBL,
    RESULTS_OP_FCALL,
    RESULTS_OP_BN254_CURVE_ADD,
    RESULTS_OP_BN254_CURVE_DBL,
    RESULTS_OP_BABYJUBJUB_ADD,
    RESULTS_OP_ED25519_ADD,
    RESULTS_OP_BN254_COMPLEX_ADD,
    RESULTS_OP_BN254_COMPLEX_SUB,
    RESULTS_OP_BN254_COMPLEX_MUL,
    RESULTS_OP_BLS12_381_CURVE_ADD,
    RESULTS_OP_BLS12_381_CURVE_DBL,
    RESULTS_OP_BLS12_381_COMPLEX_ADD,
    RESULTS_OP_BLS12_381_COMPLEX_SUB,
    RESULTS_OP_BLS12_381_COMPLEX_MUL,
    RESULTS_OP_ADD256,
} ResultsOp;

// Maps the ring segment (shm_open'ed by the caller) at `address`.
void results_ring_attach(void * address);

// Called at the start of every run, with the job the request carries (0: no ring for this run).
// The MT service starts producing for `job`; the MO service starts consuming it.
void results_ring_begin(uint64_t job);

// Called at the end of every run; `failed` when the emulation stopped early, which tells a consumer
// waiting for more of the producer's records at once that none will come.
void results_ring_end(bool failed);

// Consumer: copies the next result, of `bytes`, to `dst` and returns true; false when this run
// computes its results itself (no ring, or the ring has stopped serving it).
bool results_take(ResultsOp op, void * dst, uint64_t bytes);

// Consumer, variable size: copies the next result into `dst` (at most `max_bytes`) and returns its
// size, or 0 when this run computes its results itself.
uint64_t results_take_var(ResultsOp op, void * dst, uint64_t max_bytes);

// Producer: appends a computed result. A no-op unless this run produces.
void results_give(ResultsOp op, const void * src, uint64_t bytes);

#endif // EMULATOR_ASM_RESULTS_RING_HPP
