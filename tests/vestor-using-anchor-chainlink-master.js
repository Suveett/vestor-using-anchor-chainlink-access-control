const anchor = require('@project-serum/anchor');
const assert = require("assert");
const { Buffer } = require('buffer');
const { AccountLayout } = require("@solana/spl-token");
const { clusterApiUrl, Connection, PublicKey, SystemProgram } = require("@solana/web3.js");
const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');


const TokenInstructions = require("@project-serum/serum").TokenInstructions;
const serumCmn = require("@project-serum/common");
const TOKEN_PROGRAM_ID = new anchor.web3.PublicKey(
  TokenInstructions.TOKEN_PROGRAM_ID.toString()
);

const CHAINLINK_PROGRAM_ID = new anchor.web3.PublicKey("CaH12fwNTKJAG8PxEvo9R96Zc2j8qNHZaFj8ZW49yZNT");
// SOL/USD feed account
const CHAINLINK_SOLANA_FEED = new anchor.web3.PublicKey("EdWr4ww1Dq82vPe8GFjjcVPo2Qno3Nhn6baCgM3dCy28");
// ETH/USD feed account
const CHAINLINK_ETHEREUM_FEED = new anchor.web3.PublicKey("5zxs8888az8dgB5KauGEFoPuMANtrKtkpFiFRmo3cSa9");
const DIVISOR = 100000000;

// This is an a account on devnet that contains data for SOL price.
// THis is the link that contains all the devnet accounts https://pyth.network/developers/accounts/?cluster=devnet#
let PYTH_SOL_PRICE_ACCOUNT = new anchor.web3.PublicKey("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")
let PYTH_SOL_SYMBOL_ACCOUNT = new anchor.web3.PublicKey("3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E");



describe("vestor-using-anchor-chainlink-master", () => {
  // Specify provider environment. 
  const provider = anchor.Provider.env();
  //Set provider.
  anchor.setProvider(provider);

  const program = anchor.workspace.VestorUsingAnchorChainlinkMaster;

  let mint = null;
  let claimantReceiveTokenVault = null;
  let contractOwnerDepositTokenVault = null;
  let ticketCreatorDepositTokenVault = null;
  let vault = anchor.web3.Keypair.generate();
  let claimant = anchor.web3.Keypair.generate();
  let vestor = anchor.web3.Keypair.generate();
  let ticket = anchor.web3.Keypair.generate();
  let ticketSigner = null;

  it("Sets up initial test state", async () => {
    const [_mint, _contractOwnerDepositTokenVault] = await serumCmn.createMintAndVault(
      program.provider,
      new anchor.BN(1000000)
    );
    mint = _mint;
    contractOwnerDepositTokenVault = _contractOwnerDepositTokenVault;

    ticketCreatorDepositTokenVault = await serumCmn.createTokenAccount(
      program.provider,
      mint,
      program.provider.wallet.publicKey
    );

    claimantReceiveTokenVault = await serumCmn.createTokenAccount(
      program.provider,
      mint,
      claimant.publicKey,
    );

  });

  it("Initialize the Contract", async () => {


    await program.rpc.initialize(new anchor.BN(10000), {
      accounts: {
        vestor: vestor.publicKey,
        contractOwnerDepositTokenVault: contractOwnerDepositTokenVault,
        owner: provider.wallet.publicKey,
        tokenMint: mint,
        ticketCreatorDepositTokenVault: ticketCreatorDepositTokenVault,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,

      },
      signers: [vestor],

    });

    const vestorAccount = await program.account.vestor.fetch(vestor.publicKey);
    console.log("Vestor Account Created :", vestorAccount);


    //Now lets check if the TicketCreator(whic also happens to be provider.wallet.publicKey) 
    //actually holds these 10000 'mint' tokens on the Blockchain

    const tokenAccounts = await connection.getTokenAccountsByOwner(
      new PublicKey('CASTmiUtHG1FXTs8hJYyJmXJKfhir8CkqoxDFyfwLmTt'), // this is my solana-keygen pubkey, you may add yours.
      {
        programId: TOKEN_PROGRAM_ID,
      }
    );

    console.log("Token                                         Balance");
    console.log("------------------------------------------------------------");
    tokenAccounts.value.forEach((e) => {
      const accountInfo = AccountLayout.decode(e.account.data);
      console.log(`${new PublicKey(accountInfo.mint)}   ${accountInfo.amount}`);
    });

  });

  it("Creates Tickets", async () => {

    const vestorAccount = await program.account.vestor.fetch(vestor.publicKey);
    let current_id = vestorAccount.ticketsIssued.toString();
    // Discover/find the 'ticket' publicKey based on ticket.key.as_ref() and vestor.tickets_issued
    const [_ticketSigner, bump] = await anchor.web3.PublicKey.findProgramAddress(
      [ticket.publicKey.toBuffer(), current_id],
      program.programId
    );

    ticketSigner = _ticketSigner;


    await program.rpc.createTicket(
      claimant.publicKey,
      new anchor.BN(50),
      new anchor.BN(65),
      new anchor.BN(1000),
      new anchor.BN(bump),
      false, {
      accounts: {
        ticket: ticket.publicKey,
        owner: provider.wallet.publicKey,
        signer: ticketSigner,
        ticketCreatorDepositTokenVault: ticketCreatorDepositTokenVault,
        vault: vault.publicKey,
        claimantReceiveTokenVault: claimantReceiveTokenVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        vestor: vestor.publicKey,
        systemProgram: SystemProgram.programId,
        tokenMint: mint,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,

      },
      signers: [ticket, vault],
      instructions: [
        await program.account.ticket.createInstruction(ticket, 300),
        ...(await serumCmn.createTokenAccountInstrs(
          provider,
          vault.publicKey,
          mint,
          ticketSigner
        )),
      ],
    });


    const ticketAccount = await program.account.ticket.fetch(ticket.publicKey);
    console.log("Ticket Account Created :", ticketAccount);
    console.log("Ticket Account PublicKey : ", ticketAccount.key.toBase58());

  });


});


