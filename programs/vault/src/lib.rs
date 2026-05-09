use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// mppsol_cpi program (devnet) — invoked via CPI from `settle_payout_to_tempo`
/// to emit Receipt PDAs binding each cross-VM settlement to its Tempo origin.
pub const MPPSOL_CPI_PROGRAM: Pubkey = pubkey!("624xoctSeGzq1TAVwZU1xbM9RozAd3xZmjPeFXrAY14j");

#[program]
pub mod vault {
    use super::*;

    /// Initialize a vault PDA for a single merchant.
    pub fn initialize(
        ctx: Context<Initialize>,
        merchant_id: [u8; 32],
        kamino_market: Pubkey,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.merchant_id = merchant_id;
        vault.kamino_market = kamino_market;
        vault.usdc_mint = ctx.accounts.usdc_mint.key();
        vault.total_deposits = 0;
        vault.bump = ctx.bumps.vault;
        Ok(())
    }

    /// Receive a cross-VM intent from CCIP.
    ///
    /// Per Chainlink CCIP's Solana receiver pattern, the first account MUST
    /// be the CCIP offramp's CPI signer PDA — CCIP enforces this on dispatch
    /// and the program MUST validate it on receipt to prevent forged calls.
    /// `intent_data` is the encoded `CrossVMIntent` payload.
    pub fn ccip_receive(
        ctx: Context<CcipReceive>,
        source_chain_selector: u64,
        sender: Vec<u8>,
        intent_data: Vec<u8>,
    ) -> Result<()> {
        // TODO(v0.2): validate the offramp CPI signer is the canonical CCIP
        //              offramp program for this chain. CCIP delivers tokens
        //              to vault_usdc_ata before this call — that transfer is
        //              already complete by the time we run.
        // TODO(v0.2): validate `sender` matches the configured Tempo Buffer
        //              address (stored in vault state).

        let intent = parse_intent(&intent_data)?;
        require!(
            intent.kind == IntentKind::DepositForYield,
            VaultError::WrongIntentKind
        );
        require!(
            intent.merchant == ctx.accounts.vault.merchant_id,
            VaultError::WrongMerchant
        );

        ctx.accounts.vault.total_deposits = ctx
            .accounts
            .vault
            .total_deposits
            .checked_add(intent.amount as u64)
            .ok_or(VaultError::Overflow)?;

        emit!(IntentReceived {
            source_chain: source_chain_selector,
            amount: intent.amount as u64,
            nonce: intent.nonce,
        });

        // TODO(v0.2): immediately call allocate_to_kamino in a follow-up
        //              instruction (or inline here) so deposited USDC starts
        //              earning yield in the same transaction.

        Ok(())
    }

    /// Allocate vault USDC into Kamino USDC market.
    ///
    /// TODO: real Kamino CPI. v0.1 scaffolding emits the event but does not
    ///        actually invoke Kamino's lend program. The Kamino IDL and
    ///        market account derivation need to be wired in once a real
    ///        Kamino market is selected for the deployment.
    pub fn allocate_to_kamino(_ctx: Context<KaminoOp>, amount: u64) -> Result<()> {
        emit!(KaminoAllocated { amount });
        Ok(())
    }

    /// Withdraw from Kamino in preparation for a pull-back to Tempo.
    pub fn withdraw_from_kamino(_ctx: Context<KaminoOp>, amount: u64) -> Result<()> {
        emit!(KaminoWithdrawn { amount });
        Ok(())
    }

    /// Settle a payout via mppsol_cpi.pay_with_receipt — emits a Receipt PDA
    /// proving the cross-VM settlement, then triggers a CCIP send back to
    /// the Tempo Buffer with the USDC + receipt reference.
    ///
    /// The mppsol_cpi::PayWithReceipt accounts struct expects:
    ///   payer_authority (Signer)         = vault PDA (signed via seeds)
    ///   payer_token_account              = vault_usdc_ata
    ///   recipient_token_account          = pending_payout_ata (intermediate)
    ///   mint                             = USDC mint
    ///   receipt (init PDA)               = mppsol_receipt, seeds: [RECEIPT_SEED, payer, nonce]
    ///   token_program, system_program, instructions_sysvar
    pub fn settle_payout_to_tempo(
        ctx: Context<SettlePayoutToTempo>,
        amount: u64,
        nonce: [u8; 32],
        expiry: i64,
    ) -> Result<()> {
        require!(
            ctx.accounts.mppsol_cpi_program.key() == MPPSOL_CPI_PROGRAM,
            VaultError::WrongMppsolProgram
        );

        // TODO(v0.2): build the CPI invocation to mppsol_cpi.pay_with_receipt.
        //              The vault PDA needs to sign the SPL transfer as
        //              payer_authority — use invoke_signed with vault seeds:
        //                  &[b"vault", vault.authority.as_ref(), &[vault.bump]]
        //              `args = PayArgs { amount, expiry, nonce, ... }` per
        //              mppsol_cpi's instruction shape (see
        //              github.com/mppsol/cpi for the exact PayArgs struct).
        //
        //              The Receipt PDA is created at:
        //                  seeds = [RECEIPT_SEED, vault_pda, nonce]
        //              and persists across CPIs and tx boundaries — keeper
        //              reads it to confirm the settlement landed before
        //              issuing the CCIP send-back.

        emit!(SettlementBound {
            amount,
            nonce,
            mppsol_receipt: ctx.accounts.mppsol_receipt.key(),
        });

        // TODO(v0.2): build CCIP message and call CCIP router program (also
        //              via CPI) to send the USDC + pull-back intent back to
        //              the Tempo Buffer. CCIP-on-Solana send-side is newer
        //              than the receive side; the right pattern needs
        //              verification against current chainlink-svm docs.
        emit!(PullbackInitiated {
            amount,
            nonce,
            destination_chain: 0, // TODO: store Tempo chain selector in vault state
        });

        Ok(())
    }
}

// ============================================================
// Account contexts
// ============================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", authority.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CcipReceive<'info> {
    /// CCIP offramp CPI signer PDA. Required first-account per CCIP spec —
    /// program must validate this matches the canonical offramp PDA before
    /// trusting any state in the call.
    /// CHECK: validated against the canonical CCIP offramp program ID
    /// (validation is a v0.2 TODO).
    pub offramp_cpi_signer: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    /// Vault's USDC token account. CCIP delivers bridged tokens here before
    /// invoking ccip_receive — the transfer is already complete on entry.
    #[account(mut, token::mint = usdc_mint, token::authority = vault)]
    pub vault_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct KaminoOp<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub vault_usdc_ata: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    // TODO(v0.2): Kamino market accounts (reserve, cToken mint, obligation, etc.)
}

#[derive(Accounts)]
#[instruction(amount: u64, nonce: [u8; 32])]
pub struct SettlePayoutToTempo<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    #[account(mut, token::mint = usdc_mint, token::authority = vault)]
    pub vault_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    /// Intermediate token account holding USDC pending CCIP send to Tempo.
    /// The vault PDA transfers here via mppsol_cpi.pay_with_receipt; the
    /// keeper then triggers the CCIP send-back.
    #[account(mut, token::mint = usdc_mint)]
    pub pending_payout_ata: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    /// mppsol Receipt PDA — created by mppsol_cpi.pay_with_receipt via CPI.
    /// PDA seeds (in mppsol_cpi): [RECEIPT_SEED, vault_pda, nonce]
    /// CHECK: created and validated by mppsol_cpi.
    #[account(mut)]
    pub mppsol_receipt: AccountInfo<'info>,

    /// CHECK: enforced by `address` constraint.
    #[account(address = MPPSOL_CPI_PROGRAM)]
    pub mppsol_cpi_program: AccountInfo<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar address — required by mppsol_cpi for Ed25519 verification.
    pub instructions_sysvar: AccountInfo<'info>,
}

// ============================================================
// State
// ============================================================

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
    pub merchant_id: [u8; 32],
    pub kamino_market: Pubkey,
    pub usdc_mint: Pubkey,
    pub total_deposits: u64,
    pub bump: u8,
}

// ============================================================
// CrossVMIntent — canonical 122-byte encoding shared with the Solidity
// side (contracts/buffer/src/CrossVMIntent.sol) and the TS side
// (packages/types/src/index.ts).
//
// Layout (v0.1, version byte = 0x01):
//
//   Offset  Size  Field
//   ──────────────────────────────────────────────────────────
//   0       1     version          (= 0x01)
//   1       1     kind             (0=DepositForYield, 1=PullbackForPayout, 2=ReceiptAck)
//   2       8     sourceChain      (BE u64 — CCIP chain selector)
//   10      16    amount           (BE u128 — USDC base units, 6 decimals)
//   26      32    sourceAddress
//   58      32    merchant
//   90      32    nonce
//   ──────────────────────────────────────────────────────────
//   Total: 122 bytes, big-endian, no padding.
//
// All three implementations are tested against the same hex vector — see
// `tests::CANONICAL_HEX_VECTOR` below and matching constants in the
// Solidity test and TS test files.
// ============================================================

pub const INTENT_VERSION: u8 = 0x01;
pub const INTENT_ENCODED_LENGTH: usize = 122;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossVMIntentPayload {
    pub source_chain: u64,
    pub amount: u128,
    pub source_address: [u8; 32],
    pub merchant: [u8; 32],
    pub nonce: [u8; 32],
    pub kind: IntentKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IntentKind {
    DepositForYield = 0,
    PullbackForPayout = 1,
    ReceiptAck = 2,
}

impl IntentKind {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(IntentKind::DepositForYield),
            1 => Some(IntentKind::PullbackForPayout),
            2 => Some(IntentKind::ReceiptAck),
            _ => None,
        }
    }
}

impl CrossVMIntentPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(INTENT_ENCODED_LENGTH);
        buf.push(INTENT_VERSION);
        buf.push(self.kind as u8);
        buf.extend_from_slice(&self.source_chain.to_be_bytes()); //  8
        buf.extend_from_slice(&self.amount.to_be_bytes()); // 16
        buf.extend_from_slice(&self.source_address); // 32
        buf.extend_from_slice(&self.merchant); // 32
        buf.extend_from_slice(&self.nonce); // 32
        debug_assert_eq!(buf.len(), INTENT_ENCODED_LENGTH);
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        require!(
            data.len() == INTENT_ENCODED_LENGTH,
            VaultError::InvalidIntent
        );
        require!(data[0] == INTENT_VERSION, VaultError::InvalidIntent);

        let kind = IntentKind::from_byte(data[1]).ok_or(VaultError::InvalidIntent)?;

        // Unwraps below are safe: we verified data.len() == 122 above, and the
        // slice ranges below total exactly 122 bytes (1+1+8+16+32+32+32).
        let source_chain = u64::from_be_bytes(data[2..10].try_into().unwrap());
        let amount = u128::from_be_bytes(data[10..26].try_into().unwrap());
        let source_address: [u8; 32] = data[26..58].try_into().unwrap();
        let merchant: [u8; 32] = data[58..90].try_into().unwrap();
        let nonce: [u8; 32] = data[90..122].try_into().unwrap();

        Ok(Self {
            source_chain,
            amount,
            source_address,
            merchant,
            nonce,
            kind,
        })
    }
}

fn parse_intent(data: &[u8]) -> Result<CrossVMIntentPayload> {
    CrossVMIntentPayload::decode(data)
}

// ============================================================
// Tests — runs with `cargo test` (not `anchor test`)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared cross-language test vector — the matching Solidity and TS
    /// implementations must encode/decode the same byte string. If any
    /// implementation diverges, the per-language test fails.
    const CANONICAL_HEX_VECTOR: &str = "010000000000000000010000000000000000000000003b9aca00000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeef000000000000000000000000beefbeefbeefbeefbeefbeefbeefbeefbeefbeeffeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed";

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn vector_intent() -> CrossVMIntentPayload {
        let mut beef_addr = [0u8; 32];
        beef_addr[12..].copy_from_slice(&hex_to_bytes("beefbeefbeefbeefbeefbeefbeefbeefbeefbeef"));
        let nonce: [u8; 32] = hex_to_bytes(
            "feedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed",
        )
        .try_into()
        .unwrap();
        CrossVMIntentPayload {
            source_chain: 1,
            amount: 1_000_000_000,
            source_address: beef_addr,
            merchant: beef_addr,
            nonce,
            kind: IntentKind::DepositForYield,
        }
    }

    #[test]
    fn encoded_length_is_122() {
        let encoded = vector_intent().encode();
        assert_eq!(encoded.len(), INTENT_ENCODED_LENGTH);
        assert_eq!(encoded.len(), 122);
    }

    #[test]
    fn encodes_to_canonical_vector() {
        let encoded = vector_intent().encode();
        let expected = hex_to_bytes(CANONICAL_HEX_VECTOR);
        assert_eq!(encoded, expected);
    }

    #[test]
    fn decodes_canonical_vector() {
        let bytes = hex_to_bytes(CANONICAL_HEX_VECTOR);
        let decoded = CrossVMIntentPayload::decode(&bytes).unwrap();
        assert_eq!(decoded, vector_intent());
    }

    #[test]
    fn round_trip() {
        let original = vector_intent();
        let encoded = original.encode();
        let decoded = CrossVMIntentPayload::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn rejects_wrong_length() {
        let bytes = vec![0x01, 0x00, 0x00];
        assert!(CrossVMIntentPayload::decode(&bytes).is_err());
    }

    #[test]
    fn rejects_wrong_version() {
        let mut bytes = vec![0u8; INTENT_ENCODED_LENGTH];
        bytes[0] = 0x99;
        assert!(CrossVMIntentPayload::decode(&bytes).is_err());
    }

    #[test]
    fn rejects_invalid_kind() {
        let mut bytes = vec![0u8; INTENT_ENCODED_LENGTH];
        bytes[0] = INTENT_VERSION;
        bytes[1] = 0x07;
        assert!(CrossVMIntentPayload::decode(&bytes).is_err());
    }
}

// ============================================================
// Events
// ============================================================

#[event]
pub struct IntentReceived {
    pub source_chain: u64,
    pub amount: u64,
    pub nonce: [u8; 32],
}

#[event]
pub struct KaminoAllocated {
    pub amount: u64,
}

#[event]
pub struct KaminoWithdrawn {
    pub amount: u64,
}

#[event]
pub struct SettlementBound {
    pub amount: u64,
    pub nonce: [u8; 32],
    pub mppsol_receipt: Pubkey,
}

#[event]
pub struct PullbackInitiated {
    pub amount: u64,
    pub nonce: [u8; 32],
    pub destination_chain: u64,
}

// ============================================================
// Errors
// ============================================================

#[error_code]
pub enum VaultError {
    #[msg("Cross-VM intent decoding failed")]
    InvalidIntent,
    #[msg("Intent kind does not match expected operation")]
    WrongIntentKind,
    #[msg("Intent merchant does not match vault merchant")]
    WrongMerchant,
    #[msg("mppsol_cpi program ID mismatch")]
    WrongMppsolProgram,
    #[msg("Arithmetic overflow")]
    Overflow,
}
