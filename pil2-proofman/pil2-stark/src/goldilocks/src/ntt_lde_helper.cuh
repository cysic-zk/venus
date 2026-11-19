#ifndef NTT_LDE_HELPER_CUH
#define NTT_LDE_HELPER_CUH

#include <cuda_runtime.h>

// An LDE on `stream` may split its batches over a helper stream (NTTGoldilocksGPU::ldeColMajor)
// once the stream's owner attaches one: created at attach time, on the current device and at the
// stream's priority. Owners attach long-lived streams after creating them and detach before
// destroying them; an LDE on any other stream runs on that stream alone. Default-stream handles
// are refused: they name a different stream in every thread.
void nttAttachLdeHelper(cudaStream_t stream);
void nttDetachLdeHelper(cudaStream_t stream);

#endif
