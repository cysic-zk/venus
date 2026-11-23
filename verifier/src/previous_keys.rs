//! The vadcop_final keys of earlier releases, hand-maintained, unlike `keys.rs`.
//!
//! A key or proof from one of them is an outdated setup, not an unknown one, and is reported as such,
//! with how to generate this release's key. When a release replaces `keys.rs`, append the keys it
//! replaced here.

use crate::VADCOP_VK_LEN_WORDS;

/// One earlier release's key for one family and stage.
pub struct PreviousReleaseKey {
    /// The release, as users know it.
    pub release: &'static str,
    /// Hash family: `Poseidon1`, `Poseidon2` or `blake3`.
    pub hash: &'static str,
    /// The `vadcop_final_compressed` stage's key, not `vadcop_final`'s.
    pub compressed: bool,
    pub vk: [u64; VADCOP_VK_LEN_WORDS],
}

/// Venus v0.4.0 proved with ZisK v1.4.0-alpha's proving key: these are that key's.
const V0_4_0: &str = "Venus v0.4.0 (the ZisK v1.4.0-alpha proving key)";

pub const PREVIOUS_RELEASE_KEYS: &[PreviousReleaseKey] = &[
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "blake3",
        compressed: false,
        vk: [17146226293136250724, 16833344589786796689, 10321050178608571017, 2095991909911874349],
    },
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "blake3",
        compressed: true,
        vk: [12858410571421223333, 7366065819410512846, 4168917461533729924, 17247924614817440033],
    },
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "Poseidon1",
        compressed: false,
        vk: [16807801129105003610, 1480129380269065486, 6842518844123700219, 2605612804213468774],
    },
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "Poseidon1",
        compressed: true,
        vk: [3936316867833434652, 13002114981730899170, 17743900043057412555, 17794984613572371221],
    },
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "Poseidon2",
        compressed: false,
        vk: [7235627588029747190, 4964294244843319518, 15944021041411876293, 9165814394267558940],
    },
    PreviousReleaseKey {
        release: V0_4_0,
        hash: "Poseidon2",
        compressed: true,
        vk: [4751769779349204665, 8025699667731212605, 10343005230049885991, 17515878728116835452],
    },
];

/// The earlier release whose vadcop key (of any family or stage) is `vk`, if any.
pub fn previous_release_of(vk: &[u64]) -> Option<&'static PreviousReleaseKey> {
    PREVIOUS_RELEASE_KEYS.iter().find(|k| k.vk.as_slice() == vk)
}
