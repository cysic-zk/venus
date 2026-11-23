# Venus

Venus is a **compute backend acceleration and specialization** built on top of [ZisK](https://github.com/0xPolygonHermez/zisk), developed by the [Cysic Labs](https://cysic.xyz) team.

The majority of the codebase originates from the ZisK project. We are deeply grateful to the ZisK team ([0xPolygonHermez](https://github.com/0xPolygonHermez)) for their extraordinary contributions to the community in developing a high-performance zero-knowledge proof zkVM.

Venus follows the same dual-license model as ZisK (Apache 2.0 / MIT). Cysic aims to contribute our expertise in compute backend acceleration to Ethereum and the broader open-source community, with a particular focus on GPU, FPGA and ASIC acceleration.

Venus v0.5.0 is built on ZisK v1.4.0-alpha and pil2-proofman v1.4.0-alpha. Unlike v0.4.0, it changes the circuits and the proving key (the setup) as well as the GPU prover, at the same security level: its proving key, verification keys and proofs are its own, and are not interchangeable with ZisK's or with those of Venus v0.4.0. Venus publishes no proving keys: you generate the key from this repository (see [Generating the proving key](#generating-the-proving-key)).

## Monorepo Structure

This repository is a monorepo consolidating projects originally from [0xPolygonHermez](https://github.com/0xPolygonHermez):

| Component | Origin | Description |
|---|---|---|
| `zisk` (repository root) | [0xPolygonHermez/zisk](https://github.com/0xPolygonHermez/zisk) v1.4.0-alpha | zkVM core: state machines, emulator, executor, PIL definitions, distributed prover, CLI tools |
| `pil2-proofman/` | [0xPolygonHermez/pil2-proofman](https://github.com/0xPolygonHermez/pil2-proofman) v1.4.0-alpha | Rust proving backend with GPU (CUDA) acceleration, with its sppark and blst dependencies vendored |
| `pil2-compiler/` | [0xPolygonHermez/pil2-compiler](https://github.com/0xPolygonHermez/pil2-compiler) develop-0.14.0 | PIL (Polynomial Identity Language) compiler, the one every Venus proving key is built with |

We chose this monorepo consolidation because we believe a clean repository with minimal external dependencies facilitates rapid development iteration.

**Attribution**: The code in this repository originates from the [0xPolygonHermez](https://github.com/0xPolygonHermez) team and the ZisK project. Cysic has contributed the changes described below, mainly under `pil2-proofman/`, `setup/`, the executor and the emulators.

## What Cysic Contributes

Cysic specializes in hardware acceleration for zero-knowledge proof systems, with expertise spanning GPU, FPGA, and ASIC backends. On top of ZisK v1.4.0-alpha, Venus v0.5.0 contributes:

1. **Fewer, fuller recursion proofs** -- The BLAKE3 recursion aggregates up to four proofs in a 2^20-row aggregator (up to five in a recursive1), and one recursive1 circuit can verify a proof of each of a few small AIRs (recursive1 groups). Keccakf is verified by recursive1 directly instead of through a compressor. Together with smaller variants of sparsely filled AIRs and a single virtual table, a block needs fewer and fuller recursion proofs.

2. **GPU prover** -- The LDE batches its NTTs by transform size and alternates them over two streams; the fused LDE launch skips its zero-padded first stages; the basic proofs reuse the stage-1 Merkle tree that their contribution already built; quotient kernels are tuned to their largest spill-free chunk; opening evaluations, the recursion BLAKE3 witness rows and narrow NTT batches are faster (v0.4.0).

3. **Pipeline and execution** -- Multi-GPU proof pipelining (v0.4.0); the memory-ops emulator takes the precompile results of the minimal-trace one instead of recomputing them; witness rows are streamed into the trace with non-temporal stores; the ROM witness starts once its histogram is ready; the host no longer spins in the CUDA driver during uploads.

4. **Hardening** -- About thirty fixes: the assembly emulators confine every guest memory access, precompile operand and shared-memory read to what is mapped; the decoding of proofs and inputs is bounded; ROM code and data are bounded before they are transpiled or loaded; malformed worker contributions are refused; `cargo-zisk verify` pins and reports the statement it verified.

The security parameters are those of ZisK v1.4.0-alpha: FRI soundness is provable in the Johnson bound regime at the same target level, and the number of queries of every proof follows from its blowup factor, as in ZisK.

## Performance

Ethereum mainnet blocks proved with the [ziskethone](https://github.com/0xPolygonHermez/ziskethone) guest on RTX 5090 GPUs (GPUs 0-3 of one machine), one block at a time through the coordinator and one GPU worker. Times are the coordinator's job duration (execution, contributions, inner proofs and final proof), averaged over the blocks. Every proof was verified and its committed state root checked.

Blocks 26186000-26186099 against ZisK v1.4.0-alpha:

| GPUs | ZisK v1.4.0-alpha | Venus v0.5.0 | Change |
|---|---:|---:|---:|
| 1 | 8.729 s | 6.476 s | -25.8% (1.35x) |
| 2 | 4.898 s | 3.705 s | -24.4% (1.32x) |
| 4 | 3.190 s | 2.472 s | -22.5% (1.29x) |

Blocks 26186000-26186009 against Venus v0.4.0:

| GPUs | Venus v0.4.0 | Venus v0.5.0 | Change |
|---|---:|---:|---:|
| 1 | 7.935 s | 5.930 s | -25.3% (1.34x) |
| 2 | 4.407 s | 3.420 s | -22.4% (1.29x) |
| 4 | 2.886 s | 2.310 s | -20.0% (1.25x) |

Run-to-run variation is about 1% at 4 GPUs.

## Getting Started

Venus is used like ZisK v1.4.0-alpha: follow the ZisK [Quickstart](https://0xpolygonhermez.github.io/zisk-docs/developer/getting-started/quickstart) guide and [documentation](https://0xpolygonhermez.github.io/zisk-docs/), with the binaries built from this repository and the proving key generated below. For Ethereum blocks, use the [ziskethone](https://github.com/0xPolygonHermez/ziskethone) guest.

GPU proving requires Linux x86_64 with an NVIDIA GPU and the CUDA toolkit. Build the tools and install the guest toolchain:

```bash
cargo build --release
target/release/cargo-zisk toolchain install
```

### Generating the proving key

Venus publishes no proving keys and no verification keys, and the ZisK proving key (from `ziskup`) does not work with Venus v0.5.0. Generate the key from this repository:

```bash
tools/test-env/setup_build.sh --build-dir $HOME/.zisk --gen-exps --setup-jobs 8 --recursive-jobs 6
```

This writes `$HOME/.zisk/provingKey`, where the tools look for it by default. With these job counts it took 41 minutes on a 32-core machine, with a peak of 58 GB of memory; lower them on a machine with less memory (both default to 1, the slowest). It needs Node.js 20 or newer with npm (the PIL is compiled with the vendored `pil2-compiler`, whose Node dependencies are installed on first use) and the CUDA toolkit (`--gen-exps` compiles the GPU kernels; omit it for a CPU-only key). The default hash family is BLAKE3; pass `--hash-mode Poseidon1` or `--hash-mode Poseidon2` for the others.

The key takes 34 GB of disk, and the GPU prover caches another 15 GB of constants in it on its first proof. The setup also leaves 18 GB of intermediate files in `$HOME/.zisk/build`, which proving and verification do not use: delete them once you have generated the keys you need.

The setup is deterministic: a key generated from this release has exactly the verification keys that this release's verifier embeds, so `cargo-zisk verify` checks its proofs with no key argument. A key generated by another release is refused by the prover, and a proof made with an earlier release's key is reported as outdated, each with these instructions.

#### SNARK wrapper (optional)

The pilfflonk SNARK wrapper, for on-chain verification, needs a powers-of-tau file. Venus uses the Perpetual Powers of Tau file `ppot_0080_25.ptau` (38,654,798,994 bytes, SHA-256 `a91fd8e8ed3332b5ebafc90bc05329c874133f7d47c6bfe33e26dca0c2d11ee1`). Download it next to the repository, check it, and generate the SNARK key from the proving key:

```bash
(cd .. && curl -L -O https://pse-trusted-setup-ppot.s3.eu-central-1.amazonaws.com/pot28_0080/ppot_0080_25.ptau)
sha256sum ../ppot_0080_25.ptau
PTAU_PATH=../ppot_0080_25.ptau FINAL_SNARK=pilfflonk tools/test-env/setup_build.sh --build-dir $HOME/.zisk --snark
```

This writes `$HOME/.zisk/provingKeySnark` (7 GB, in about 20 minutes). A SNARK key wraps one proving key: generate it again whenever you generate the proving key. The Solidity verifiers of this release's keys are in `zisk-contracts/`.

## License

All crates in this monorepo are licensed under one of the following options:

- The Apache License, Version 2.0 (see LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)

- The MIT License (see LICENSE-MIT or http://opensource.org/licenses/MIT)

You may choose either license at your discretion.

## Acknowledgements and Declaration

Venus should be understood as Cysic's optimization, extension, and specialization of compute backends (GPU, FPGA, ASIC) built on top of the ZisK system.

We extend our deepest gratitude to the [ZisK](https://github.com/0xPolygonHermez/zisk) team and [0xPolygonHermez](https://github.com/0xPolygonHermez) for building and open-sourcing a high-performance zero-knowledge proving virtual machine. Their foundational work makes projects like Venus possible.

We also thank the [Plonky3](https://github.com/Plonky3/Plonky3) team for their contributions to zero-knowledge proving systems, the [RISC-V](https://github.com/riscv) community for providing a robust ISA that enables the zkVM model, the [Privacy & Scaling Explorations](https://pse.dev) team for the Perpetual Powers of Tau, and the broader open-source cryptography and ZK research communities whose work continues to advance scalable zero-knowledge technologies.

## Disclaimer: Software Under Development

This software is currently under **active development** and has not been audited for security or correctness.

Please be aware of the following:
* The software is **not fully tested**.
* **Do not use it in production environments** until a stable production release is available.
* Additional functionalities and optimizations **are planned for future releases**.
* Future updates may introduce breaking **backwards compatible changes** as development progresses.
* GPU proving requires Linux with an NVIDIA GPU and CUDA; macOS is not supported for GPU proving.

If you encounter any errors or unexpected behavior, please report them. Your feedback is highly appreciated in improving the software.
