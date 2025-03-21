// Prepare and Sign Tx
pub async fn prepare_and_sign_tx(
    account_data: &AccountResponse,
    sender_priv_key: &SigningKey,
    sender_pub_key: &PublicKey,
    msgs: Vec<cosmrs::Any>,
) -> Raw {
    // Get account info
    let account_number = account_data.account.account_number.parse::<u64>().unwrap();
    let sequence = account_data.account.sequence.parse::<u64>().unwrap();
    
    // Create body builder and add messages
    let mut tx_body_builder = BodyBuilder::new();
    tx_body_builder.msgs(msgs);
    let tx_body = tx_body_builder.finish();
    
    // Create signer info
    let signer_info = SignerInfo::single_direct(
        Some(sender_pub_key.clone()),
        sequence
    );

    // Get chain ID and create default gas estimate for simulation
    let cfg = get_config();
    let denom = cfg.get_string("denom").unwrap();
    let chain_id = cfg.get_string("chain_id").unwrap().parse::<cosmrs::tendermint::chain::Id>().unwrap();
    
    // Create initial fee for simulation (will be adjusted after simulation)
    let initial_gas_limit = 2_000_000u64;
    let gas_amount = cosmrs::Coin {
        amount: 0,
        denom: denom.parse().unwrap(),
    };
    
    // Create auth info for simulation
    let auth_info = signer_info.clone().auth_info(Fee::from_amount_and_gas(gas_amount.clone(), initial_gas_limit));
    
    // Create sign doc for simulation
    let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number).unwrap();
    let tx_for_simulation = sign_doc.sign(&sender_priv_key).unwrap();
    
    // Simulate the transaction
    let lcd_url = cfg.get_string("lcd").unwrap();
    let simulate_url = format!("{}cosmos/tx/v1beta1/simulate", lcd_url);
    let client = reqwest::Client::new();
    let tx_bytes = tx_for_simulation.to_bytes().unwrap();
    let tx_json = get_tx_json(tx_bytes);
    
    // Send simulation request
    let response = client.post(simulate_url).json(&tx_json).send().await;
    
    // Process simulation result to determine gas
    match response {
        Ok(response) => {
            let text_response = response.text().await.unwrap_or_default();
            
            match serde_json::from_str::<SimulateResponse>(&text_response) {
                Ok(simulate_response) => {
                    // Calculate gas based on simulation
                    let gas_used = simulate_response.gas_info.gas_used.parse::<u64>().unwrap_or(initial_gas_limit);
                    let safe_gas_limit = (gas_used as f64 * 1.15) as u64; // Apply 15% buffer
                    
                    // Calculate fee amount based on gas price
                    let gas_price = cfg.get_int("gas_price").unwrap_or(100000) as u64;
                    let fee_amount = gas_price * safe_gas_limit / 1_000_000;
                    
                    // Create the final fee
                    let final_gas_amount = cosmrs::Coin {
                        amount: fee_amount as u128,
                        denom: denom.parse().unwrap(),
                    };
                    
                    // Create final auth info with correct gas and fee
                    let final_auth_info = signer_info.auth_info(
                        Fee::from_amount_and_gas(final_gas_amount, safe_gas_limit)
                    );
                    
                    // Create final sign doc and sign transaction
                    let final_sign_doc = SignDoc::new(&tx_body, &final_auth_info, &chain_id, account_number).unwrap();
                    final_sign_doc.sign(&sender_priv_key).unwrap()
                },
                Err(_) => {
                    // Fallback if simulation fails: use default values
                    let default_gas_limit = 300_000u64;
                    let gas_price = cfg.get_int("gas_price").unwrap_or(100000) as u64;
                    let fee_amount = gas_price * default_gas_limit / 1_000_000;
                    
                    let default_gas_amount = cosmrs::Coin {
                        amount: fee_amount as u128,
                        denom: denom.parse().unwrap(),
                    };
                    
                    let default_auth_info = signer_info.auth_info(
                        Fee::from_amount_and_gas(default_gas_amount, default_gas_limit)
                    );
                    
                    let default_sign_doc = SignDoc::new(&tx_body, &default_auth_info, &chain_id, account_number).unwrap();
                    default_sign_doc.sign(&sender_priv_key).unwrap()
                }
            }
        },
        Err(_) => {
            // Fallback if simulation request fails: use default values
            let default_gas_limit = 300_000u64;
            let gas_price = cfg.get_int("gas_price").unwrap_or(100000) as u64;
            let fee_amount = gas_price * default_gas_limit / 1_000_000;
            
            let default_gas_amount = cosmrs::Coin {
                amount: fee_amount as u128,
                denom: denom.parse().unwrap(),
            };
            
            let default_auth_info = signer_info.auth_info(
                Fee::from_amount_and_gas(default_gas_amount, default_gas_limit)
            );
            
            let default_sign_doc = SignDoc::new(&tx_body, &default_auth_info, &chain_id, account_number).unwrap();
            default_sign_doc.sign(&sender_priv_key).unwrap()
        }
    }
}

pub async fn broadcast_tx(tx_json: Value) -> Result<BroadcastTxResponse, reqwest::Error> {
    let cfg = get_config();
    let lcd_url = cfg.get_string("lcd").unwrap();
    let url = format!("{}cosmos/tx/v1beta1/txs", lcd_url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();
    
    client
        .post(url)
        .json(&tx_json)
        .send()
        .await?
        .json::<BroadcastTxResponse>()
        .await
}

pub fn get_tx_json(tx_bytes: Vec<u8>) -> Value {
    let encoder = general_purpose::STANDARD;
    let tx_bytes_base64 = encoder.encode(&tx_bytes);
    json!({
        "tx_bytes": tx_bytes_base64,
        "mode": "BROADCAST_MODE_SYNC"
    })
} 