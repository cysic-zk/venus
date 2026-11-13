#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>
#include <sched.h>
#include <time.h>
#include <x86intrin.h>
#include "results_ring.hpp"
#include "globals.hpp"
#include "log.hpp"

// Ring header, one cache line per field written by a different side. x86 keeps stores and loads
// in program order (TSO), so the producer's record bytes are visible before the `written` that
// publishes them, and a consumer that reads a record and then still sees its job in `job` read
// bytes stored before the next run took the ring over.
typedef struct {
    uint64_t job;        // producer: the run whose records follow, stored after `written` is reset
    uint64_t pad0[7];
    uint64_t written;    // producer: record bytes appended for `job`
    uint64_t pad1[7];
    uint64_t read;       // consumer: record bytes taken, the producer's room
    uint64_t pad2[7];
    uint64_t consuming;  // consumer: the job it is taking records of, 0 when it is done
    uint64_t pad3[7];
} ResultsRingHeader;

_Static_assert(sizeof(ResultsRingHeader) <= RESULTS_RING_HEADER_SIZE, "ring header too large");
_Static_assert((RESULTS_RING_DATA_SIZE & (RESULTS_RING_DATA_SIZE - 1)) == 0, "ring size not a power of two");

#define RECORD_HEADER_BYTES 16ull

// A side that waits this long for the other gives the ring up for the rest of the run.
#define RESULTS_WAIT_NS (2ull * 1000 * 1000 * 1000)

typedef enum { RING_OFF = 0, RING_PRODUCE, RING_CONSUME } RingMode;

static ResultsRingHeader * header = NULL;
static uint8_t * data = NULL;
static RingMode mode = RING_OFF;
static uint64_t run_job = 0;
static uint64_t position = 0; // bytes this side has appended (producer) or taken (consumer)

static uint64_t now_ns(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

// Spins, then yields, until `ready()` holds or RESULTS_WAIT_NS pass; returns whether it holds.
#define WAIT_UNTIL(ready) ({                                                     \
    bool _ok = (ready);                                                          \
    uint64_t _spins = 0, _start = 0;                                             \
    while (!_ok) {                                                               \
        if (++_spins < 4096) {                                                   \
            _mm_pause();                                                         \
        } else {                                                                 \
            if (_start == 0) _start = now_ns();                                  \
            else if (now_ns() - _start > RESULTS_WAIT_NS) break;                 \
            sched_yield();                                                       \
        }                                                                        \
        _ok = (ready);                                                           \
    }                                                                            \
    _ok;                                                                         \
})

static inline uint64_t load_acquire(const uint64_t * p) { return __atomic_load_n(p, __ATOMIC_ACQUIRE); }
static inline void store_release(uint64_t * p, uint64_t v) { __atomic_store_n(p, v, __ATOMIC_RELEASE); }

static void ring_copy_out(void * dst, uint64_t at, uint64_t bytes)
{
    uint64_t offset = at & (RESULTS_RING_DATA_SIZE - 1);
    uint64_t first = RESULTS_RING_DATA_SIZE - offset;
    if (first >= bytes) {
        memcpy(dst, data + offset, bytes);
    } else {
        memcpy(dst, data + offset, first);
        memcpy((uint8_t *)dst + first, data, bytes - first);
    }
}

static void ring_copy_in(uint64_t at, const void * src, uint64_t bytes)
{
    uint64_t offset = at & (RESULTS_RING_DATA_SIZE - 1);
    uint64_t first = RESULTS_RING_DATA_SIZE - offset;
    if (first >= bytes) {
        memcpy(data + offset, src, bytes);
    } else {
        memcpy(data + offset, src, first);
        memcpy(data, (const uint8_t *)src + first, bytes - first);
    }
}

void results_ring_attach(void * address)
{
    header = (ResultsRingHeader *)address;
    data = (uint8_t *)address + RESULTS_RING_HEADER_SIZE;
}

void results_ring_begin(uint64_t job)
{
    mode = RING_OFF;
    run_job = job;
    position = 0;
    if ((header == NULL) || (job == 0)) return;

    if (gen_method == MinimalTrace)
    {
        // The previous run's consumer may still be reading: take the ring over only once it is
        // done, or after the wait, when it gives up on its own (it checks the job per record).
        if (!WAIT_UNTIL(load_acquire(&header->consuming) == 0))
        {
            asm_printf("results_ring_begin() the consumer of job %lu is still busy; job %lu takes the ring over\n",
                       load_acquire(&header->consuming), job);
        }
        store_release(&header->written, 0);
        store_release(&header->read, 0);
        __atomic_store_n(&header->job, job, __ATOMIC_SEQ_CST);
        mode = RING_PRODUCE;
    }
    else if (gen_method == MemOp)
    {
        // The producer resets the ring and then publishes the job; until it does, the ring holds
        // an older run's records.
        if (!WAIT_UNTIL(load_acquire(&header->job) == job))
        {
            asm_printf("results_ring_begin() no results for job %lu; this run computes its own\n", job);
            return;
        }
        __atomic_store_n(&header->consuming, job, __ATOMIC_SEQ_CST);
        // Taken after the job was seen: a producer that started the next run since then wins.
        if (load_acquire(&header->job) != job)
        {
            store_release(&header->consuming, 0);
            return;
        }
        mode = RING_CONSUME;
    }
}

void results_ring_end(bool failed)
{
    if (mode == RING_CONSUME) store_release(&header->consuming, 0);
    if ((mode == RING_PRODUCE) && failed) __atomic_store_n(&header->job, 0, __ATOMIC_SEQ_CST);
    mode = RING_OFF;
}

// The consumer gives the ring up: this result and the rest of the run are computed here.
static void consumer_stop(const char * why, ResultsOp op)
{
    asm_printf("results ring: %s at op %u, byte %lu of job %lu; computing the rest of the run\n",
               why, (unsigned)op, position, run_job);
    store_release(&header->consuming, 0);
    mode = RING_OFF;
}

// Reads the next record's header once the producer has published it; returns the payload size,
// or UINT64_MAX after giving the ring up.
static uint64_t next_record(ResultsOp op)
{
    if (!WAIT_UNTIL((load_acquire(&header->written) >= position + RECORD_HEADER_BYTES) ||
                    (load_acquire(&header->job) != run_job)))
    {
        consumer_stop("no record in time", op);
        return UINT64_MAX;
    }
    uint64_t rec[2];
    ring_copy_out(rec, position, RECORD_HEADER_BYTES);
    if (load_acquire(&header->job) != run_job)
    {
        consumer_stop("the ring moved on to another job", op);
        return UINT64_MAX;
    }
    if ((rec[0] != run_job) || ((rec[1] >> 32) != (uint64_t)op))
    {
        // Same program, same input: the two services can only disagree through a bug.
        asm_printf("results ring: record job %lu op %lu where job %lu op %u was expected (byte %lu)\n",
                   rec[0], rec[1] >> 32, run_job, (unsigned)op, position);
        exit(-1);
    }
    return rec[1] & 0xFFFFFFFFull;
}

static bool take_payload(ResultsOp op, void * dst, uint64_t bytes)
{
    uint64_t padded = (bytes + 7) & ~7ull;
    uint64_t end = position + RECORD_HEADER_BYTES + padded;
    if (!WAIT_UNTIL((load_acquire(&header->written) >= end) || (load_acquire(&header->job) != run_job)))
    {
        consumer_stop("no record payload in time", op);
        return false;
    }
    ring_copy_out(dst, position + RECORD_HEADER_BYTES, bytes);
    // Still this run's ring after the copy: the bytes were the producer's for this job.
    if (load_acquire(&header->job) != run_job)
    {
        consumer_stop("the ring moved on to another job", op);
        return false;
    }
    position = end;
    store_release(&header->read, position);
    return true;
}

bool results_take(ResultsOp op, void * dst, uint64_t bytes)
{
    if (mode != RING_CONSUME) return false;
    uint64_t size = next_record(op);
    if (size == UINT64_MAX) return false;
    if (size != bytes)
    {
        asm_printf("results ring: op %u record of %lu bytes where %lu were expected\n", (unsigned)op, size, bytes);
        exit(-1);
    }
    return take_payload(op, dst, bytes);
}

uint64_t results_take_var(ResultsOp op, void * dst, uint64_t max_bytes)
{
    if (mode != RING_CONSUME) return 0;
    uint64_t size = next_record(op);
    if (size == UINT64_MAX) return 0;
    if ((size == 0) || (size > max_bytes))
    {
        asm_printf("results ring: op %u record of %lu bytes, at most %lu expected\n", (unsigned)op, size, max_bytes);
        exit(-1);
    }
    return take_payload(op, dst, size) ? size : 0;
}

void results_give(ResultsOp op, const void * src, uint64_t bytes)
{
    if (mode != RING_PRODUCE) return;
    uint64_t padded = (bytes + 7) & ~7ull;
    uint64_t len = RECORD_HEADER_BYTES + padded;
    if (len > RESULTS_RING_DATA_SIZE) return; // never: results are a few KB at most
    if (!WAIT_UNTIL(position + len - load_acquire(&header->read) <= RESULTS_RING_DATA_SIZE))
    {
        // The consumer stopped taking records. Clearing the job tells it at once to compute the
        // rest of the run itself.
        asm_printf("results ring: no room for op %u at byte %lu of job %lu; no more results for this run\n",
                   (unsigned)op, position, run_job);
        __atomic_store_n(&header->job, 0, __ATOMIC_SEQ_CST);
        mode = RING_OFF;
        return;
    }
    uint64_t rec[2] = { run_job, ((uint64_t)op << 32) | bytes };
    ring_copy_in(position, rec, RECORD_HEADER_BYTES);
    ring_copy_in(position + RECORD_HEADER_BYTES, src, bytes);
    position += len;
    store_release(&header->written, position);
}
