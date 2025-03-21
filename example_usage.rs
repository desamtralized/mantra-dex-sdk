// Example function using the updated wallet implementation
async fn execute_transaction(msgs: Vec<cosmrs::Any>) -> Result<String, Error> {
    // Create or load wallet
    let wallet = MantraWallet::from_mnemonic("your mnemonic phrase here", 0)?;
    let address = wallet.address()?.to_string();
    
    // Fetch account details
    let account_data = fetch_account_details(&address).await;
    let account_number = account_data.account.account_number.parse::<u64>().unwrap();
    let sequence = account_data.account.sequence.parse::<u64>().unwrap();
    
    // Get chain details
    let cfg = get_config();
    let chain_id = cfg.get_string("chain_id").unwrap();
    
    // Create fee (can use default fee or custom)
    let fee = wallet.create_default_fee(300_000)?;
    
    // Sign transaction
    let raw_tx = wallet.sign_tx(
        account_number,
        sequence,
        &chain_id,
        fee,
        msgs,
        None,
        None
    )?;
    
    // Get tx bytes and create json payload
    let tx_bytes = raw_tx.to_bytes()?;
    let tx_json = get_tx_json(tx_bytes);
    
    // Broadcast transaction
    let response = broadcast_tx(tx_json).await?;
    let tx_hash = response.tx_response.txhash;
    
    Ok(tx_hash)
} 