use crate::{
    config::{PyraError, ANCHOR_DISCRIMINATOR, TIME_LOCK_DURATION_SLOTS},
    state::{TimeLock, Vault, WithdrawOrder},
    utils::{allocate_time_lock_owner_payer, allocate_time_lock_program_payer},
};
use anchor_lang::{prelude::*, Discriminator};

#[derive(Accounts)]
pub struct InitiateWithdraw<'info> {
    #[account(
        seeds = [b"vault".as_ref(), owner.key().as_ref()],
        bump = vault.bump
    )]
    pub vault: Box<Account<'info, Vault>>,

    pub owner: Signer<'info>,

    #[account(mut)]
    pub withdraw_order: Signer<'info>,

    /// CHECK: Checked in handler
    #[account(mut)]
    pub time_lock_rent_payer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: Can be any account
    pub destination: UncheckedAccount<'info>,
}

/// Creates a time locked withdraw order, which can be fulfilled permissionlessly once the time lock has expired. Time locks prevent edge cases of double spend with the Pyra card.
pub fn initiate_withdraw_handler<'info>(
    ctx: Context<'_, '_, '_, 'info, InitiateWithdraw<'info>>,
    amount_base_units: u64,
    drift_market_index: u16,
    reduce_only: bool,
) -> Result<()> {
    let is_owner_payer = ctx
        .accounts
        .time_lock_rent_payer
        .key()
        .eq(&ctx.accounts.owner.key());

    if is_owner_payer {
        allocate_time_lock_owner_payer(
            &ctx.accounts.owner,
            &ctx.accounts.withdraw_order,
            &ctx.accounts.system_program,
            WithdrawOrder::INIT_SPACE,
        )?;
    } else {
        allocate_time_lock_program_payer(
            &ctx.accounts.time_lock_rent_payer.to_account_info(),
            &ctx.accounts.withdraw_order,
            &ctx.accounts.system_program,
            WithdrawOrder::INIT_SPACE,
        )?;
    }

    let current_slot = Clock::get()?.slot;
    let release_slot = current_slot
        .checked_add(TIME_LOCK_DURATION_SLOTS)
        .ok_or(PyraError::MathOverflow)?;

    let withdraw_order_data = WithdrawOrder {
        time_lock: TimeLock {
            owner: ctx.accounts.owner.key(),
            is_owner_payer,
            release_slot,
        },
        amount_base_units,
        drift_market_index,
        reduce_only,
        destination: ctx.accounts.destination.key(),
    };
    let withdraw_order_data_vec = withdraw_order_data.try_to_vec()?;

    let mut data = ctx.accounts.withdraw_order.try_borrow_mut_data()?;
    data[..ANCHOR_DISCRIMINATOR].copy_from_slice(&WithdrawOrder::DISCRIMINATOR);
    data[ANCHOR_DISCRIMINATOR..].copy_from_slice(&withdraw_order_data_vec[..]);

    Ok(())
}
