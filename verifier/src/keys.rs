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
    [15461407890693697661, 13265554553530680587, 6614213292858300065, 9823939739648210575];

pub const VADCOP_FINAL_COMPRESSED_VK_POSEIDON1: [u64; VADCOP_VK_LEN_WORDS] =
    [13574303488310497724, 5254435154700575357, 909982639573570008, 17902495290010062366];

pub const PILFFLONK_VKEY_DIGEST_POSEIDON1: Option<&str> =
    Some("0xb3ec443b322b5a4e0232edeb07adeb8aa91deb20344bb72152d1e7d887b92ad3");

pub const VADCOP_FINAL_VK_POSEIDON2: [u64; VADCOP_VK_LEN_WORDS] =
    [3182186370080328716, 12563819335885145081, 14560845723196067463, 3055073783397912426];

pub const VADCOP_FINAL_COMPRESSED_VK_POSEIDON2: [u64; VADCOP_VK_LEN_WORDS] =
    [6267310520184080941, 13030771853862324649, 7205839900808650065, 10648367035715451111];

pub const PILFFLONK_VKEY_DIGEST_POSEIDON2: Option<&str> =
    Some("0x2f256374ad4463ba0435d66ef202d8a52677c69615861ccad5203ea402af87c4");

pub const VADCOP_FINAL_VK_BLAKE3: [u64; VADCOP_VK_LEN_WORDS] =
    [3326678779613212670, 3106253548763207250, 14260447671364541094, 14250829146210478116];

pub const VADCOP_FINAL_COMPRESSED_VK_BLAKE3: [u64; VADCOP_VK_LEN_WORDS] =
    [9252516614610794390, 13335112760342477626, 7752790433985556446, 10362225625644650439];

pub const PILFFLONK_VKEY_DIGEST_BLAKE3: Option<&str> =
    Some("0x35d115a7eed2e4bf3587c0c0f21cf35fc9c77f77444209be16234c16845a039a");
