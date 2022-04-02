
const anchor = require('@project-serum/anchor');
const assert = require("assert");
const { Buffer } = require('buffer');
const { AccountLayout } = require("@solana/spl-token");
const { clusterApiUrl, Connection, PublicKey } = require("@solana/web3.js");
const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');


const TokenInstructions = require("@project-serum/serum").TokenInstructions;
const serumCmn = require("@project-serum/common");
const TOKEN_PROGRAM_ID = new anchor.web3.PublicKey(
  TokenInstructions.TOKEN_PROGRAM_ID.toString()
);

const CHAINLINK_PROGRAM_ID = "CaH12fwNTKJAG8PxEvo9R96Zc2j8qNHZaFj8ZW49yZNT";
// SOL/USD feed account
const SOLANA_FEED = "EdWr4ww1Dq82vPe8GFjjcVPo2Qno3Nhn6baCgM3dCy28";
// ETH/USD feed account
const ETHEREUM_FEED = "5zxs8888az8dgB5KauGEFoPuMANtrKtkpFiFRmo3cSa9";
const DIVISOR = 100000000;



describe("vestor-using-anchor-chainlink-master", () => {
  // Specify provider environment. 
  const provider = anchor.Provider.env();
  //Set provider.
  anchor.setProvider(provider);

  const program = anchor.workspace.VestorUsingAnchorChainlinkMaster;

  let mint = null;
  let claimantReceiveTokenVault = null;
  let grantorDepositTokenVault = null;
  let claimant = anchor.web3.Keypair.generate();


  it("Initialize the test state and Creates All Accounts", async () => {

    //Discover/find the 'vestor PDA' publicKey through off-chain computation using Pubkey::find_program_address
    const [vestor, nonce] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("vesting__init"), provider.wallet.publicKey.toBuffer()],
      program.programId
    );

    mint = await createMint(provider);
    console.log("Mint Info : ", await getMintInfo(provider, mint));

    grantorDepositTokenVault = await createTokenAccount(provider, mint, provider.wallet.publicKey);
    console.log("Grantor Deposit Token vault created : ", await getTokenAccount(provider, grantorDepositTokenVault));

    claimantReceiveTokenVault = await createTokenAccount(provider, mint, claimant.publicKey);
    console.log("Claimant Receive Token vault created : ", await getTokenAccount(provider, claimantReceiveTokenVault));

    // #[access_control] will check if expected_publicKey == actual_publicKey and therefore restrict anybody else's
    // access to use this 'Initialize' fn..
    const mint_to_tx = await program.rpc.initializeTestState(new anchor.BN(10000e8), new anchor.BN(nonce), {
      accounts: {
        vestor: vestor,
        grantor: provider.wallet.publicKey,
        tokenMint: mint,
        grantorDepositTokenVault: grantorDepositTokenVault,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
    });

    console.log("Minting 10000 tokens to Grantor Token Vault, here's the signature : ", mint_to_tx);

    //Now lets check if the Grantor(public Key = provider.wallet.publicKey == solana-keygen pubkey ) 
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

  it("Creates a new Ticket for Vesting with Schedule", async () => {

    // Discover/find the 'ticket' publicKey based on vestor.key.as_ref()
    const [ticket, bump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from(anchor.utils.bytes.utf8.encode("init_____ticket"))],
      program.programId
    );

    //Now lets register the Ticket Account with a specified vesting period
    await program.rpc.createTicket(
      claimant.publicKey,
      new anchor.BN(50),
      new anchor.BN(65),
      new anchor.BN(5000),
      new anchor.BN(bump),
      false, {
      accounts: {
        ticket: ticket,
        tokenMint: mint,
        grantorDepositTokenVault: grantorDepositTokenVault,
        grantor: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,

      }
    });


    const ticketAccount = await program.account.ticket.fetch(ticket);
    console.log("Ticket Account Created :", ticketAccount);
    console.log("Ticket Account PublicKey : ", ticketAccount.key.toBase58());

  });



});


async function createMint(provider, authority) {
  if (authority === undefined) {
    authority = provider.wallet.publicKey;
  }
  const mint = anchor.web3.Keypair.generate();
  const instructions = await createMintInstructions(
    provider,
    authority,
    mint.publicKey
  );

  const tx = new anchor.web3.Transaction();
  tx.add(...instructions);

  await provider.send(tx, [mint]);

  return mint.publicKey;
}

async function createMintInstructions(provider, authority, mint) {
  let instructions = [
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey: mint,
      space: 82,
      lamports: await provider.connection.getMinimumBalanceForRentExemption(82),
      programId: TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeMint({
      mint: mint,//
      decimals: 0,
      mintAuthority: authority,
    }),
  ];
  return instructions;
}


async function createTokenAccount(provider, mint, owner) {
  const vault = anchor.web3.Keypair.generate();
  const tx = new anchor.web3.Transaction();
  tx.add(
    ...(await createTokenAccountInstrs(provider, vault.publicKey, mint, owner))
  );
  await provider.send(tx, [vault]);
  return vault.publicKey;
}

async function createTokenAccountInstrs(
  provider,
  newAccountPubkey,
  mint,
  owner,
  lamports
) {
  if (lamports === undefined) {
    lamports = await provider.connection.getMinimumBalanceForRentExemption(165);
  }
  let instructions = [
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey,
      space: 165,
      lamports,
      programId: TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeAccount({
      account: newAccountPubkey,
      mint: mint,
      owner: owner,
    }),
  ];
  return instructions;
}


async function getTokenAccount(provider, addr) {
  return await serumCmn.getTokenAccount(provider, addr);
}

async function getMintInfo(provider, mintAddr) {
  return await serumCmn.getMintInfo(provider, mintAddr);
}


