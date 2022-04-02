use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_lang::solana_program::{clock};
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo, SetAuthority, Transfer};
use spl_token::instruction::AuthorityType;
use chainlink_solana as chainlink;

declare_id!("AzZzWJ8U5F6PJ46Y6umvJTYrE2tVGn1axBYfUeqvtNzm");


pub fn available(
    ticket: &mut Box<Account<Ticket>>,
) -> u64 {
    if has_cliffed(ticket) {
        return unlocked(ticket);
    } else {
        return 0;
    }
}


pub fn has_cliffed(
    ticket: &mut Box<Account<Ticket>>,
) -> bool {
    let clock = clock::Clock::get().unwrap();
    if ticket.cliff == 0 {
        return true;
    }

    return  clock.unix_timestamp as u64 > ticket.created_at.checked_add(
        ticket.cliff.checked_mul(
            86400
        ).unwrap()
    ).unwrap();
}


pub fn unlocked(
    ticket: &mut Box<Account<Ticket>>,
) -> u64 {
    let clock = clock::Clock::get().unwrap();
    
    let timelapsed = (clock.unix_timestamp as u64).checked_sub(ticket.created_at).unwrap();  
    let vesting_in_seconds = ticket.vesting.checked_mul(86400).unwrap();

    return timelapsed.checked_mul(ticket.balance).unwrap().checked_div(
        vesting_in_seconds as u64
    ).unwrap();
}


const VESTOR_PDA_SEED: &[u8] = b"vesting__init";


#[program]
pub mod vestor_using_anchor_chainlink_master {
    use super::*;

   #[access_control(Initialize::initialize_test_state(&ctx, bump))]
   pub fn initialize_test_state(ctx : Context<Initialize>, amount : u64, bump : u8) -> Result<()> {
       
       #[warn(unused_must_use)]
       token::mint_to(ctx.accounts.into(), amount);
       ctx.accounts.vestor.bump = bump;

       Ok(())

   }
 
    
    pub fn create_ticket(ctx: Context<CreateTicket>, beneficiary: Pubkey, cliff: u64, vesting: u64, amount: u64, irrevocable: bool  , bump_seed : u8) -> Result<()> {
        let clock = clock::Clock::get().unwrap();

        if amount == 0 {
            return Err(ErrorCode::AmountMustBeGreaterThanZero.into());
        } if vesting < cliff {
            return Err(ErrorCode::VestingPeriodShouldBeEqualOrLongerThanCliff.into());
        } 


        ctx.accounts.ticket.grantor = *ctx.accounts.grantor.to_account_info().key;
        ctx.accounts.ticket.grantor_deposit_token_vault = *ctx.accounts.grantor_deposit_token_vault.to_account_info().key;
        require!(ctx.accounts.grantor_deposit_token_vault.amount >= amount, ErrorCode::NotEnoughTokensMinted);
        ctx.accounts.ticket.bump = bump_seed;
        
        let (vestor, _bump_seed) = Pubkey::find_program_address(&[VESTOR_PDA_SEED,
            &ctx.accounts.grantor.to_account_info().key.as_ref()], ctx.program_id);
        //Set authority of the Tickets to the 'vestor pda'
        token::set_authority(ctx.accounts.into(), AuthorityType::AccountOwner, Some(vestor))?;
       
        
        let ticket = &mut ctx.accounts.ticket;
        ticket.token_mint = ctx.accounts.token_mint.key();
        ticket.grantor_deposit_token_vault = ctx.accounts.grantor_deposit_token_vault.key();
        ticket.grantor = ctx.accounts.grantor.key();
        ticket.claimant = beneficiary;
        ticket.cliff = cliff;
        ticket.vesting = vesting;
        ticket.amount = amount;
        ticket.balance = amount;
        ticket.created_at = clock.unix_timestamp as u64;
        ticket.irrevocable = irrevocable;
        ticket.is_revoked = false;
       

        Ok(())
    }


    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        
        let (_vestor, bump_seed) = Pubkey::find_program_address(&[VESTOR_PDA_SEED, 
            &ctx.accounts.grantor.to_account_info().key.as_ref()], 
            ctx.program_id);
        let seeds = &[&VESTOR_PDA_SEED, &ctx.accounts.grantor.to_account_info().key.as_ref()[..], &[bump_seed]];
        let clock = clock::Clock::get().unwrap();

        if ctx.accounts.ticket.is_revoked == true {
            return Err(ErrorCode::TicketRevoked.into());
        }

        let sol_round = chainlink::latest_round_data(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_sol_feed.to_account_info(),
        )?;

        let sol_description = chainlink::description(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_sol_feed.to_account_info(),
        )?;

        let sol_decimals = chainlink::decimals(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_sol_feed.to_account_info(),
        )?;

        // Set the account value
        let value_of_sol: &mut Account<Value> = &mut ctx.accounts.value;
        value_of_sol.value= sol_round.answer;
        value_of_sol.decimals=u32::from(sol_decimals);

        // Also print the SOL value to the program output
        let value_print_sol = Value::new(sol_round.answer, u32::from(sol_decimals));
        msg!("{} price is {}", sol_description, value_print_sol);


        let eth_round = chainlink::latest_round_data(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_eth_feed.to_account_info(),
        )?;

        let eth_description = chainlink::description(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_eth_feed.to_account_info(),
        )?;

        let eth_decimals = chainlink::decimals(
            ctx.accounts.chainlink_program.to_account_info(),
            ctx.accounts.chainlink_eth_feed.to_account_info(),
        )?;

        // Set the account value
        let value_of_eth: &mut Account<Value> = &mut ctx.accounts.value;
        value_of_eth.value= eth_round.answer;
        value_of_eth.decimals=u32::from(eth_decimals);

        // Also print the ETH value to the program output
        let value_print_eth = Value::new(eth_round.answer, u32::from(eth_decimals));
        msg!("{} price is {}", eth_description, value_print_eth);

        let now = clock.unix_timestamp as u64;

        //Lucky combination of 0 claims + randomness of Time + condition of SOL Price having crossed ETH Price, 
        // Now all Vestors can claim and sell their Tokens as its Merry Christmas Time for the SOL Ecosystem..
        if now % 2 == 0  && ctx.accounts.ticket.claimed == 0 && value_print_sol.value > value_print_eth.value 
            {
                let amount = ctx.accounts.ticket.balance;

                // Transfer and set authority
                {
                    token::transfer(ctx.accounts
                        .into_transfer_to_claimant_context()
                        .with_signer(&[&seeds[..]]),
                        amount)?;
                    
                    token::set_authority(ctx.accounts
                        .into_set_authority_context()
                        .with_signer(&[&seeds[..]]), AuthorityType::AccountOwner, 
                        Some(ctx.accounts.ticket.grantor))?;

                }

                ctx.accounts.ticket.claimed += amount;
                ctx.accounts.ticket.balance -= amount;
                ctx.accounts.ticket.last_claimed_at = clock.unix_timestamp as u64;
                ctx.accounts.ticket.num_claims += 1;

            }
        else 
            {
                let amount = available(&mut ctx.accounts.ticket);


                // Transfer and set Authority
                {
                    token::transfer(ctx.accounts
                        .into_transfer_to_claimant_context()
                        .with_signer(&[&seeds[..]]),
                        amount)?;
                    
                    token::set_authority(ctx.accounts
                        .into_set_authority_context()
                        .with_signer(&[&seeds[..]]), AuthorityType::AccountOwner, 
                        Some(ctx.accounts.ticket.grantor))?;

                }

                ctx.accounts.ticket.claimed += amount;
                ctx.accounts.ticket.balance -= amount;
                ctx.accounts.ticket.last_claimed_at = clock.unix_timestamp as u64;
                ctx.accounts.ticket.num_claims += 1;
            }
        
       

        Ok(())
    }


    pub fn revoke(ctx: Context<Revoke>) -> Result<()> {
        
        let (_vestor, bump_seed) = Pubkey::find_program_address(&[VESTOR_PDA_SEED,
            &ctx.accounts.grantor.to_account_info().key.as_ref()], ctx.program_id);
        let seeds = &[&VESTOR_PDA_SEED, &ctx.accounts.grantor.to_account_info().key.as_ref()[..], &[bump_seed]];
        let _clock = clock::Clock::get().unwrap();

        if ctx.accounts.ticket.is_revoked == true {
            return Err(ErrorCode::TicketRevoked.into());
        } 

 
        if ctx.accounts.ticket.irrevocable == true {
            return Err(ErrorCode::TicketIrrevocable.into());
        }

        // Transfer.
        {
            token::transfer(ctx.accounts
                .into_transfer_to_grantor_context()
                .with_signer(&[&seeds[..]]),
                ctx.accounts.ticket.balance)?;

            token::set_authority(ctx.accounts
                .into_set_authority_context()
                .with_signer(&[&seeds[..]]), AuthorityType::AccountOwner, 
                Some(ctx.accounts.ticket.grantor))?;
        }

        ctx.accounts.ticket.is_revoked = true;
        ctx.accounts.ticket.balance = 0;

        Ok(())
    }



}


#[derive(Accounts)]
#[instruction( bump : u8 )]
pub struct Initialize<'info> {
    // Total 6 accounts used :
    /// CHECK: The 'vestor PDA' can also be used as AccountInfo<'info> & is not dangerous because
    /// its seeds + bump are used to initialize the Program
    #[account(init, payer = grantor)]
    pub vestor : Account<'info, Vestor>, 
    #[account(mut)]
    pub grantor_deposit_token_vault : Account<'info, TokenAccount>, 
    pub token_mint : Account<'info, Mint>, 
    #[account(mut)]
    pub grantor : Signer<'info>,
    pub token_program : Program<'info, Token>, 
    pub system_program : Program<'info, System>, 
}

    impl<'info> Initialize<'info> {
        pub fn initialize_test_state(ctx: &Context<Initialize>, bump: u8) -> Result<()> {
          let vestor_bump = 
          Pubkey::find_program_address(&[VESTOR_PDA_SEED, &ctx.accounts.grantor.to_account_info().key.as_ref()], ctx.program_id).1;
          if vestor_bump != bump {
              return Err(ErrorCode::UnauthorizedVestingProgramCreator.into())
          }
          let seeds = &[VESTOR_PDA_SEED,
          &ctx.accounts.grantor.to_account_info().key.as_ref(), &[vestor_bump]];
          Pubkey::create_program_address(seeds, &ctx.program_id).map_err(|_| ErrorCode::InvalidNonce)?;
         
          Ok(())
        }
      } 

#[derive(Accounts)]
#[instruction(amount : u64  , bump_seed : u8 )]
pub struct CreateTicket<'info> {
    // Total 6 accounts used in 'Create'

    
    #[account(mut)]
    pub grantor: Signer<'info>,

    #[account(mut,
        constraint = grantor_deposit_token_vault.mint == token_mint.key())]
    pub grantor_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(init, seeds = [b"init_____ticket"], bump, payer = grantor, space = Ticket::LEN)]
    pub ticket : Box<Account<'info, Ticket>>,

    pub token_mint: Box<Account<'info, Mint>>,
    
    pub token_program : Program<'info, Token>, 

    pub system_program : Program<'info, System>, 

    }

  


   
#[derive(Accounts)]
pub struct Claim<'info> {  
    // Total 13 accounts are used for 'Claim'

    /// CHECK: The 'vestor PDA' is not dangerous because
    /// its seeds + bump are used to sign this 'transfer' tx in the 'claim' function.
    #[account(mut)]
    pub vestor: AccountInfo<'info>, // This is the PDA which signs

    ///CHECK : The grantor is not unsafe because some other constraints have been issued to Ticket
    /// which ensure that ticket.grantor == *grantor.key
    #[account(mut)]
    pub grantor : AccountInfo<'info>, 

    #[account(
        mut,
        has_one = claimant,
        has_one = claimant_receive_token_vault,
        has_one = grantor, 
        constraint = ticket.balance > 0,
        constraint = ticket.balance <= pda_deposit_token_vault.amount,
        constraint = ticket.grantor_deposit_token_vault == *pda_deposit_token_vault.to_account_info().key, 
        close = grantor     
    )]
    pub ticket: Box<Account<'info, Ticket>>,

    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = pda_deposit_token_vault.mint == token_mint.key(),
        constraint = pda_deposit_token_vault.owner == vestor.key(),
    )]
    pub pda_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = claimant_receive_token_vault.mint == token_mint.key(),
        constraint = claimant_receive_token_vault.owner == claimant.key(),
    )]
    pub claimant_receive_token_vault: Box<Account<'info, TokenAccount>>,

    ///CHECK: The claimant is not unsafe because some other constraints have been issued to Ticket
    /// which ensure that ticket.claimant == *claimant.key (see => has_one = claimant)
    #[account(signer)]
    pub claimant: AccountInfo<'info>,


    #[account(init, payer = grantor, space = 100)]
    pub value: Account<'info, Value>,

    ///CHECK : This account just reads the Sol Price from SOLANA_FEED ADDRESS && which arrived from the Chainlink Program
    pub chainlink_sol_feed: AccountInfo<'info>,

    ///CHECK : This account just reads the ETH Price from ETHEREUM_FEED ADDRESS && which arrived from the Chainlink Program
    pub chainlink_eth_feed: AccountInfo<'info>,

    /// CHECK : This is the Chainlink program's account
    pub chainlink_program: AccountInfo<'info>,

     /// CHECK : System Program address is already defined
     #[account(address = system_program::ID)]
    pub system_program: AccountInfo<'info>,

    //pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}



#[derive(Accounts)]
pub struct Revoke<'info> {
    //Total 6 accounts used for Revoke

    /// CHECK: The 'vestor PDA' is not dangerous because
    /// its seeds + bump are used to sign this 'set_authority ctx' tx in the 'revoke' function.
    #[account(mut)]
    pub vestor: AccountInfo<'info>, // This is the PDA which signs

    #[account(mut)]
    pub grantor: Signer<'info>,

    #[account(
        mut,
        has_one = grantor,
        has_one = token_mint,
        constraint = ticket.grantor_deposit_token_vault == *pda_deposit_token_vault.to_account_info().key,
        constraint = ticket.balance > 0,
        close = grantor
    )]
    pub ticket: Box<Account<'info, Ticket>>,

    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = pda_deposit_token_vault.mint == token_mint.key(),
        constraint = pda_deposit_token_vault.owner == vestor.key(),
    )]
    pub pda_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
pub struct Vestor {
    bump : u8,
}

#[account]
#[derive(Default)]
pub struct Ticket {
    pub token_mint: Pubkey, // 32
    pub grantor: Pubkey, // 32
    pub claimant: Pubkey, //32
    pub cliff: u64, //8
    pub vesting: u64, //8
    pub amount: u64, //8
    pub claimed: u64, //8
    pub balance: u64, //8
    pub created_at: u64, //8
    pub last_claimed_at: u64, //8
    pub num_claims: u64, //8
    pub irrevocable: bool, //8
    pub is_revoked: bool, //8
    pub revoked_at: u64, //8
    pub grantor_receive_token_vault : Pubkey, //32 
    pub grantor_deposit_token_vault : Pubkey, //32
    pub claimant_receive_token_vault : Pubkey, //32
    pub bump : u8, // 8
    

}

impl Ticket {
    pub const LEN : usize = 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 32 + 32 + 32 + 8; //Total = 288 bytes

}

#[account]
#[derive(Default)]
pub struct SignerAccount {
    bump : u8,

}

#[account]
pub struct Value {
    pub value: i128,
    pub decimals: u32,
}

impl Value {
    pub fn new(value: i128, decimals: u32) -> Self {
        Value { value, decimals }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut scaled_val = self.value.to_string();
        if scaled_val.len() <= self.decimals as usize {
            scaled_val.insert_str(
                0,
                &vec!["0"; self.decimals as usize - scaled_val.len()].join(""),
            );
            scaled_val.insert_str(0, "0.");
        } else {
            scaled_val.insert(scaled_val.len() - self.decimals as usize, '.');
        }
        f.write_str(&scaled_val)
    }
}



impl<'info> From<&mut Initialize<'info>> for CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
    fn from(accounts: &mut Initialize<'info>) -> Self {
        let cpi_accounts = MintTo {
            authority: accounts.grantor.to_account_info().clone(),
            mint: accounts.token_mint.to_account_info().clone(),
            to: accounts.grantor_deposit_token_vault.to_account_info().clone(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
   }
}


impl<'info> From<&mut CreateTicket<'info>> for CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
    fn from(accounts: &mut CreateTicket<'info>) -> Self {
        let cpi_accounts = SetAuthority {
            account_or_mint: accounts
                .grantor_deposit_token_vault
                .to_account_info()
                .clone(),
            current_authority: accounts.grantor.to_account_info().clone(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
   }
}


impl<'info> Claim<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint : self.pda_deposit_token_vault.to_account_info().clone(),
            current_authority : self.vestor.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'info> Claim<'info> {
    fn into_transfer_to_claimant_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.pda_deposit_token_vault.to_account_info().clone(),
            to : self.claimant_receive_token_vault.to_account_info().clone(),
            authority : self.vestor.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'info> Revoke<'info> {
    fn into_transfer_to_grantor_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.pda_deposit_token_vault.to_account_info().clone(),
            to : self.pda_deposit_token_vault.to_account_info().clone(),
            authority : self.vestor.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}


impl<'info> Revoke<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint : self.pda_deposit_token_vault.to_account_info().clone(),
            current_authority : self.vestor.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Amount must be greater than zero.")]
    AmountMustBeGreaterThanZero,
    #[msg("Vesting period should be equal or longer to the cliff")]
    VestingPeriodShouldBeEqualOrLongerThanCliff,
    #[msg("Ticket has been revoked")]
    TicketRevoked,
    #[msg("Ticket is irrevocable")]
    TicketIrrevocable,
    #[msg("Incorrect Nonce provided")]
    InvalidNonce,
    #[msg("Unauthorized Ticket Creator")]
    UnauthorizedVestingProgramCreator,
    #[msg("Ask Admin/Owner to mint more tokens")]
    NotEnoughTokensMinted,
}

