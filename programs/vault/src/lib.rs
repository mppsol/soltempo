use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3");

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
    /// `trusted_keeper` is the off-chain keeper allowed to call
    /// `trusted_keeper_receive` for the v0.2 demo path (see that
    /// instruction's docs). Set to `Pubkey::default()` to disable the
    /// trusted-keeper path entirely.
    pub fn initialize(
        ctx: Context<Initialize>,
        merchant_id: [u8; 32],
        kamino_market: Pubkey,
        ccip_router: Pubkey,
        expected_tempo_sender: [u8; 32],
        expected_tempo_chain_selector: u64,
        trusted_keeper: Pubkey,
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
        vault.trusted_keeper = trusted_keeper;
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

    /// V0.2 demo path: receive a cross-VM intent from a trusted off-chain
    /// keeper instead of from Chainlink CCIP. Same business logic as
    /// `ccip_receive` (source chain check, sender match, intent decode,
    /// deposit tracking) but a simpler signer model: the configured
    /// `trusted_keeper` signs the call directly.
    ///
    /// Used because Chainlink CCIP is not yet deployed on Tempo testnet
    /// (Andantino decommissioned, Moderato not yet on CCIP). When
    /// CCIP-on-Tempo ships, the keeper switches to calling `ccip_receive`
    /// (no vault redeploy needed) and the trusted-keeper path can be
    /// disabled by setting `vault.trusted_keeper = Pubkey::default()`.
    ///
    /// SPL token transfer of the bridged USDC happens out-of-band: the
    /// keeper transfers from its Solana-side USDC inventory to
    /// `vault_usdc_ata` BEFORE invoking this instruction.
    pub fn trusted_keeper_receive(
        ctx: Context<TrustedKeeperReceive>,
        message: Any2SVMMessage,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        require!(
            vault.trusted_keeper != Pubkey::default(),
            VaultError::TrustedKeeperPathDisabled
        );
        require!(
            ctx.accounts.trusted_keeper.key() == vault.trusted_keeper,
            VaultError::NotTrustedKeeper
        );

        require!(
            message.source_chain_selector == vault.expected_tempo_chain_selector,
            VaultError::UnexpectedSourceChain
        );
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

        Ok(())
    }

    /// Deposit vault USDC into a Kamino lending reserve via CPI.
    ///
    /// This is the production-shaped Kamino integration. The vault PDA
    /// acts as `obligation owner` (signer via invoke_signed) and as the
    /// authority on `user_source_liquidity` (the vault's USDC ATA).
    ///
    /// Caller responsibilities (klend requirements that we DON'T re-check
    /// because klend will reject a malformed setup at execution):
    ///
    ///   1. The transaction MUST include `klend.refresh_reserve(reserve)`
    ///      and `klend.refresh_obligation(obligation, [reserves])`
    ///      *before* this instruction. v2 doesn't enforce this at the
    ///      ix level, but LTV/borrow-cap checks inside klend assume
    ///      fresh interest accruals. Off-chain caller (keeper) builds
    ///      the multi-ix tx.
    ///   2. The obligation must already be initialized for the vault PDA
    ///      via `init_kamino_obligation`. UserMetadata likewise.
    ///   3. The Kamino market chosen (`vault.kamino_market`) must have
    ///      a USDC reserve. The vault enforces by passing the lending
    ///      market account; klend validates `reserve.lending_market ==
    ///      lending_market`.
    ///
    /// `amount` is the USDC liquidity amount in base units (6 decimals).
    pub fn deposit_to_kamino(
        ctx: Context<KaminoDepositV2>,
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, VaultError::ZeroAmount);

        // Snapshot vault PDA seeds for the invoke_signed below.
        let vault_authority = ctx.accounts.vault.authority;
        let vault_bump = ctx.accounts.vault.bump;

        let keys = kamino_klend_client::DepositV2AccountKeys {
            owner: ctx.accounts.vault.key(),
            obligation: ctx.accounts.obligation.key(),
            lending_market: ctx.accounts.lending_market.key(),
            lending_market_authority: ctx.accounts.lending_market_authority.key(),
            reserve: ctx.accounts.reserve.key(),
            reserve_liquidity_mint: ctx.accounts.reserve_liquidity_mint.key(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.key(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.key(),
            reserve_destination_deposit_collateral: ctx
                .accounts
                .reserve_destination_deposit_collateral
                .key(),
            user_source_liquidity: ctx.accounts.vault_usdc_ata.key(),
            placeholder_user_destination_collateral: ctx
                .accounts
                .placeholder_user_destination_collateral
                .key(),
            collateral_token_program: ctx.accounts.collateral_token_program.key(),
            liquidity_token_program: ctx.accounts.liquidity_token_program.key(),
            instruction_sysvar_account: ctx.accounts.instructions_sysvar.key(),
            obligation_farm_user_state: ctx.accounts.obligation_farm_user_state.key(),
            reserve_farm_state: ctx.accounts.reserve_farm_state.key(),
            farms_program: ctx.accounts.farms_program.key(),
        };

        let ix = kamino_klend_client::build_deposit_v2_ix(
            &ctx.accounts.klend_program.key(),
            &keys,
            amount,
        );

        let account_infos = [
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.obligation.to_account_info(),
            ctx.accounts.lending_market.to_account_info(),
            ctx.accounts.lending_market_authority.to_account_info(),
            ctx.accounts.reserve.to_account_info(),
            ctx.accounts.reserve_liquidity_mint.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts
                .reserve_destination_deposit_collateral
                .to_account_info(),
            ctx.accounts.vault_usdc_ata.to_account_info(),
            ctx.accounts
                .placeholder_user_destination_collateral
                .to_account_info(),
            ctx.accounts.collateral_token_program.to_account_info(),
            ctx.accounts.liquidity_token_program.to_account_info(),
            ctx.accounts.instructions_sysvar.to_account_info(),
            ctx.accounts.obligation_farm_user_state.to_account_info(),
            ctx.accounts.reserve_farm_state.to_account_info(),
            ctx.accounts.farms_program.to_account_info(),
            ctx.accounts.klend_program.to_account_info(),
        ];

        let vault_signer_seeds: &[&[u8]] =
            &[b"vault", vault_authority.as_ref(), &[vault_bump]];

        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &account_infos,
            &[vault_signer_seeds],
        )?;

        emit!(KaminoAllocated { amount });
        Ok(())
    }

    /// Initialize klend obligation + UserMetadata for the vault PDA.
    ///
    /// Two CPIs in sequence:
    ///   1. klend.init_user_metadata(vault, lookup_table=default)
    ///   2. klend.init_obligation(tag=0, id=0)
    ///
    /// The vault PDA is the obligation owner; it signs both via
    /// invoke_signed using `[b"vault", authority, bump]`. The fee_payer
    /// is the off-chain authority (the merchant or keeper) — they pay
    /// rent for the new accounts.
    ///
    /// Idempotency: klend's init handlers fail if the account already
    /// exists. Callers should check `obligation.is_initialized()` off-
    /// chain and skip this instruction on subsequent runs. We don't
    /// re-implement the existence check here to keep the ix focused.
    pub fn init_kamino_obligation(ctx: Context<InitKaminoObligation>) -> Result<()> {
        let vault_authority = ctx.accounts.vault.authority;
        let vault_bump = ctx.accounts.vault.bump;
        let vault_signer_seeds: &[&[u8]] =
            &[b"vault", vault_authority.as_ref(), &[vault_bump]];

        // 1. init_user_metadata
        let meta_keys = kamino_klend_client::InitUserMetadataAccountKeys {
            owner: ctx.accounts.vault.key(),
            fee_payer: ctx.accounts.fee_payer.key(),
            user_metadata: ctx.accounts.user_metadata.key(),
            referrer_user_metadata: ctx.accounts.system_program.key(), // None marker
            rent: ctx.accounts.rent.key(),
            system_program: ctx.accounts.system_program.key(),
        };
        let meta_ix = kamino_klend_client::build_init_user_metadata_ix(
            &ctx.accounts.klend_program.key(),
            &meta_keys,
            &Pubkey::default(), // no lookup table
        );
        let meta_account_infos = [
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.fee_payer.to_account_info(),
            ctx.accounts.user_metadata.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.klend_program.to_account_info(),
        ];
        anchor_lang::solana_program::program::invoke_signed(
            &meta_ix,
            &meta_account_infos,
            &[vault_signer_seeds],
        )?;

        // 2. init_obligation
        let oblig_keys = kamino_klend_client::InitObligationAccountKeys {
            obligation_owner: ctx.accounts.vault.key(),
            fee_payer: ctx.accounts.fee_payer.key(),
            obligation: ctx.accounts.obligation.key(),
            lending_market: ctx.accounts.lending_market.key(),
            seed1_account: ctx.accounts.system_program.key(),
            seed2_account: ctx.accounts.system_program.key(),
            owner_user_metadata: ctx.accounts.user_metadata.key(),
            rent: ctx.accounts.rent.key(),
            system_program: ctx.accounts.system_program.key(),
        };
        let oblig_ix = kamino_klend_client::build_init_obligation_ix(
            &ctx.accounts.klend_program.key(),
            &oblig_keys,
            &kamino_klend_client::InitObligationArgs { tag: 0, id: 0 },
        );
        let oblig_account_infos = [
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.fee_payer.to_account_info(),
            ctx.accounts.obligation.to_account_info(),
            ctx.accounts.lending_market.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.user_metadata.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.klend_program.to_account_info(),
        ];
        anchor_lang::solana_program::program::invoke_signed(
            &oblig_ix,
            &oblig_account_infos,
            &[vault_signer_seeds],
        )?;

        emit!(KaminoObligationInitialized {
            obligation: ctx.accounts.obligation.key(),
            user_metadata: ctx.accounts.user_metadata.key(),
        });
        Ok(())
    }

    /// Withdraw from Kamino in preparation for a pull-back to Tempo.
    ///
    /// Kept as a stub for now. The withdraw_v2 ix mirrors deposit_v2 in
    /// account shape (with cToken redemption replacing collateral
    /// deposit). Wiring it follows the same pattern as
    /// `deposit_to_kamino` — defer until pull-back path is online.
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

        // Build the canonical CrossVMIntent payload for the pull-back.
        // The keeper picks this up off-chain, calls
        // router.derive_accounts_ccip_send to discover LUTs, and submits
        // the ccip_send tx. See PullbackRequested docs.
        let pullback_intent = CrossVMIntentPayload {
            source_chain: CCIP_SOLANA_DEVNET_CHAIN_SELECTOR,
            amount: amount as u128,
            source_address: ctx.accounts.vault.key().to_bytes(),
            merchant: ctx.accounts.vault.merchant_id,
            nonce,
            kind: IntentKind::PullbackForPayout,
        };
        let intent_bytes = pullback_intent.encode();

        emit!(PullbackRequested {
            amount,
            nonce,
            destination_chain: ctx.accounts.vault.expected_tempo_chain_selector,
            receiver: ctx.accounts.vault.expected_tempo_sender,
            intent_bytes,
            usdc_mint: ctx.accounts.usdc_mint.key(),
            // Suggested EVM destination gas limit — keeper-overridable.
            // Pull-back receive on Tempo Buffer is a single _ccipReceive
            // call that updates state + emits an event; 200k is generous.
            suggested_gas_limit: 200_000,
        });

        Ok(())
    }

    /// Trusted-keeper pull-back: emit a structured request the keeper
    /// consumes off-chain to perform the EVM-side settlement on Tempo.
    ///
    /// Symmetric to `trusted_keeper_receive`. While CCIP-on-Tempo is
    /// pending, the off-chain keeper bridges Solana → Tempo by:
    ///   1. Reading this event
    ///   2. Burning/escrowing USDC on Solana (vault holds it pending)
    ///   3. Releasing equivalent USDC into Buffer.sol on Tempo
    ///
    /// When CCIP ships on Tempo, callers switch to `settle_payout_to_tempo`
    /// (which already emits `PullbackRequested` after Receipt binding) +
    /// keeper invokes ccip_send via the official derive_accounts flow.
    /// No vault redeploy required.
    pub fn request_pullback_to_tempo(
        ctx: Context<RequestPullback>,
        amount: u64,
        nonce: [u8; 32],
    ) -> Result<()> {
        require!(amount > 0, VaultError::ZeroAmount);
        require!(
            ctx.accounts.vault.trusted_keeper != Pubkey::default(),
            VaultError::TrustedKeeperPathDisabled
        );
        // The merchant authority signs pull-back requests; the keeper is
        // only the executor (it doesn't get to choose when to pull back).
        require!(
            ctx.accounts.authority.key() == ctx.accounts.vault.authority,
            VaultError::WrongAuthority
        );

        let pullback_intent = CrossVMIntentPayload {
            source_chain: CCIP_SOLANA_DEVNET_CHAIN_SELECTOR,
            amount: amount as u128,
            source_address: ctx.accounts.vault.key().to_bytes(),
            merchant: ctx.accounts.vault.merchant_id,
            nonce,
            kind: IntentKind::PullbackForPayout,
        };
        let intent_bytes = pullback_intent.encode();

        emit!(PullbackRequested {
            amount,
            nonce,
            destination_chain: ctx.accounts.vault.expected_tempo_chain_selector,
            receiver: ctx.accounts.vault.expected_tempo_sender,
            intent_bytes,
            usdc_mint: ctx.accounts.usdc_mint.key(),
            suggested_gas_limit: 200_000,
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
#[instruction(message: Any2SVMMessage)]
pub struct TrustedKeeperReceive<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    /// Vault's USDC token account. The keeper must transfer the bridged
    /// USDC here BEFORE calling this instruction. We don't enforce the
    /// transfer atomically; the keeper is trusted to do the right thing
    /// (which is the whole point of the trusted-keeper path).
    #[account(token::mint = usdc_mint, token::authority = vault)]
    pub vault_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    /// The configured off-chain keeper. Must match `vault.trusted_keeper`
    /// (set at initialize). The vault rejects the call otherwise.
    pub trusted_keeper: Signer<'info>,
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
}

/// Account context for `deposit_to_kamino` — mirrors klend's
/// `DepositReserveLiquidityAndObligationCollateralV2` account list.
///
/// All Kamino accounts are passed as `UncheckedAccount` because they're
/// validated by klend at execution. The vault's only on-chain checks
/// here are that:
///   - vault PDA matches the configured authority
///   - vault_usdc_ata is the vault's USDC ATA
///   - klend_program is the configured Kamino program
///
/// Everything else (reserve belongs to lending_market, obligation
/// belongs to vault, etc.) is enforced by klend's own constraints.
#[derive(Accounts)]
pub struct KaminoDepositV2<'info> {
    #[account(
        mut,
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    /// Vault's USDC token account — `user_source_liquidity` for klend.
    #[account(
        mut,
        token::mint = reserve_liquidity_mint,
        token::authority = vault,
    )]
    pub vault_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: klend obligation, owned by vault PDA. Validated by klend.
    #[account(mut)]
    pub obligation: AccountInfo<'info>,

    /// CHECK: klend LendingMarket. Must match `vault.kamino_market` —
    /// enforced by `address` constraint.
    #[account(address = vault.kamino_market)]
    pub lending_market: AccountInfo<'info>,

    /// CHECK: klend lending_market_authority PDA. Validated by klend.
    pub lending_market_authority: AccountInfo<'info>,

    /// CHECK: klend Reserve. Validated by klend (reserve.lending_market
    /// must equal lending_market).
    #[account(mut)]
    pub reserve: AccountInfo<'info>,

    pub reserve_liquidity_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Reserve's liquidity supply token account. Mut.
    #[account(mut)]
    pub reserve_liquidity_supply: AccountInfo<'info>,

    /// CHECK: Reserve's collateral mint. Mut.
    #[account(mut)]
    pub reserve_collateral_mint: AccountInfo<'info>,

    /// CHECK: Reserve's destination collateral account. Mut.
    #[account(mut)]
    pub reserve_destination_deposit_collateral: AccountInfo<'info>,

    /// CHECK: Legacy placeholder slot (klend treats this as None for v2).
    /// Pass system_program here.
    pub placeholder_user_destination_collateral: AccountInfo<'info>,

    /// CHECK: SPL Token v1 program — klend uses this for collateral.
    pub collateral_token_program: AccountInfo<'info>,

    /// CHECK: SPL Token or Token-2022 — must match the reserve's
    /// liquidity mint owner program.
    pub liquidity_token_program: AccountInfo<'info>,

    /// CHECK: sysvar::instructions::id() — klend reads tx context.
    pub instructions_sysvar: AccountInfo<'info>,

    /// CHECK: Optional farm user state. Pass system_program for None.
    #[account(mut)]
    pub obligation_farm_user_state: AccountInfo<'info>,

    /// CHECK: Optional reserve farm state. Pass system_program for None.
    #[account(mut)]
    pub reserve_farm_state: AccountInfo<'info>,

    /// CHECK: klend's farms program. Always required even for non-farm
    /// reserves — klend validates it equals `FARMS_PROGRAM_ID`.
    pub farms_program: AccountInfo<'info>,

    /// CHECK: klend program — mainnet or staging. Caller must pass the
    /// same program klend's accounts originate from.
    pub klend_program: AccountInfo<'info>,
}

/// Account context for `request_pullback_to_tempo`. Lightweight —
/// no token movement happens here, just an authoritative event for
/// the keeper to consume. Token movement comes via
/// `settle_payout_to_tempo` (which uses mppsol_cpi for receipt binding).
#[derive(Accounts)]
pub struct RequestPullback<'info> {
    #[account(
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    /// Merchant authority — must equal vault.authority.
    pub authority: Signer<'info>,
}

/// Account context for `init_kamino_obligation` — wraps klend's
/// init_user_metadata + init_obligation in a single instruction.
///
/// `fee_payer` (the merchant or keeper) pays rent for the new accounts.
/// The vault PDA is the obligation owner, signing both CPIs via
/// invoke_signed.
#[derive(Accounts)]
pub struct InitKaminoObligation<'info> {
    #[account(
        seeds = [b"vault", vault.authority.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: klend Obligation account to be created. Klend derives the
    /// PDA at [tag, id, owner, lending_market, seed1, seed2] under
    /// klend's program; the caller off-chain must match.
    #[account(mut)]
    pub obligation: AccountInfo<'info>,

    /// CHECK: klend UserMetadata PDA at [USER_METADATA_SEED, owner]
    /// under klend's program. Created by init_user_metadata CPI.
    #[account(mut)]
    pub user_metadata: AccountInfo<'info>,

    /// CHECK: LendingMarket — must match vault.kamino_market.
    #[account(address = vault.kamino_market)]
    pub lending_market: AccountInfo<'info>,

    /// CHECK: klend program (mainnet or staging).
    pub klend_program: AccountInfo<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
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

    /// The off-chain keeper allowed to call `trusted_keeper_receive`
    /// during the v0.2 demo path (when Chainlink CCIP isn't yet on
    /// Tempo testnet). Set to `Pubkey::default()` to disable that
    /// path entirely (production CCIP-only mode).
    pub trusted_keeper: Pubkey,
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
// IMPORTANT: send-side from a Solana program via direct CPI is
// substantially more complex than receive-side. The router's
// `CcipSend` requires 18 named accounts plus 13-account pool blocks
// per token bridged, plus per-token Address Lookup Tables that the
// router validates against on-chain. The recommended pattern is:
//
//   1. Off-chain client calls router.derive_accounts_ccip_send
//      (multi-stage) to get the full account list + LUTs.
//   2. Client builds the tx with the discovered accounts + LUTs.
//   3. Caller program optionally invokes ccip_send via CPI; many
//      integrations instead let the user wallet sign ccip_send
//      directly, with the caller program just emitting the
//      structured payload.
//
// soltempo's pull-back flow uses pattern (3): the vault emits a
// `PullbackRequested` event carrying the full SVM2AnyMessage; the
// keeper builds the multi-instruction transaction (derive_accounts
// → set up LUTs → ccip_send) off-chain. This mirrors the inbound
// trusted-keeper path and stays honest about what the on-chain
// program can verify without a CCIP-on-Tempo deployment to test
// against. When CCIP ships on Tempo + derive_accounts is stable,
// the vault gains an alternate `send_pullback_via_ccip` instruction
// that does the in-program CPI (see TODO at end of file).
//
// References (chainlink-ccip solana-v1.6.2):
//   chains/solana/contracts/programs/example-ccip-sender/src/lib.rs
//   chains/solana/contracts/programs/ccip-router/src/messages.rs
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

/// CCIP `GenericExtraArgsV2` encoding — required for EVM destinations
/// to specify a gas limit on the destination chain. Format per
/// chainlink-ccip ccip-router/src/messages.rs:
///
///   tag (4 bytes)            = 0x181dcf10  (GenericExtraArgsV2)
///   gasLimit (32 bytes BE)   = uint256
///   allowOOOExecution (1)    = bool
///
/// We expose a small builder so the caller doesn't have to remember
/// the magic tag. If chainlink rev's the format, update here +
/// extra_args_v2_encoding test below.
pub const GENERIC_EXTRA_ARGS_V2_TAG: [u8; 4] = [0x18, 0x1d, 0xcf, 0x10];

pub fn encode_generic_extra_args_v2(gas_limit: u64, allow_ooo: bool) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + 32 + 1);
    buf.extend_from_slice(&GENERIC_EXTRA_ARGS_V2_TAG);
    // gasLimit as uint256 BE — pad u64 with 24 zero bytes.
    buf.extend_from_slice(&[0u8; 24]);
    buf.extend_from_slice(&gas_limit.to_be_bytes());
    buf.push(if allow_ooo { 1 } else { 0 });
    buf
}

// ============================================================
// Kamino klend client — manual CPI without taking klend as a Cargo dep.
//
// Same justification as mppsol_cpi_client: klend lives in a separate
// repo with its own Anchor workspace, optional features (`staging`),
// and zero-copy types we don't actually need to mirror in full. We
// build the instruction by hand and let the on-chain klend program
// validate the accounts at execution time.
//
// Verified against klend master 3f7bd693 — see:
//   github.com/Kamino-Finance/klend/blob/3f7bd693/programs/klend/src/
//     handlers/handler_deposit_reserve_liquidity_and_obligation_collateral.rs
//
// Account ordering, mut/signer flags, and the `placeholder_user_destination_collateral`
// slot all mirror the upstream `DepositReserveLiquidityAndObligationCollateralV2`
// struct exactly. The drift-catcher tests below re-derive every Anchor
// discriminator from `sha256("global:<name>")[..8]`, so a future klend
// function rename trips a test rather than producing a silent mainnet
// failure.
// ============================================================

pub mod kamino_klend_client {
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};

    /// Mainnet klend program. Use this once vault is on mainnet with real
    /// merchant funds.
    pub const PROGRAM_ID_MAINNET: Pubkey =
        pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

    /// Staging/test klend program (deployed on mainnet under a separate ID
    /// when klend is built with `--features staging`). klend has NO
    /// devnet deployment — for local development against this CPI client,
    /// run a localnet validator with mainnet klend cloned via
    /// `solana-test-validator --clone KLend2g3...`. See the
    /// `scripts/localnet-with-klend.sh` helper.
    pub const PROGRAM_ID_STAGING: Pubkey =
        pubkey!("SLendK7ySfcEzyaFqy93gDnD3RtrpXJcnRwb6zFHJSh");

    /// Farms program ID — required for the v2 deposit ix even when no
    /// farm is active for the obligation/reserve. The farm accounts
    /// themselves are Optional but the program ID is mandatory.
    pub const FARMS_PROGRAM_ID: Pubkey =
        pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

    /// PDA seed for klend's `lending_market_authority`. The PDA is
    /// derived under klend's program ID as
    /// `[LENDING_MARKET_AUTH_SEED, lending_market]`.
    pub const LENDING_MARKET_AUTH_SEED: &[u8] = b"lma";

    /// PDA seed for klend's `UserMetadata` account, derived as
    /// `[USER_METADATA_SEED, owner]` under klend's program ID. Verify
    /// against state/user_metadata.rs in the klend repo.
    pub const USER_METADATA_SEED: &[u8] = b"user_meta";

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

    /// Account keys for `deposit_reserve_liquidity_and_obligation_collateral_v2`.
    /// Field order MUST match the order Anchor inlines in
    /// `DepositReserveLiquidityAndObligationCollateralV2`. See the
    /// upstream handler file referenced at the top of this module.
    pub struct DepositV2AccountKeys {
        // ── Inlined `deposit_accounts` (positions 1..=10) ──
        pub owner: Pubkey,                                     // signer + mut
        pub obligation: Pubkey,                                // mut, AccountLoader<Obligation>
        pub lending_market: Pubkey,                            // readonly, zero-copy
        pub lending_market_authority: Pubkey,                  // readonly, PDA
        pub reserve: Pubkey,                                   // mut, zero-copy
        pub reserve_liquidity_mint: Pubkey,                    // readonly
        pub reserve_liquidity_supply: Pubkey,                  // mut, TokenAccount
        pub reserve_collateral_mint: Pubkey,                   // mut, Mint
        pub reserve_destination_deposit_collateral: Pubkey,    // mut, TokenAccount
        pub user_source_liquidity: Pubkey,                     // mut, owner-authority
        // ── Position 11: legacy placeholder, pass system_program (treated as None by klend) ──
        pub placeholder_user_destination_collateral: Pubkey,
        // ── Token programs ──
        pub collateral_token_program: Pubkey,                  // SPL Token v1 only
        pub liquidity_token_program: Pubkey,                   // SPL Token or Token-2022
        pub instruction_sysvar_account: Pubkey,                // sysvar::instructions::id()
        // ── Farm accounts (Optional, but slots must exist) ──
        // For obligations/reserves with no farm, pass system_program as a
        // None marker per Anchor's Option<Account> on-wire convention.
        pub obligation_farm_user_state: Pubkey,
        pub reserve_farm_state: Pubkey,
        pub farms_program: Pubkey,                             // FARMS_PROGRAM_ID
    }

    /// Build the Instruction for klend's
    /// `deposit_reserve_liquidity_and_obligation_collateral_v2`.
    ///
    /// `program_id` is either `PROGRAM_ID_MAINNET` or `PROGRAM_ID_STAGING`.
    /// Caller must invoke via `invoke_signed` with the obligation owner's
    /// signer seeds (vault PDA seeds in the soltempo case).
    pub fn build_deposit_v2_ix(
        program_id: &Pubkey,
        keys: &DepositV2AccountKeys,
        liquidity_amount: u64,
    ) -> Instruction {
        let mut data = Vec::with_capacity(8 + 8);
        data.extend_from_slice(&DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISC);
        data.extend_from_slice(&liquidity_amount.to_le_bytes());

        Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(keys.owner, true),
                AccountMeta::new(keys.obligation, false),
                AccountMeta::new_readonly(keys.lending_market, false),
                AccountMeta::new_readonly(keys.lending_market_authority, false),
                AccountMeta::new(keys.reserve, false),
                AccountMeta::new_readonly(keys.reserve_liquidity_mint, false),
                AccountMeta::new(keys.reserve_liquidity_supply, false),
                AccountMeta::new(keys.reserve_collateral_mint, false),
                AccountMeta::new(keys.reserve_destination_deposit_collateral, false),
                AccountMeta::new(keys.user_source_liquidity, false),
                AccountMeta::new_readonly(keys.placeholder_user_destination_collateral, false),
                AccountMeta::new_readonly(keys.collateral_token_program, false),
                AccountMeta::new_readonly(keys.liquidity_token_program, false),
                AccountMeta::new_readonly(keys.instruction_sysvar_account, false),
                AccountMeta::new(keys.obligation_farm_user_state, false),
                AccountMeta::new(keys.reserve_farm_state, false),
                AccountMeta::new_readonly(keys.farms_program, false),
            ],
            data,
        }
    }

    /// Account keys for `init_obligation`. The obligation PDA is
    /// derived under klend as `[tag, id, owner, lending_market, seed1,
    /// seed2]` per state/obligation.rs.
    pub struct InitObligationAccountKeys {
        pub obligation_owner: Pubkey,        // signer
        pub fee_payer: Pubkey,               // signer + mut
        pub obligation: Pubkey,              // mut (created)
        pub lending_market: Pubkey,          // readonly
        pub seed1_account: Pubkey,           // typically system_program
        pub seed2_account: Pubkey,           // typically system_program
        pub owner_user_metadata: Pubkey,     // readonly
        pub rent: Pubkey,                    // sysvar::rent::id()
        pub system_program: Pubkey,
    }

    pub fn build_init_obligation_ix(
        program_id: &Pubkey,
        keys: &InitObligationAccountKeys,
        args: &InitObligationArgs,
    ) -> Instruction {
        let mut data = Vec::with_capacity(8 + 2);
        data.extend_from_slice(&INIT_OBLIGATION_DISC);
        args.serialize(&mut data).unwrap();

        Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new_readonly(keys.obligation_owner, true),
                AccountMeta::new(keys.fee_payer, true),
                AccountMeta::new(keys.obligation, false),
                AccountMeta::new_readonly(keys.lending_market, false),
                AccountMeta::new_readonly(keys.seed1_account, false),
                AccountMeta::new_readonly(keys.seed2_account, false),
                AccountMeta::new_readonly(keys.owner_user_metadata, false),
                AccountMeta::new_readonly(keys.rent, false),
                AccountMeta::new_readonly(keys.system_program, false),
            ],
            data,
        }
    }

    /// Account keys for `init_user_metadata`. UserMetadata PDA at
    /// `[USER_METADATA_SEED, owner]` under klend.
    pub struct InitUserMetadataAccountKeys {
        pub owner: Pubkey,                    // signer
        pub fee_payer: Pubkey,                // signer + mut
        pub user_metadata: Pubkey,            // mut (created)
        pub referrer_user_metadata: Pubkey,   // readonly, optional (pass system_program for None)
        pub rent: Pubkey,
        pub system_program: Pubkey,
    }

    /// Args for init_user_metadata: just the user_lookup_table pubkey
    /// (defaults to Pubkey::default() if no LUT).
    pub fn build_init_user_metadata_ix(
        program_id: &Pubkey,
        keys: &InitUserMetadataAccountKeys,
        user_lookup_table: &Pubkey,
    ) -> Instruction {
        let mut data = Vec::with_capacity(8 + 32);
        data.extend_from_slice(&INIT_USER_METADATA_DISC);
        data.extend_from_slice(user_lookup_table.as_ref());

        Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new_readonly(keys.owner, true),
                AccountMeta::new(keys.fee_payer, true),
                AccountMeta::new(keys.user_metadata, false),
                AccountMeta::new_readonly(keys.referrer_user_metadata, false),
                AccountMeta::new_readonly(keys.rent, false),
                AccountMeta::new_readonly(keys.system_program, false),
            ],
            data,
        }
    }

    /// Derive klend's `lending_market_authority` PDA for a given market.
    pub fn derive_lending_market_authority(
        program_id: &Pubkey,
        lending_market: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[LENDING_MARKET_AUTH_SEED, lending_market.as_ref()],
            program_id,
        )
    }

    /// Derive klend's `UserMetadata` PDA for an owner.
    pub fn derive_user_metadata_pda(
        program_id: &Pubkey,
        owner: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[USER_METADATA_SEED, owner.as_ref()],
            program_id,
        )
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
    use sha2::{Digest, Sha256};

    /// Compute sha256 of a single input — re-derives the Anchor
    /// instruction discriminator formula. Returns 32 bytes; tests
    /// compare the first 8.
    fn sha256(input: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(input);
        h.finalize().into()
    }
    fn hashv_helper(parts: &[&[u8]]) -> [u8; 32] {
        let mut h = Sha256::new();
        for p in parts {
            h.update(p);
        }
        h.finalize().into()
    }
    // Shadow the old `hashv(&[..]).to_bytes()` style with one that returns [u8;32].
    struct HashWrap([u8; 32]);
    impl HashWrap {
        fn to_bytes(self) -> [u8; 32] {
            self.0
        }
    }
    fn hashv(parts: &[&[u8]]) -> HashWrap {
        HashWrap(hashv_helper(parts))
    }
    #[allow(dead_code)]
    fn _use_sha256() -> [u8; 32] {
        sha256(b"unused")
    }

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

    // -- klend deposit_v2 ix builder tests --------------------------

    fn dummy_deposit_v2_keys() -> kamino_klend_client::DepositV2AccountKeys {
        kamino_klend_client::DepositV2AccountKeys {
            owner: Pubkey::new_unique(),
            obligation: Pubkey::new_unique(),
            lending_market: Pubkey::new_unique(),
            lending_market_authority: Pubkey::new_unique(),
            reserve: Pubkey::new_unique(),
            reserve_liquidity_mint: Pubkey::new_unique(),
            reserve_liquidity_supply: Pubkey::new_unique(),
            reserve_collateral_mint: Pubkey::new_unique(),
            reserve_destination_deposit_collateral: Pubkey::new_unique(),
            user_source_liquidity: Pubkey::new_unique(),
            placeholder_user_destination_collateral: Pubkey::new_unique(),
            collateral_token_program: Pubkey::new_unique(),
            liquidity_token_program: Pubkey::new_unique(),
            instruction_sysvar_account: Pubkey::new_unique(),
            obligation_farm_user_state: Pubkey::new_unique(),
            reserve_farm_state: Pubkey::new_unique(),
            farms_program: kamino_klend_client::FARMS_PROGRAM_ID,
        }
    }

    #[test]
    fn klend_deposit_v2_ix_has_17_accounts_in_canonical_order() {
        let keys = dummy_deposit_v2_keys();
        let ix = kamino_klend_client::build_deposit_v2_ix(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &keys,
            1_000_000,
        );
        assert_eq!(ix.program_id, kamino_klend_client::PROGRAM_ID_MAINNET);
        assert_eq!(ix.accounts.len(), 17);
        // Discriminator (8 bytes) + liquidity_amount u64 LE (8 bytes) = 16
        assert_eq!(ix.data.len(), 16);
        assert_eq!(
            &ix.data[..8],
            &kamino_klend_client::DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISC
        );
        assert_eq!(&ix.data[8..16], &1_000_000u64.to_le_bytes());

        // Position 0: owner — signer + mut
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[0].is_writable);
        assert_eq!(ix.accounts[0].pubkey, keys.owner);

        // Position 1: obligation — mut, not signer
        assert!(!ix.accounts[1].is_signer && ix.accounts[1].is_writable);
        // Position 2: lending_market — readonly
        assert!(!ix.accounts[2].is_signer && !ix.accounts[2].is_writable);
        // Position 3: lending_market_authority — readonly
        assert!(!ix.accounts[3].is_writable);
        // Position 4: reserve — mut
        assert!(ix.accounts[4].is_writable);
        // Position 5: reserve_liquidity_mint — readonly
        assert!(!ix.accounts[5].is_writable);
        // Position 6: reserve_liquidity_supply — mut
        assert!(ix.accounts[6].is_writable);
        // Position 7: reserve_collateral_mint — mut
        assert!(ix.accounts[7].is_writable);
        // Position 8: reserve_destination_deposit_collateral — mut
        assert!(ix.accounts[8].is_writable);
        // Position 9: user_source_liquidity — mut
        assert!(ix.accounts[9].is_writable);
        // Position 10: placeholder_user_destination_collateral — readonly
        assert!(!ix.accounts[10].is_writable);
        // Position 11: collateral_token_program — readonly
        assert!(!ix.accounts[11].is_writable);
        // Position 12: liquidity_token_program — readonly
        assert!(!ix.accounts[12].is_writable);
        // Position 13: instruction_sysvar — readonly
        assert!(!ix.accounts[13].is_writable);
        // Position 14: obligation_farm_user_state — mut
        assert!(ix.accounts[14].is_writable);
        // Position 15: reserve_farm_state — mut
        assert!(ix.accounts[15].is_writable);
        // Position 16: farms_program — readonly
        assert!(!ix.accounts[16].is_writable);
        assert_eq!(ix.accounts[16].pubkey, kamino_klend_client::FARMS_PROGRAM_ID);
    }

    #[test]
    fn klend_init_obligation_ix_account_shape() {
        let keys = kamino_klend_client::InitObligationAccountKeys {
            obligation_owner: Pubkey::new_unique(),
            fee_payer: Pubkey::new_unique(),
            obligation: Pubkey::new_unique(),
            lending_market: Pubkey::new_unique(),
            seed1_account: Pubkey::new_unique(),
            seed2_account: Pubkey::new_unique(),
            owner_user_metadata: Pubkey::new_unique(),
            rent: Pubkey::new_unique(),
            system_program: Pubkey::new_unique(),
        };
        let args = kamino_klend_client::InitObligationArgs { tag: 0, id: 0 };
        let ix = kamino_klend_client::build_init_obligation_ix(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &keys,
            &args,
        );
        assert_eq!(ix.accounts.len(), 9);
        // disc(8) + tag(1) + id(1) = 10
        assert_eq!(ix.data.len(), 10);
        assert_eq!(&ix.data[..8], &kamino_klend_client::INIT_OBLIGATION_DISC);

        // owner is signer (readonly), fee_payer is signer + mut.
        assert!(ix.accounts[0].is_signer && !ix.accounts[0].is_writable);
        assert!(ix.accounts[1].is_signer && ix.accounts[1].is_writable);
        // obligation is mut.
        assert!(!ix.accounts[2].is_signer && ix.accounts[2].is_writable);
    }

    #[test]
    fn klend_init_user_metadata_ix_account_shape() {
        let keys = kamino_klend_client::InitUserMetadataAccountKeys {
            owner: Pubkey::new_unique(),
            fee_payer: Pubkey::new_unique(),
            user_metadata: Pubkey::new_unique(),
            referrer_user_metadata: Pubkey::new_unique(),
            rent: Pubkey::new_unique(),
            system_program: Pubkey::new_unique(),
        };
        let lut = Pubkey::default();
        let ix = kamino_klend_client::build_init_user_metadata_ix(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &keys,
            &lut,
        );
        assert_eq!(ix.accounts.len(), 6);
        // disc(8) + lookup_table([u8;32]) = 40
        assert_eq!(ix.data.len(), 40);
        assert_eq!(&ix.data[..8], &kamino_klend_client::INIT_USER_METADATA_DISC);
        assert_eq!(&ix.data[8..40], lut.as_ref());

        // owner is signer (readonly), fee_payer is signer + mut.
        assert!(ix.accounts[0].is_signer && !ix.accounts[0].is_writable);
        assert!(ix.accounts[1].is_signer && ix.accounts[1].is_writable);
        assert!(!ix.accounts[2].is_signer && ix.accounts[2].is_writable);
    }

    #[test]
    fn klend_lending_market_authority_pda_derivation_is_deterministic() {
        let market = Pubkey::new_unique();
        let (pda1, bump1) = kamino_klend_client::derive_lending_market_authority(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &market,
        );
        let (pda2, bump2) = kamino_klend_client::derive_lending_market_authority(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &market,
        );
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn klend_user_metadata_pda_derivation_is_deterministic() {
        let owner = Pubkey::new_unique();
        let (pda1, _) = kamino_klend_client::derive_user_metadata_pda(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &owner,
        );
        let (pda2, _) = kamino_klend_client::derive_user_metadata_pda(
            &kamino_klend_client::PROGRAM_ID_MAINNET,
            &owner,
        );
        assert_eq!(pda1, pda2);
    }

    // -- CCIP send-side encoding tests -----------------------------

    #[test]
    fn generic_extra_args_v2_encoding_layout() {
        // Format: tag(4) + gasLimit(uint256 BE = 32) + allowOOO(bool = 1) = 37 bytes
        let encoded = super::encode_generic_extra_args_v2(200_000, false);
        assert_eq!(encoded.len(), 37);
        assert_eq!(&encoded[..4], &super::GENERIC_EXTRA_ARGS_V2_TAG);
        // First 24 bytes of gasLimit are zero (u64 padded into uint256 BE)
        assert!(encoded[4..28].iter().all(|b| *b == 0));
        // Next 8 bytes = 200_000 in BE
        assert_eq!(&encoded[28..36], &200_000u64.to_be_bytes());
        // Last byte = allowOOO = false = 0
        assert_eq!(encoded[36], 0);
    }

    #[test]
    fn generic_extra_args_v2_allow_ooo_true_encodes_one() {
        let encoded = super::encode_generic_extra_args_v2(1, true);
        assert_eq!(encoded[36], 1);
    }

    #[test]
    fn pullback_intent_round_trips_with_pullback_kind() {
        // The pull-back intent uses PullbackForPayout (kind=1). Verify
        // it round-trips through encode/decode just like the deposit
        // intent — same layout, different kind byte.
        let intent = CrossVMIntentPayload {
            source_chain: super::CCIP_SOLANA_DEVNET_CHAIN_SELECTOR,
            amount: 500_000_000,
            source_address: [0x42u8; 32],
            merchant: [0x99u8; 32],
            nonce: [0xCDu8; 32],
            kind: IntentKind::PullbackForPayout,
        };
        let encoded = intent.encode();
        assert_eq!(encoded.len(), INTENT_ENCODED_LENGTH);
        // Kind byte at offset 1 must be 0x01 (PullbackForPayout)
        assert_eq!(encoded[1], 0x01);
        let decoded = CrossVMIntentPayload::decode(&encoded).unwrap();
        assert_eq!(decoded, intent);
    }

    #[test]
    fn klend_farms_program_id_is_canonical() {
        // Pinned to the Kamino Farms program ID per upstream verification.
        // Drift on this constant means the v2 deposit ix will fail at
        // klend's farms_program address check.
        assert_eq!(
            kamino_klend_client::FARMS_PROGRAM_ID,
            pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"),
        );
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
pub struct KaminoObligationInitialized {
    pub obligation: Pubkey,
    pub user_metadata: Pubkey,
}

#[event]
pub struct SettlementBound {
    pub amount: u64,
    pub nonce: [u8; 32],
    pub mppsol_receipt: Pubkey,
}

/// Authoritative pull-back request emitted by `settle_payout_to_tempo`
/// or `request_pullback_to_tempo`. The keeper consumes this off-chain
/// to perform the EVM-side settlement (either via Chainlink CCIP, once
/// available on Tempo, or via the trusted-keeper path that mirrors the
/// inbound flow). All fields needed to construct the destination tx
/// are included — no extra account reads required.
#[event]
pub struct PullbackRequested {
    pub amount: u64,
    pub nonce: [u8; 32],
    /// CCIP chain selector for the destination Tempo chain, mirroring
    /// the inbound `vault.expected_tempo_chain_selector`.
    pub destination_chain: u64,
    /// Tempo Buffer.sol address (left-padded 32 bytes per Solidity
    /// convention). Same form as the inbound `expected_tempo_sender`.
    pub receiver: [u8; 32],
    /// Canonical 122-byte CrossVMIntent encoded as PullbackForPayout.
    /// The keeper places this verbatim in CCIP message data, or
    /// passes it to the Tempo Buffer's _ccipReceive equivalent in
    /// the trusted-keeper path.
    pub intent_bytes: Vec<u8>,
    /// USDC mint on Solana (informational — the keeper already knows,
    /// but emitting closes the gap for indexers).
    pub usdc_mint: Pubkey,
    /// Suggested EVM destination gas limit. Encoded into CCIP
    /// extra_args via `encode_generic_extra_args_v2`. Keeper-overridable.
    pub suggested_gas_limit: u64,
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
    #[msg("Caller does not match configured trusted keeper")]
    NotTrustedKeeper,
    #[msg("Trusted-keeper path is disabled (vault.trusted_keeper is default)")]
    TrustedKeeperPathDisabled,
    #[msg("Caller does not match vault authority")]
    WrongAuthority,
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
