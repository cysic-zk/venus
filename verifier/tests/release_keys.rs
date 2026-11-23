//! A release replaces `keys.rs` and appends the keys it replaced to `previous_keys.rs`. A key of this
//! release that is also listed as an earlier one would make every proof of this release look outdated,
//! which is what an un-regenerated `keys.rs` does.

#[test]
fn release_keys_are_not_earlier_releases_keys() {
    for hash in ["Poseidon1", "Poseidon2", "blake3"] {
        for compressed in [false, true] {
            if let Some(vk) = zisk_verifier::vadcop_vk(hash, compressed) {
                if let Some(old) = zisk_verifier::previous_release_of(&vk) {
                    panic!(
                        "{hash} (compressed: {compressed}): this release's key {vk:?} is {}'s; \
                         regenerate keys.rs with tools/release-artifacts/regen.sh",
                        old.release
                    );
                }
            }
        }
    }
}
