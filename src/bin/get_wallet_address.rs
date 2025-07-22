use mantra_dex_sdk::{MantraWallet, MantraNetworkConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the test mnemonic from config/test.toml
    let test_mnemonic = "damage spring lunch thrive dumb shuffle enact metal force scissors black sound exit cabin park story eager quote town jacket thought host scorpion buffalo";
    
    // Create wallet with derivation index 0 (same as tests)
    let wallet = MantraWallet::from_mnemonic(test_mnemonic, 0)?;
    
    // Get the network config to get chain ID
    let config = MantraNetworkConfig::default();
    
    // Get wallet address
    let address = wallet.address()?;
    
    println!("🔑 Wallet Address: {}", address);
    println!("📋 Chain ID: {}", config.chain_id);
    println!("💰 Required Asset: factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY");
    println!("📊 Amount Needed: 1000 aUSDY");
    println!();
    println!("To fund this wallet with aUSDY:");
    println!("1. Go to Mantra testnet faucet or DEX");
    println!("2. Send aUSDY tokens to: {}", address);
    println!("3. Minimum required: 1000 aUSDY + gas fees");
    
    Ok(())
}