//! Shared helpers for the harness runners. Wallet home comes from
//! `LEE_WALLET_HOME_DIR`; the sequencer address from that wallet's config.

use lee_core::program::ProgramId;

/// Guest artifacts the harness can deploy, by name.
pub struct Guest {
    /// Name as passed on the command line.
    pub name: &'static str,
    /// Embedded ELF.
    pub elf: &'static [u8],
    /// Image id, which is the on-chain program id.
    pub id: ProgramId,
}

/// Look up a guest by name.
#[must_use]
pub fn guest(name: &str) -> Option<Guest> {
    match name {
        "argo_lending" => Some(Guest {
            name: "argo_lending",
            elf: argo_lending_methods::ARGO_LENDING_ELF,
            id: argo_lending_methods::ARGO_LENDING_ID,
        }),
        "spike_vault" => Some(Guest {
            name: "spike_vault",
            elf: spike_vault_methods::SPIKE_VAULT_ELF,
            id: spike_vault_methods::SPIKE_VAULT_ID,
        }),
        _ => None,
    }
}

/// Parse a program id printed by `argo_deploy` (`PROGRAM_ID=<hex of 8 LE u32>`).
pub fn parse_program_id(s: &str) -> anyhow::Result<ProgramId> {
    let bytes = hex::decode(s.trim())?;
    anyhow::ensure!(bytes.len() == 32, "program id must be 32 bytes");
    let mut id = [0u32; 8];
    for (i, chunk) in bytes.chunks(4).enumerate() {
        let arr: [u8; 4] = chunk.try_into()?;
        if let Some(slot) = id.get_mut(i) {
            *slot = u32::from_le_bytes(arr);
        }
    }
    Ok(id)
}

/// Format a program id the way `parse_program_id` reads it.
#[must_use]
pub fn format_program_id(id: &ProgramId) -> String {
    let mut out = Vec::with_capacity(32);
    for w in id {
        out.extend_from_slice(&w.to_le_bytes());
    }
    hex::encode(out)
}
