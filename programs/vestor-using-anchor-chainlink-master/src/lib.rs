use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_lang::solana_program::{clock};
use anchor_spl::token::{self, Mint, Token, TokenAccount, SetAuthority, Transfer};
use spl_token::instruction::AuthorityType;
use chainlink_solana as chainlink;
use pyth_client::{self, load_price, Price};
declare_id!("8hst6KmcWGU5SDoJUQUpjNckeyQxJrsHrksXhx52x1C4");


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



#[program]
pub mod vestor_using_anchor_chainlink_master {
    

    use super::*;

   #[access_control(Initialize::has_access(&ctx))]
   pub fn initialize(ctx : Context<Initialize>, amount : u64) -> Result<()> {

        token::transfer(ctx.accounts
        .into_transfer_to_ticket_creator_context(), 
        amount)?;

       
        ctx.accounts.vestor.tickets_issued = 0;

       
       Ok(())

   }


   
    #[access_control(CreateTicket::accounts(&ctx, bump))]
    pub fn create_ticket(ctx: Context<CreateTicket>, beneficiary: Pubkey, cliff: u64, vesting: u64, amount: u64, irrevocable: bool  , bump : u8) -> Result<()> {
        let clock = clock::Clock::get().unwrap();
        
        if amount == 0 {
            return Err(ErrorCode::AmountMustBeGreaterThanZero.into());
        } if vesting < cliff {
            return Err(ErrorCode::VestingPeriodShouldBeEqualOrLongerThanCliff.into());
        } 

        require!(ctx.accounts.ticket_creator_deposit_token_vault.amount >= amount, ErrorCode::NotEnoughTokens);

        
        let (signer, _bump_seed) = Pubkey::find_program_address(&[
            &ctx.accounts.ticket.to_account_info().key.as_ref(),
            &ctx.accounts.vestor.tickets_issued.to_string().as_ref()], 
            ctx.program_id);
        //Set authority of the Tickets to the signer pda'
        token::set_authority(ctx.accounts.into(), AuthorityType::AccountOwner, Some(signer))?;
       
        
        let ticket = &mut ctx.accounts.ticket;
       
        ticket.creator_deposit_token_vault = *ctx.accounts
        .ticket_creator_deposit_token_vault
        .to_account_info().key;
        ticket.claimant_receive_token_vault = *ctx.accounts
        .claimant_receive_token_vault
        .to_account_info().key;
        ticket.vault = *ctx.accounts
        .vault
        .to_account_info().key;
        ticket.owner = *ctx.accounts
        .owner
        .to_account_info().key;
        ticket.token_mint = *ctx.accounts
        .token_mint
        .to_account_info().key;
        ticket.claimant = beneficiary;
        ticket.cliff = cliff;
        ticket.vesting = vesting;
        ticket.amount = amount;
        ticket.balance = amount;
        ticket.created_at = clock.unix_timestamp as u64;
        ticket.irrevocable = irrevocable;
        ticket.is_revoked = false;
        ticket.bump = bump;
        ticket.num_claims = 0;
       

        ctx.accounts.vestor.tickets_issued += 1;
       

        Ok(())
    }

    #[access_control(not_revoked(&ctx.accounts.ticket))]
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let clock = clock::Clock::get().unwrap();
        let (_signer, bump_seed) = Pubkey::find_program_address(&[ 
            &ctx.accounts.ticket.to_account_info().key.as_ref()], 
            ctx.program_id);
        let seeds = &[&ctx.accounts.ticket.to_account_info().key.as_ref()[..], &[bump_seed]];
        

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
        let value_of_sol: &mut Account<ChainlinkValue> = &mut ctx.accounts.chainlink_value;
        value_of_sol.value= sol_round.answer;
        value_of_sol.decimals=u32::from(sol_decimals);

        // Also print the SOL value to the program output
        let value_print_sol = ChainlinkValue::new(sol_round.answer, u32::from(sol_decimals));
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
        let value_of_eth: &mut Account<ChainlinkValue> = &mut ctx.accounts.chainlink_value;
        value_of_eth.value= eth_round.answer;
        value_of_eth.decimals=u32::from(eth_decimals);

        // Also print the ETH value to the program output
        let value_print_eth = ChainlinkValue::new(eth_round.answer, u32::from(eth_decimals));
        msg!("{} price is {}", eth_description, value_print_eth);

       // Now lets console the Pyth values: 
        let pyth_price_info = &ctx.accounts.pyth_account;
        let pyth_price_data = &pyth_price_info.try_borrow_data()?;
        let price_account: Price = *load_price(pyth_price_data).unwrap();
            
        msg!("Pyth's Sol price_account address .. {:?}", pyth_price_info.key);
        msg!("Price_Type ... {:?}", price_account.ptype);
        msg!("Sol price from Pyth........ {}", price_account.agg.price);
        let value_print_sol_pyth = price_account.agg.price as i128;
         
        let now = clock.unix_timestamp as u64;
       
       

        //Lucky combination of 0 claims + randomness of Time + condition of SOL Price having crossed ETH Price (i.e Merry Christmas Time) , 
        // && Pyth Sol Price == Chainlink Sol Price ( which is almost an impossibility)
        // Then all Tickets can be claimed before Vesting schedule Expiration. 
        if now % 2 == 0  
        && ctx.accounts.ticket.claimed == 0 
        && value_print_sol.value > value_print_eth.value 
        && value_print_sol.value == value_print_sol_pyth

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
                        Some(ctx.accounts.ticket.owner))?;

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
                        Some(ctx.accounts.ticket.owner))?;

                }

                ctx.accounts.ticket.claimant = *ctx.accounts
                .claimant
                .to_account_info().key;
                ctx.accounts.ticket.claimed += amount;
                ctx.accounts.ticket.balance -= amount;
                ctx.accounts.ticket.last_claimed_at = clock.unix_timestamp as u64;
                ctx.accounts.ticket.num_claims += 1;
            }
        
       

        Ok(())
    }


    pub fn revoke(ctx: Context<Revoke>) -> Result<()> {
        let clock = clock::Clock::get().unwrap();
        let (_signer, bump_seed) = Pubkey::find_program_address(&[
            &ctx.accounts.ticket.to_account_info().key.as_ref()], ctx.program_id);
        let seeds = &[&ctx.accounts.ticket.to_account_info().key.as_ref()[..], &[bump_seed]];
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
                .into_transfer_to_ticket_creator_context()
                .with_signer(&[&seeds[..]]),
                ctx.accounts.ticket.balance)?;

            token::set_authority(ctx.accounts
                .into_set_authority_context()
                .with_signer(&[&seeds[..]]), AuthorityType::AccountOwner, 
                Some(ctx.accounts.ticket.owner))?;
        }

        ctx.accounts.ticket.is_revoked = true;
        ctx.accounts.ticket.balance = 0;
        ctx.accounts.ticket.revoked_at = clock.unix_timestamp as u64;

        Ok(())
    }

  
}






#[derive(Accounts)]
pub struct Initialize<'info> {
    // Total 7 accounts used in Initialization of the Program :
    // vestor being created
   #[account(init, payer = owner, space = 8 + 8)]
    pub vestor : Box<Account<'info, Vestor>>,

    #[account(mut, has_one = owner, constraint = contract_owner_deposit_token_vault.mint == token_mint.key())]
    pub contract_owner_deposit_token_vault : Box<Account<'info, TokenAccount>>, 

    #[account(mut,
        constraint = ticket_creator_deposit_token_vault.mint == contract_owner_deposit_token_vault.mint)]
    pub ticket_creator_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    pub token_mint : Box<Account<'info, Mint>>, 

    // The Owner of contract_owner_deposit_token_vault
    #[account(mut)]
    pub owner : Signer<'info>,
    
    pub token_program : Program<'info, Token>, 

    pub system_program : Program<'info, System>, 

     
}

    impl<'info> Initialize<'info> {
        pub fn has_access(_ctx: &Context<Initialize>) -> Result<()> {
         // TODO : add some whitelist admins
         
          Ok(())
        }
      } 


#[derive(Accounts)]
pub struct CreateTicket<'info> {
    // Total 10 accounts used in 'Create'
    //ticket being created
    #[account(zero)]
    pub ticket : Box<Account<'info, Ticket>>,

    /// CHECK : The Owner of ticket_creator_deposit_token_vault
    pub owner : AccountInfo<'info>,

    ///CHECK: Program Derived address (PDA) for the Ticket
    #[account(
        seeds = [
            ticket.to_account_info().key.as_ref(),
            vestor.tickets_issued.to_string().as_ref()
            ],
        bump = ticket.bump,
    )]
    pub signer : AccountInfo<'info>, 

    pub token_mint : Box<Account<'info, Mint>>, 

    // This is the 'from' token
    #[account(mut, has_one = owner )]
    pub ticket_creator_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    //This is the 'to' token
    #[account(mut, 
        constraint = claimant_receive_token_vault.mint == ticket_creator_deposit_token_vault.mint)]
    pub claimant_receive_token_vault : Box<Account<'info, TokenAccount>>, 
    
    // Ticket's token vault owned by the 'signer PDA'. This is the intermediate/temp token account. 
    #[account(mut, 
        constraint = &vault.owner == signer.key)]
    pub vault: Box<Account<'info, TokenAccount>>,
    
    /// CHECK : The Token program
    pub token_program : AccountInfo<'info>, 

    pub vestor : Box<Account<'info, Vestor>>,

    /// CHECK : the System Program 
    pub system_program : AccountInfo<'info>, 


    }

    impl<'info> CreateTicket<'info> {
        pub fn accounts(ctx: &Context<CreateTicket>, bump: u8) -> Result<()> {
       
            let signer_account = Pubkey::create_program_address(
                &[ctx.accounts.ticket.to_account_info().key.as_ref(), 
                ctx.accounts.vestor.tickets_issued.to_string().as_ref(), 
                &[bump]], 
                &ctx.program_id)
                .map_err(|_| ErrorCode::InvalidNonce)?;
  
            if &signer_account != ctx.accounts.signer.to_account_info().key {
                return Err(ErrorCode::InvalidProgramInitializer.into());
            }
  
          Ok(())
        }

      } 


   
#[derive(Accounts)]
pub struct Claim<'info> {  
    // Total 14 accounts are used for 'Claim'
    
    /// CHECK: The 'signer PDA' is not dangerous because of seed + bump contraints
    #[account(
        seeds = [
            ticket.to_account_info().key.as_ref(),
            vestor.tickets_issued.to_string().as_ref()
            ],
        bump = ticket.bump,
    )]
    pub signer : AccountInfo<'info>, 

    ///CHECK : The ticket_creator is not unsafe because some other constraints have been issued to Ticket
    /// which ensure that ticket.creator == *ticket_creator.key
    #[account(mut)]
    pub ticket_creator : AccountInfo<'info>, 

    #[account(
        mut,
        has_one = claimant,
        has_one = claimant_receive_token_vault, 
        constraint = ticket.balance > 0,
        constraint = ticket.balance <= pda_deposit_token_vault.amount,
        constraint = ticket.vault == *pda_deposit_token_vault.to_account_info().key, 
        close = ticket_creator     
    )]
    pub ticket: Box<Account<'info, Ticket>>,

    pub vestor : Box<Account<'info, Vestor>>,

    #[account(
        constraint = pda_deposit_token_vault.owner == signer.key(),
    )]
    pub pda_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = claimant_receive_token_vault.mint == pda_deposit_token_vault.mint,
        constraint = claimant_receive_token_vault.owner == claimant.key(),
    )]
    pub claimant_receive_token_vault: Box<Account<'info, TokenAccount>>,

    ///CHECK: The claimant is not unsafe because some other constraints have been issued to Ticket
    /// which ensure that ticket.claimant == *claimant.key (see => has_one = claimant)
    #[account(signer)]
    pub claimant: AccountInfo<'info>,

    #[account(init, payer = ticket_creator, space = 100)]
    pub chainlink_value: Account<'info, ChainlinkValue>,

    ///CHECK : This account just reads the Sol Price from SOLANA_FEED ADDRESS && which arrived from the Chainlink Program
    pub chainlink_sol_feed: AccountInfo<'info>,

    ///CHECK : This account just reads the ETH Price from ETHEREUM_FEED ADDRESS && which arrived from the Chainlink Program
    pub chainlink_eth_feed: AccountInfo<'info>,

    /// CHECK : This is the Chainlink program's account
    pub chainlink_program: AccountInfo<'info>,

    /// CHECK : This is the Pyth program's account
    pub pyth_account : AccountInfo<'info>,

     /// CHECK : System Program address is already defined
     #[account(address = system_program::ID)]
    pub system_program: AccountInfo<'info>,

    //pub system_program: Program<'info, Token>,
    pub token_program: Program<'info, Token>,
}




#[derive(Accounts)]
pub struct Revoke<'info> {
    //Total 8 accounts used for Revoke

    /// CHECK: The 'signer PDA' is not dangerous because of seed + bump contraints
    #[account(
        seeds = [
            ticket.to_account_info().key.as_ref(),
            vestor.tickets_issued.to_string().as_ref()
            ],
        bump = ticket.bump,
    )]
    pub signer : AccountInfo<'info>, 

    pub vestor : Box<Account<'info, Vestor>>,

    pub ticket_creator: Signer<'info>,

    #[account(
        mut,
        has_one = token_mint,
        constraint = ticket.vault == *pda_deposit_token_vault.to_account_info().key,
        constraint = ticket.balance > 0,
        close = ticket_creator
    )]
    pub ticket: Box<Account<'info, Ticket>>,

    pub token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub ticket_creator_deposit_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = pda_deposit_token_vault.mint == token_mint.key(),
        constraint = pda_deposit_token_vault.owner == signer.key(),
    )]
    pub pda_deposit_token_vault: Box<Account<'info, TokenAccount>>,


    pub token_program: Program<'info, Token>,
}




#[account]
pub struct Vestor {
   
    tickets_issued: u8, // 8
}



#[account]
pub struct Ticket {
   pub token_mint : Pubkey, // 32
    pub owner: Pubkey, // 32
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
    pub vault : Pubkey, //32 
    pub creator_deposit_token_vault : Pubkey, //32
    pub claimant_receive_token_vault : Pubkey, //32
    pub bump : u8, // 8
    

}



#[account]
pub struct ChainlinkValue {
    pub value: i128,
    pub decimals: u32,
}



impl ChainlinkValue {
    pub fn new(value: i128, decimals: u32) -> Self {
        ChainlinkValue { value, decimals }
    }
}

impl std::fmt::Display for ChainlinkValue {
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





impl<'info> Initialize<'info> {
    fn into_transfer_to_ticket_creator_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.contract_owner_deposit_token_vault.to_account_info().clone(),
            to : self.ticket_creator_deposit_token_vault.to_account_info().clone(),
            authority : self.owner.to_account_info().clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'info> From<&mut CreateTicket<'info>> for CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
    fn from(accounts: &mut CreateTicket<'info>) -> Self {
        let cpi_accounts = SetAuthority {
            account_or_mint: accounts
                .ticket_creator_deposit_token_vault
                .to_account_info()
                .clone(),
            current_authority: accounts.owner.to_account_info().clone(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
   }
}


impl<'info> Claim<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint : self.pda_deposit_token_vault.to_account_info().clone(),
            current_authority : self.signer.clone(),
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
            authority : self.signer.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'info> Revoke<'info> {
    fn into_transfer_to_ticket_creator_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.pda_deposit_token_vault.to_account_info().clone(),
            to : self.ticket_creator_deposit_token_vault.to_account_info().clone(),
            authority : self.signer.clone(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}


impl<'info> Revoke<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint : self.pda_deposit_token_vault.to_account_info().clone(),
            current_authority : self.signer.clone(),
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
    #[msg("Ask Admin/Owner to mint more tokens")]
    NotEnoughTokens,
    #[msg("The Program Initializer Address is incorrect")]
    InvalidProgramInitializer,
}


fn not_revoked(ticket: &Ticket) -> Result<()> {
    if ticket.is_revoked {
        return err!(ErrorCode::TicketRevoked);
    }
    Ok(())
}

