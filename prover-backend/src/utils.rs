use proofman_fields::PrimeField64;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use zisk_common::ProgramVK;

use crate::{GuestProgram, ProgramId};
use proofman_common::{
    initialize_logger, json_to_debug_instances_map, DebugInfo, ProofCtx, ProofmanResult,
    VerboseMode,
};
use zisk_rom_setup::{get_elf_data_hash, get_rom_path, rom_merkle_setup, HashMode};

fn hash_mode_from_pctx<F: PrimeField64>(pctx: &ProofCtx<F>) -> Result<HashMode> {
    HashMode::from_str(&pctx.global_info.hash).with_context(|| {
        format!(
            "proving key global_info.hash {:?} is not a recognized HashMode",
            pctx.global_info.hash
        )
    })
}

/// Build the ROM Merkle setup for `elf` and return its program verification
/// key, using the hash mode declared in the proving key's global info.
pub fn ensure_program_vk<F: PrimeField64>(
    pctx: &ProofCtx<F>,
    elf: &GuestProgram,
) -> Result<ProgramVK> {
    let hash_mode = hash_mode_from_pctx(pctx)?;
    rom_merkle_setup(pctx, elf.elf(), &None, false, hash_mode)
}

/// Resolve the on-disk path of the compiled ROM binary for `program_id`,
/// using the hash mode declared in the proving key's global info.
pub fn get_rom_bin_path<F: PrimeField64>(
    pctx: &ProofCtx<F>,
    program_id: &ProgramId,
) -> Result<PathBuf> {
    let hash_mode = hash_mode_from_pctx(pctx)?;
    let rom_bin_path = get_rom_path(pctx, program_id.get_hash(), &None, hash_mode)?;
    Ok(rom_bin_path)
}

/// Return the `(minimal-trace, rom-histogram)` ASM binary filenames for `elf`.
///
/// Named by `zisk_rom_setup`, which generates these files, so the two cannot
/// disagree: the name is the ELF hash and the hints mode.
pub fn get_asm_paths(elf: &GuestProgram, with_hints: bool) -> Result<(String, String)> {
    let hash = get_elf_data_hash(elf.elf());
    let [mt, rh, ..] =
        zisk_rom_setup::get_assembly_file_paths_from_id(&hash, Path::new(""), with_hints);
    let file_name = |path: PathBuf| path.to_string_lossy().into_owned();

    Ok((file_name(mt), file_name(rh)))
}

/// A SNARK key wraps one vadcop_final key, recorded as its `vadcop_final.verkey.json`: refuse one
/// made for another proving key (before the proving key was generated again, or by another
/// release), which would wrap proofs into SNARKs that verify under nothing this key proves.
pub fn check_snark_key(proving_key: &Path, proving_key_snark: &Path) -> Result<()> {
    let wraps_path = proving_key_snark.join("vadcop_final.verkey.json");
    if !wraps_path.is_file() {
        anyhow::bail!(
            "no SNARK key at {} ({} is missing). {}",
            proving_key_snark.display(),
            wraps_path.display(),
            zisk_setup::REGENERATE_SNARK_KEY
        );
    }
    let text = std::fs::read_to_string(&wraps_path)
        .with_context(|| format!("failed to read {}", wraps_path.display()))?;
    let wraps: Vec<u64> = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a JSON array of u64", wraps_path.display()))?;
    let leaf = proofman::get_vadcop_final_proof_vkey(proving_key, false).map_err(|e| {
        anyhow::anyhow!("failed to read the vadcop_final key of {}: {e}", proving_key.display())
    })?;
    if wraps != leaf {
        anyhow::bail!(
            "the SNARK key at {} wraps the vadcop_final key {wraps:?}, but the proving key at {} has \
             {leaf:?}: it was generated for another proving key. {}",
            proving_key_snark.display(),
            proving_key.display(),
            zisk_setup::REGENERATE_SNARK_KEY
        );
    }
    Ok(())
}

/// Return an error if `path` does not exist.
pub fn check_paths_exist(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {:?}", path));
    }
    Ok(())
}

/// Translate the CLI-style debug selector into a [`DebugInfo`]:
/// `None` → no debugging, `Some(None)` → debug all instances,
/// `Some(Some(spec))` → debug only the instances named in `spec`.
pub fn create_debug_info(
    debug_info: Option<Option<String>>,
    proving_key: PathBuf,
) -> ProofmanResult<DebugInfo> {
    match &debug_info {
        None => Ok(DebugInfo::default()),
        Some(None) => Ok(DebugInfo::new_debug()),
        Some(Some(debug_value)) => json_to_debug_instances_map(proving_key, debug_value.clone()),
    }
}

/// Initialize the global logger at the given verbosity (non-distributed).
pub fn setup_logger(verbose: VerboseMode) {
    initialize_logger(verbose, None);
}
