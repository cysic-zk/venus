// Written by `cargo-zisk-dev release-artifacts render` (tools/release-artifacts/regen.sh).

//! The keys of the release setup the verifiers in this crate were generated from.
//!
//! `VADCOP_FINAL_*`: the `<stage>.verkey.json` of each family's proving key. A key is the root of
//! a stage's constant tree. It is shared by every ZisK program and identifies the prover, not the
//! program: check `Verified::program_vk` for that. It covers leaf proofs only; a recurser proof
//! verifies under its recurser's own key.
//!
//! `PILFFLONK_VKEY_DIGEST_*`: the `digest` of each family's `pilfflonk.vkey.json`, which pins the
//! key (see [`pilfflonk_vkey_digest`](crate::pilfflonk_vkey_digest)).
//!
//! Regenerate with `tools/release-artifacts/regen.sh`, together with the verifiers.

use crate::VADCOP_VK_LEN_WORDS;

pub const VADCOP_FINAL_VK_POSEIDON1: [u64; VADCOP_VK_LEN_WORDS] =
    [16807801129105003610, 1480129380269065486, 6842518844123700219, 2605612804213468774];

pub const VADCOP_FINAL_COMPRESSED_VK_POSEIDON1: [u64; VADCOP_VK_LEN_WORDS] =
    [3936316867833434652, 13002114981730899170, 17743900043057412555, 17794984613572371221];

pub const PILFFLONK_VKEY_DIGEST_POSEIDON1: Option<&str> =
    Some("0xe8f7e04ba2ff5bedda661368eb890d4b70fa8c139cac772ef42cb30cac2f454b");

pub const VADCOP_FINAL_VK_POSEIDON2: [u64; VADCOP_VK_LEN_WORDS] =
    [7235627588029747190, 4964294244843319518, 15944021041411876293, 9165814394267558940];

pub const VADCOP_FINAL_COMPRESSED_VK_POSEIDON2: [u64; VADCOP_VK_LEN_WORDS] =
    [4751769779349204665, 8025699667731212605, 10343005230049885991, 17515878728116835452];

pub const PILFFLONK_VKEY_DIGEST_POSEIDON2: Option<&str> =
    Some("0xe761f9f2dafc828573770c6ab829d0fa97e858133aaf46bed6ee6096f9fb569e");

pub const VADCOP_FINAL_VK_BLAKE3: [u64; VADCOP_VK_LEN_WORDS] =
    [3326678779613212670, 3106253548763207250, 14260447671364541094, 14250829146210478116];

pub const VADCOP_FINAL_COMPRESSED_VK_BLAKE3: [u64; VADCOP_VK_LEN_WORDS] =
    [9252516614610794390, 13335112760342477626, 7752790433985556446, 10362225625644650439];

pub const PILFFLONK_VKEY_DIGEST_BLAKE3: Option<&str> =
    Some("0x35d115a7eed2e4bf3587c0c0f21cf35fc9c77f77444209be16234c16845a039a");
