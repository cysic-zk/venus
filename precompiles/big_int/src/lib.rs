mod add256;
mod add256_constants;
mod add256_mem_inputs;

pub use add256::*;
pub use add256_constants::*;

zisk_common::zisk_precompile! {
    name = Add256,
    op_type = BigInt,
    row = Add256Trace,
    // The same air at two heights: blocks usually carry a few hundred additions, which the short
    // air holds at a sixty-fourth of the tall one's cost.
    traces = [
        (Add256Trace, ::zisk_pil::Add256Trace::<()>::NUM_ROWS, ::zisk_pil::ADD_256_INSTANCE_COST),
        (
            Add256SmallTrace,
            ::zisk_pil::Add256SmallTrace::<()>::NUM_ROWS,
            ::zisk_pil::ADD_256_SMALL_INSTANCE_COST
        ),
    ],
    ops = [
        (OperationAdd256Data, Add256Input),
    ],
}

#[cfg(test)]
mod add256_tests {
    use zisk_common::io::ZiskStdin;
    use zisk_test_artifacts::ELF_ADD256;

    #[test]
    fn add256_tests() {
        ELF_ADD256.run_emulation(ZiskStdin::new(), None).expect("add256 guest emulation failed");
    }
}
