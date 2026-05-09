use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// mppsol_cpi program (devnet) — invoked via CPI from `settle_payout_to_tempo`
/// to emit Receipt PDAs binding each cross-VM settlement to its Tempo origin.
pub const MPPSOL_CPI_PROGRAM: Pubkey = pubkey!("624xoctSeGzq1TAVwZU1xbM9RozAd3xZmjPeFXrAY14j");

/// Chainlink CCIP router (Solana devnet). Set on Vault at initialize.
/// Mainnet ID will differ — verify against
/// https://docs.chain.link/ccip/directory before mainnet deployment.
pub const CCIP_ROUTER_DEVNET: Pubkey = pubkey!("Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C");

/// Solana devnet CCIP chain selector (per Chainlink directory).
pub const CCIP_SOLANA_DEVNET_CHAIN_SELECTOR: u64 = 16423721717087811551;

/// CCIP receiver pattern seeds — must mirror the constants used by the
/// Chainlink CCIP offramp + router programs, per the official receiver
/// pattern at github.com/smartcontractkit/chainlink-ccip
/// (chains/solana/contracts/programs/example-ccip-receiver).
pub const EXTERNAL_EXECUTION_CONFIG_SEED: &[u8] = b"external_execution_config";
pub const ALLOWED_OFFRAMP_SEED: &[u8] = b"allowed_offramp";

/// CCIP sender pattern seed — used by programs that originate CCIP
/// messages via CPI to the router. The sender PDA at [CCIP_SENDER_SEED]
/// derived under the caller program signs the ccip_send CPI.
pub const CCIP_SENDER_SEED: &[u8] = b"ccip_sender";

/// Anchor instruction discriminators on the Chainlink CCIP router
/// program. Verified against
/// github.com/smartcontractkit/chainlink-ccip/.../example-ccip-sender
/// at solana-v1.6.0. Re-derive at test time so a future router rename
/// is caught (see `tests::ccip_send_discriminator_matches_anchor_formula`).
pub const CCIP_SEND_DISCRIMINATOR: [u8; 8] = [108, 216, 134, 191, 249, 234, 33, 84];
pub const CCIP_GET_FEE_DISCRIMINATOR: [u8; 8] = [115, 195, 235, 161, 25, 219, 60, 29];

#[program]
pub mod vault {
    use super::*;

    /// Initialize a vault PDA for a single merchant.
    ///
    /// `expected_tempo_sender` is the configured Tempo Buffer.sol address
    /// (left-padded to 32 bytes — Solidity `bytes32(uint256(uint160(addr)))`).
    /// `ccip_router` is the Chainlink CCIP router program ID for this cluster
    /// (use `CCIP_ROUTER_DEVNET` for devnet).
    pub fn initialize(
        ctx: Context<Initialize>,
        merchant_id: [u8; 32],
        kamino_market: Pubkey,
        ccip_router: Pubkey,
        expected_tempo_sender: [u8; 32],
        expected_tempo_chain_selector: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.merchant_id = merchant_id;
        vault.kamino_market = kamino_market;
        vault.usdc_mint = ctx.accounts.usdc_mint.key();
        vault.total_deposits = 0;
        vault.bump = ctx.bumps.vault;
        vault.ccip_router = ccip_router;
        vault.expected_tempo_sender = expected_tempo_sender;
        vault.expected_tempo_chain_selector = expected_tempo_chain_selector;
        Ok(())
    }

    /// Receive a cross-VM intent from CCIP.
    ///
    /// Follows the canonical Chainlink CCIP receiver pattern (see
    /// github.com/smartcontractkit/chainlink-ccip/.../example-ccip-receiver).
    /// Three security checks happen at the account-constraint level before
    /// this body runs:
    ///
    ///   1. `authority` is a PDA at [EXTERNAL_EXECUTION_CONFIG_SEED, our_program_id]
    ///      derived under `offramp_program`. Only the offramp can produce a
    ///      signed CPI where this PDA signs.
    ///   2. `allowed_offramp` is a PDA at [ALLOWED_OFFRAMP, source_chain_le,
    ///      offramp_program] owned by `vault.ccip_router`. If the router
    ///      hasn't allowlisted that offramp, the account doesn't exist and
    ///      the constraint fails.
    ///   3. `vault` provides `vault.ccip_router` for (2)'s seeds::program.
    ///
    /// Then the body validates the source chain + sender match the
    /// configured Tempo Buffer.
    pub fn ccip_receive(ctx: Context<CcipReceive>, message: Any2SVMMessage) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        require!(
            message.source_chain_selector == vault.expected_tempo_chain_selector,
            VaultError::UnexpectedSourceChain
        );

        // EVM addresses arrive from CCIP as 20 raw bytes; we compare against
        // the left-padded 32-byte form stored in vault state (Solidity's
        // `bytes32(uint256(uint160(addr)))` convention). If CCIP changes the
        // padding behavior, adjust here.
        require!(
            sender_matches(&message.sender, &vault.expected_tempo_sender),
            VaultError::UnexpectedSender
        );

        let intent = parse_intent(&message.data)?;
        require!(
            intent.kind == IntentKind::DepositForYield,
            VaultError::WrongIntentKind
        );
        require!(
            intent.merchant == vault.merchant_id,
            VaultError::WrongMerchant
        );

        vault.total_deposits = vault
            .total_deposits
            .checked_add(intent.amount as u64)
            .ok_or(VaultError::Overflow)?;

        emit!(IntentReceived {
            source_chain: message.source_chain_selector,
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
    /// Vault PDA acts as `payer_authority` in mppsol_cpi.pay_with_receipt;
    /// the SPL transfer authority and the Receipt rent-payer are both the
    /// vault PDA. We sign the CPI with the vault's `[b"vault", authority,
    /// bump]` seeds via `invoke_signed`. The Receipt PDA is created at
    /// `[RECEIPT_SEED, vault_pda, nonce]` inside mppsol_cpi.
    ///
    /// IMPORTANT: vault PDA must have enough lamports to fund the Receipt
    /// PDA rent. The Vault account itself only carries its own rent, so
    /// before calling this instruction the keeper should top up the vault
    /// PDA via a plain SystemProgram::transfer. Future v0.2 work: split
    /// rent-payer from authority via an explicit `payer` Signer account
    /// (would require an mppsol_cpi instruction shape change).
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
        require!(amount > 0, VaultError::ZeroAmount);

        // Snapshot fields we need after consuming &mut for the CPI.
        let vault_authority = ctx.accounts.vault.authority;
        let vault_bump = ctx.accounts.vault.bump;

        // For v0.2 cross-VM binding we set request_hash = nonce as a
        // placeholder. v0.3 will set request_hash = sha256(intent_bytes)
        // so the on-chain Receipt cryptographically binds to the Tempo
        // intent that originated the payout.
        let args = mppsol_cpi_client::PayArgs {
            amount,
            nonce,
            request_hash: nonce,
            expiry,
        };

        let ix = mppsol_cpi_client::build_pay_with_receipt_ix(
            &mppsol_cpi_client::PayWithReceiptAccountKeys {
                payer_authority: ctx.accounts.vault.key(),
                payer_token_account: ctx.accounts.vault_usdc_ata.key(),
                recipient_token_account: ctx.accounts.pending_payout_ata.key(),
                mint: ctx.accounts.usdc_mint.key(),
                receipt: ctx.accounts.mppsol_receipt.key(),
                token_program: ctx.accounts.token_program.key(),
                system_program: ctx.accounts.system_program.key(),
                instructions_sysvar: ctx.accounts.instructions_sysvar.key(),
            },
            &args,
        );

        // Account infos must include every account referenced by the
        // instruction PLUS the program account itself.
        let account_infos = [
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.vault_usdc_ata.to_account_info(),
            ctx.accounts.pending_payout_ata.to_account_info(),
            ctx.accounts.usdc_mint.to_account_info(),
            ctx.accounts.mppsol_receipt.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.instructions_sysvar.to_account_info(),
            ctx.accounts.mppsol_cpi_program.to_account_info(),
        ];

        let vault_signer_seeds: &[&[u8]] =
            &[b"vault", vault_authority.as_ref(), &[vault_bump]];

        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &account_infos,
            &[vault_signer_seeds],
        )?;

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
#[instruction(message: Any2SVMMessage)]
pub struct CcipReceive<'info> {
    // ── First 3 accounts mandated by Chainlink CCIP receiver pattern ──

    /// Offramp CPI signer PDA. Only the offramp can produce a CPI where
    /// this PDA signs (PDA derived under offramp_program with our crate ID
    /// as a seed). This is the security gate that proves the call came
    /// from the offramp.
    #[account(
        seeds = [EXTERNAL_EXECUTION_CONFIG_SEED, crate::ID.as_ref()],
        bump,
        seeds::program = offramp_program.key(),
    )]
    pub authority: Signer<'info>,

    /// CHECK: offramp program — used as seeds::program for `authority` and
    /// in the `allowed_offramp` PDA derivation. Not directly validated;
    /// security comes from `allowed_offramp` being owned by the router.
    pub offramp_program: UncheckedAccount<'info>,

    /// CHECK: PDA owned by `vault.ccip_router`, derived as
    /// [ALLOWED_OFFRAMP, source_chain_le, offramp_program]. If the router
    /// has not allowlisted this offramp for this source chain, the account
    /// doesn't exist and the `owner` constraint below fails.
    #[account(
        owner = vault.ccip_router @ VaultError::OfframpNotAllowed,
        seeds = [
            ALLOWED_OFFRAMP_SEED,
            message.source_chain_selector.to_le_bytes().as_ref(),
            offramp_program.key().as_ref(),
        ],
        bump,
        seeds::program = vault.ccip_router,
    )]
    pub allowed_offramp: UncheckedAccount<'info>,

    // ── Receiver-specific accounts ──

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

    /// Chainlink CCIP router for this cluster. Used as `seeds::program`
    /// for the `allowed_offramp` constraint in `CcipReceive` — i.e., the
    /// security check that the offramp delivering this message is
    /// allowlisted by this router.
    pub ccip_router: Pubkey,

    /// Configured Tempo Buffer.sol address (left-padded to 32 bytes per
    /// Solidity `bytes32(uint256(uint160(addr)))`). `ccip_receive`
    /// rejects messages whose decoded sender doesn't match.
    pub expected_tempo_sender: [u8; 32],

    /// CCIP chain selector for the configured Tempo source chain.
    pub expected_tempo_chain_selector: u64,
}

// ============================================================
// CCIP message types — local mirror of chainlink-ccip svm-v1.6 types.
//
// Defined here to avoid taking the chainlink-ccip crate as a Cargo dep
// (similar reasoning to mppsol_cpi_client). The Borsh layout MUST agree
// with what the Chainlink CCIP offramp actually serializes — verify
// against current chainlink-ccip svm sources before each release bump.
// ============================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SVMTokenAmount {
    pub token: Pubkey,
    pub amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Any2SVMMessage {
    pub message_id: [u8; 32],
    pub source_chain_selector: u64,
    /// Raw sender bytes from the source chain. For EVM sources, this is
    /// the 20-byte address (no padding).
    pub sender: Vec<u8>,
    /// Arbitrary payload — for soltempo, this is the canonical
    /// `CrossVMIntent` 122-byte encoding (see CrossVMIntentPayload).
    pub data: Vec<u8>,
    pub token_amounts: Vec<SVMTokenAmount>,
}

/// Compare a CCIP message sender (variable-length raw bytes) against the
/// 32-byte left-padded form stored in vault state (Solidity convention:
/// `bytes32(uint256(uint160(addr)))`).
///
/// Truth table:
/// - sender.len() == 20 (EVM raw): match against the last 20 bytes of the
///   stored 32-byte form.
/// - sender.len() == 32: match all 32 bytes directly.
/// - other lengths: reject.
fn sender_matches(sender: &[u8], expected_padded: &[u8; 32]) -> bool {
    match sender.len() {
        20 => sender == &expected_padded[12..32],
        32 => sender == expected_padded.as_slice(),
        _ => false,
    }
}

// ============================================================
// CCIP send-side types — Borsh mirrors of ccip-router types.
//
// IMPORTANT: send-side from a Solana program is substantially more
// complex than receive-side. ccip_send via CPI requires 18+ accounts,
// a separate get_fee CPI to quote fees, fee-token approval, and a
// dedicated `ccip_sender` PDA. See:
//   github.com/smartcontractkit/chainlink-ccip/.../example-ccip-sender
//   (solana-v1.6.0) for the canonical pattern.
//
// These types are defined here as a forward investment — they document
// what the send path will look like once implemented. The actual CPI
// invocation is deferred to a focused follow-up commit (see TODO #6 in
// README).
// ============================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SVM2AnyMessage {
    /// Destination address — for EVM destinations this is the 20-byte
    /// address left-padded to 32 bytes (`bytes32(uint256(uint160(addr)))`).
    pub receiver: Vec<u8>,
    /// Arbitrary payload — for soltempo, the canonical CrossVMIntent
    /// 122-byte encoding (PullbackForPayout kind).
    pub data: Vec<u8>,
    pub token_amounts: Vec<SVMTokenAmount>,
    pub fee_token: Pubkey,
    pub extra_args: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GetFeeResult {
    pub amount: u64,
    pub juels: u128,
    pub token: Pubkey,
}

// ============================================================
// Kamino klend client — verified constants + drift catchers.
//
// Same scope situation as the CCIP send-side: full integration is a
// multi-day sprint. klend's deposit handler requires zero-copy loaded
// Reserve, LendingMarket, Obligation, and UserMetadata accounts, plus
// oracle dependencies, refresh sequencing, and (for v2) farm logic.
// See:
//   github.com/Kamino-Finance/klend/programs/klend/src/handlers/
//     handler_deposit_reserve_liquidity_and_obligation_collateral.rs
//
// What this module ships today:
//   - Verified program IDs (mainnet + staging)
//   - Instruction discriminators for the supply-only flow soltempo needs
//   - Drift-catcher tests so a Kamino function rename fails loud
//   - InitObligationArgs Borsh mirror (the only small args struct)
//
// The actual CPI invocation is left to the dedicated Kamino integration
// sprint (TODO #4 in README). Discriminators are pre-staged so that
// sprint can hit the ground running.
// ============================================================

pub mod kamino_klend_client {
    use anchor_lang::prelude::*;

    /// Mainnet klend program. Use this once vault is on mainnet with real
    /// merchant funds.
    pub const PROGRAM_ID_MAINNET: Pubkey =
        pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

    /// Staging/test klend program — the closest thing to a devnet
    /// deployment. Verify against
    /// github.com/Kamino-Finance/klend/programs/klend/src/lib.rs before
    /// each deployment.
    pub const PROGRAM_ID_STAGING: Pubkey =
        pubkey!("SLendK7ySfcEzyaFqy93gDnD3RtrpXJcnRwb6zFHJSh");

    /// Anchor instruction discriminators on the klend program. All
    /// derived as `sha256("global:<name>")[..8]` — re-derived at test
    /// time by the drift-catcher tests. If a klend function is renamed
    /// upstream, the test fails loudly.
    pub const INIT_OBLIGATION_DISC: [u8; 8] =
        [251, 10, 231, 76, 27, 11, 159, 96];
    pub const DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISC: [u8; 8] =
        [216, 224, 191, 27, 204, 151, 102, 175];
    pub const WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2_DISC: [u8; 8] =
        [235, 52, 119, 152, 149, 197, 20, 7];
    pub const REFRESH_RESERVE_DISC: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
    pub const REFRESH_OBLIGATION_DISC: [u8; 8] = [33, 132, 147, 228, 151, 192, 72, 89];
    pub const INIT_USER_METADATA_DISC: [u8; 8] =
        [117, 169, 176, 69, 197, 23, 15, 162];

    /// Mirror of klend's InitObligationArgs (smallest args struct in the
    /// flow). Verify against state/types.rs in the klend repo before each
    /// release bump.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
    pub struct InitObligationArgs {
        pub tag: u8,
        pub id: u8,
    }
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

    // -- mppsol_cpi client tests -----------------------------------

    use crate::mppsol_cpi_client;
    use anchor_lang::solana_program::hash::hashv;

    #[test]
    fn pay_with_receipt_discriminator_matches_anchor_formula() {
        // Anchor instruction discriminator = sha256("global:<name>")[0..8].
        // Re-derive at test time so a future change to the upstream
        // function name is caught by this test rather than by silent
        // mainnet failures.
        let derived = hashv(&[b"global:pay_with_receipt"]).to_bytes();
        assert_eq!(
            &derived[..8],
            &mppsol_cpi_client::PAY_WITH_RECEIPT_DISC,
            "mppsol_cpi.pay_with_receipt discriminator drift detected"
        );
    }

    #[test]
    fn pay_args_serializes_to_80_bytes() {
        // amount(u64=8) + nonce([u8;32]=32) + request_hash([u8;32]=32) + expiry(i64=8) = 80
        let args = mppsol_cpi_client::PayArgs {
            amount: 1_000_000,
            nonce: [0xAA; 32],
            request_hash: [0xBB; 32],
            expiry: 1_700_000_000,
        };
        let mut buf = Vec::new();
        args.serialize(&mut buf).unwrap();
        assert_eq!(buf.len(), 80);
    }

    #[test]
    fn pay_args_round_trip() {
        use anchor_lang::AnchorDeserialize;
        let original = mppsol_cpi_client::PayArgs {
            amount: 12_345_678,
            nonce: [0x01; 32],
            request_hash: [0x02; 32],
            expiry: -1, // negative expiry for sanity (i64 sign bit)
        };
        let mut buf = Vec::new();
        original.serialize(&mut buf).unwrap();
        let decoded = mppsol_cpi_client::PayArgs::deserialize(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn pay_with_receipt_ix_account_order_and_signers() {
        let keys = mppsol_cpi_client::PayWithReceiptAccountKeys {
            payer_authority: Pubkey::new_unique(),
            payer_token_account: Pubkey::new_unique(),
            recipient_token_account: Pubkey::new_unique(),
            mint: Pubkey::new_unique(),
            receipt: Pubkey::new_unique(),
            token_program: Pubkey::new_unique(),
            system_program: Pubkey::new_unique(),
            instructions_sysvar: Pubkey::new_unique(),
        };
        let args = mppsol_cpi_client::PayArgs {
            amount: 1,
            nonce: [0; 32],
            request_hash: [0; 32],
            expiry: 0,
        };
        let ix = mppsol_cpi_client::build_pay_with_receipt_ix(&keys, &args);

        assert_eq!(ix.program_id, mppsol_cpi_client::PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 8);
        // discriminator + 80 bytes args
        assert_eq!(ix.data.len(), 88);
        assert_eq!(&ix.data[..8], &mppsol_cpi_client::PAY_WITH_RECEIPT_DISC);

        // payer_authority is the only signer; the receipt is mut but not signer.
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[0].is_writable);
        assert!(!ix.accounts[1].is_signer && ix.accounts[1].is_writable);
        assert!(!ix.accounts[2].is_signer && ix.accounts[2].is_writable);
        assert!(!ix.accounts[3].is_signer && !ix.accounts[3].is_writable); // mint readonly
        assert!(!ix.accounts[4].is_signer && ix.accounts[4].is_writable); // receipt mut
        assert!(!ix.accounts[5].is_signer && !ix.accounts[5].is_writable); // token program
        assert!(!ix.accounts[6].is_signer && !ix.accounts[6].is_writable); // system program
        assert!(!ix.accounts[7].is_signer && !ix.accounts[7].is_writable); // instructions sysvar
    }

    #[test]
    fn receipt_pda_derivation_is_deterministic() {
        let payer = Pubkey::new_unique();
        let nonce = [0xAB; 32];
        let (pda1, bump1) = mppsol_cpi_client::derive_receipt_pda(&payer, &nonce);
        let (pda2, bump2) = mppsol_cpi_client::derive_receipt_pda(&payer, &nonce);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    // -- CCIP receiver tests ---------------------------------------

    #[test]
    fn sender_matches_accepts_20_byte_evm_address() {
        // EVM address 0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef left-padded to 32 bytes.
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(&hex_to_bytes("beefbeefbeefbeefbeefbeefbeefbeefbeefbeef"));

        let raw_20 = hex_to_bytes("beefbeefbeefbeefbeefbeefbeefbeefbeefbeef");
        assert!(super::sender_matches(&raw_20, &padded));
    }

    #[test]
    fn sender_matches_accepts_32_byte_padded_form() {
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(&hex_to_bytes("beefbeefbeefbeefbeefbeefbeefbeefbeefbeef"));

        assert!(super::sender_matches(&padded, &padded));
    }

    #[test]
    fn sender_matches_rejects_wrong_address() {
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(&hex_to_bytes("beefbeefbeefbeefbeefbeefbeefbeefbeefbeef"));

        let attacker = hex_to_bytes("deaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        assert!(!super::sender_matches(&attacker, &padded));
    }

    #[test]
    fn sender_matches_rejects_invalid_length() {
        let padded = [0u8; 32];
        assert!(!super::sender_matches(&[0u8; 19], &padded));
        assert!(!super::sender_matches(&[0u8; 21], &padded));
        assert!(!super::sender_matches(&[0u8; 33], &padded));
        assert!(!super::sender_matches(&[], &padded));
    }

    #[test]
    fn external_execution_config_pda_derivation() {
        // The authority PDA derived under the offramp program with our crate
        // ID as a seed. CCIP's offramp must produce a CPI signed by this
        // exact PDA for our ccip_receive to accept the call.
        let offramp = Pubkey::new_unique();
        let (pda, _bump) = Pubkey::find_program_address(
            &[super::EXTERNAL_EXECUTION_CONFIG_SEED, super::ID.as_ref()],
            &offramp,
        );
        // Just verify it's deterministic and well-formed.
        assert_ne!(pda, Pubkey::default());
        let (pda2, _) = Pubkey::find_program_address(
            &[super::EXTERNAL_EXECUTION_CONFIG_SEED, super::ID.as_ref()],
            &offramp,
        );
        assert_eq!(pda, pda2);
    }

    #[test]
    fn allowed_offramp_pda_derivation_matches_canonical_pattern() {
        // The allowlist PDA: [ALLOWED_OFFRAMP, source_chain_le, offramp_program]
        // derived under the router. Owner must equal the router program ID.
        let router = super::CCIP_ROUTER_DEVNET;
        let offramp = Pubkey::new_unique();
        let source_chain: u64 = 16015286601757825753; // Sepolia selector example
        let (pda, _) = Pubkey::find_program_address(
            &[
                super::ALLOWED_OFFRAMP_SEED,
                source_chain.to_le_bytes().as_ref(),
                offramp.as_ref(),
            ],
            &router,
        );
        assert_ne!(pda, Pubkey::default());
    }

    // -- CCIP send-side discriminator drift catchers ---------------

    #[test]
    fn ccip_send_discriminator_matches_anchor_formula() {
        let derived = hashv(&[b"global:ccip_send"]).to_bytes();
        assert_eq!(
            &derived[..8],
            &super::CCIP_SEND_DISCRIMINATOR,
            "CCIP router ccip_send discriminator drift detected — verify against current chainlink-ccip svm release"
        );
    }

    #[test]
    fn ccip_get_fee_discriminator_matches_anchor_formula() {
        let derived = hashv(&[b"global:get_fee"]).to_bytes();
        assert_eq!(
            &derived[..8],
            &super::CCIP_GET_FEE_DISCRIMINATOR,
            "CCIP router get_fee discriminator drift detected"
        );
    }

    #[test]
    fn ccip_sender_pda_derives_under_caller_program() {
        // Send-side PDA: [CCIP_SENDER_SEED] derived under the calling
        // program. This PDA signs the ccip_send CPI via invoke_signed.
        let (pda1, bump1) =
            Pubkey::find_program_address(&[super::CCIP_SENDER_SEED], &super::ID);
        let (pda2, bump2) =
            Pubkey::find_program_address(&[super::CCIP_SENDER_SEED], &super::ID);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    // -- Kamino klend discriminator drift catchers ----------------

    use crate::kamino_klend_client;

    #[test]
    fn klend_init_obligation_discriminator_matches_anchor_formula() {
        let derived = hashv(&[b"global:init_obligation"]).to_bytes();
        assert_eq!(
            &derived[..8],
            &kamino_klend_client::INIT_OBLIGATION_DISC,
            "klend init_obligation discriminator drift detected"
        );
    }

    #[test]
    fn klend_deposit_v2_discriminator_matches_anchor_formula() {
        let derived = hashv(&[
            b"global:deposit_reserve_liquidity_and_obligation_collateral_v2",
        ])
        .to_bytes();
        assert_eq!(
            &derived[..8],
            &kamino_klend_client::DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISC,
            "klend deposit_v2 discriminator drift detected"
        );
    }

    #[test]
    fn klend_withdraw_v2_discriminator_matches_anchor_formula() {
        let derived = hashv(&[
            b"global:withdraw_obligation_collateral_and_redeem_reserve_collateral_v2",
        ])
        .to_bytes();
        assert_eq!(
            &derived[..8],
            &kamino_klend_client::WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2_DISC,
            "klend withdraw_v2 discriminator drift detected"
        );
    }

    #[test]
    fn klend_refresh_discriminators_match_anchor_formula() {
        let reserve = hashv(&[b"global:refresh_reserve"]).to_bytes();
        assert_eq!(
            &reserve[..8],
            &kamino_klend_client::REFRESH_RESERVE_DISC,
            "klend refresh_reserve discriminator drift detected"
        );
        let obligation = hashv(&[b"global:refresh_obligation"]).to_bytes();
        assert_eq!(
            &obligation[..8],
            &kamino_klend_client::REFRESH_OBLIGATION_DISC,
            "klend refresh_obligation discriminator drift detected"
        );
    }

    #[test]
    fn klend_init_user_metadata_discriminator_matches_anchor_formula() {
        let derived = hashv(&[b"global:init_user_metadata"]).to_bytes();
        assert_eq!(
            &derived[..8],
            &kamino_klend_client::INIT_USER_METADATA_DISC,
            "klend init_user_metadata discriminator drift detected"
        );
    }

    #[test]
    fn klend_init_obligation_args_serializes_to_2_bytes() {
        let args = kamino_klend_client::InitObligationArgs { tag: 0, id: 0 };
        let mut buf = Vec::new();
        args.serialize(&mut buf).unwrap();
        // tag(u8) + id(u8) = 2 bytes
        assert_eq!(buf.len(), 2);
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
    #[msg("Settlement amount must be > 0")]
    ZeroAmount,
    #[msg("CCIP offramp not allowlisted by configured router")]
    OfframpNotAllowed,
    #[msg("Message source chain does not match configured Tempo chain")]
    UnexpectedSourceChain,
    #[msg("Message sender does not match configured Tempo Buffer")]
    UnexpectedSender,
}

// ============================================================
// mppsol_cpi client — manual CPI without taking mppsol_cpi as a Cargo dep
//
// Why manual: mppsol_cpi lives in a separate repo (github.com/mppsol/cpi)
// with its own Anchor workspace structure. Cargo git deps don't resolve
// workspace members cleanly; a path dep would only work for local
// development. Building the instruction by hand keeps soltempo
// independently cloneable while still calling the real on-chain
// program. The instruction discriminator is verified against the Anchor
// formula by the test in `tests::pay_with_receipt_discriminator_matches`.
// ============================================================

pub mod mppsol_cpi_client {
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};

    /// mppsol_cpi program ID on Solana devnet (mirrors the parent
    /// MPPSOL_CPI_PROGRAM constant; declared here so this module is
    /// self-contained).
    pub const PROGRAM_ID: Pubkey = pubkey!("624xoctSeGzq1TAVwZU1xbM9RozAd3xZmjPeFXrAY14j");

    /// Receipt PDA seed prefix — must mirror mppsol_cpi::RECEIPT_SEED.
    pub const RECEIPT_SEED: &[u8] = b"receipt";

    /// Anchor instruction discriminator for `pay_with_receipt`.
    /// Computed as `sha256("global:pay_with_receipt")[0..8]`.
    /// Verified by `super::tests::pay_with_receipt_discriminator_matches`.
    pub const PAY_WITH_RECEIPT_DISC: [u8; 8] =
        [45, 221, 79, 34, 209, 140, 222, 126];

    /// Mirror of mppsol_cpi::PayArgs — must agree on Borsh layout.
    /// Layout (Anchor/Borsh): amount(u64 LE) | nonce([u8;32]) |
    /// request_hash([u8;32]) | expiry(i64 LE) = 80 bytes.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
    pub struct PayArgs {
        pub amount: u64,
        pub nonce: [u8; 32],
        pub request_hash: [u8; 32],
        pub expiry: i64,
    }

    /// All account keys required by mppsol_cpi.pay_with_receipt, in
    /// instruction order. Wrapping in a struct keeps the call site
    /// readable.
    pub struct PayWithReceiptAccountKeys {
        pub payer_authority: Pubkey,
        pub payer_token_account: Pubkey,
        pub recipient_token_account: Pubkey,
        pub mint: Pubkey,
        pub receipt: Pubkey,
        pub token_program: Pubkey,
        pub system_program: Pubkey,
        pub instructions_sysvar: Pubkey,
    }

    /// Build the Instruction for mppsol_cpi.pay_with_receipt.
    pub fn build_pay_with_receipt_ix(
        keys: &PayWithReceiptAccountKeys,
        args: &PayArgs,
    ) -> Instruction {
        let mut data = Vec::with_capacity(8 + 80);
        data.extend_from_slice(&PAY_WITH_RECEIPT_DISC);
        args.serialize(&mut data).unwrap();

        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(keys.payer_authority, true), // signer + mut
                AccountMeta::new(keys.payer_token_account, false), // mut
                AccountMeta::new(keys.recipient_token_account, false), // mut
                AccountMeta::new_readonly(keys.mint, false),
                AccountMeta::new(keys.receipt, false), // mut (init)
                AccountMeta::new_readonly(keys.token_program, false),
                AccountMeta::new_readonly(keys.system_program, false),
                AccountMeta::new_readonly(keys.instructions_sysvar, false),
            ],
            data,
        }
    }

    /// Derive the Receipt PDA address mppsol_cpi will create at
    /// `[RECEIPT_SEED, payer_authority, nonce]`.
    pub fn derive_receipt_pda(payer_authority: &Pubkey, nonce: &[u8; 32]) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[RECEIPT_SEED, payer_authority.as_ref(), nonce],
            &PROGRAM_ID,
        )
    }
}
