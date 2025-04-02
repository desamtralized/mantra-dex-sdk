use mantra_dex_sdk::{MantraDexClient, MantraWallet, NetworkConstants, MantraNetworkConfig};

use crate::config::TuiConfig;
use crate::error::{Result, TuiError};

/// Create a client from config
pub async fn create_client(config: &TuiConfig) -> Result<MantraDexClient> {
    // Check if there's an active wallet
    let _wallet_name = config
        .active_wallet
        .as_ref()
        .ok_or_else(|| TuiError::Command("No active wallet selected".to_string()))?;

    // Check if password is available
    let _password = config
        .get_session_password()
        .ok_or_else(|| TuiError::Command("Wallet password not available. Please unlock the wallet first".to_string()))?;

    // Load network constants
    let network_constants = load_network_constants(config)?;

    // Create wallet (this would need actual implementation to decrypt and load wallet)
    // For now, we're just using a placeholder
    let _wallet = MantraWallet::from_mnemonic("test mnemonic", 0, &network_constants)
        .map_err(|e| TuiError::Wallet(e.to_string()))?;

    // Create network config
    let network_config = MantraNetworkConfig {
        network_name: network_constants.network_name,
        network_id: network_constants.network_id,
        rpc_url: network_constants.default_rpc,
        gas_price: network_constants.default_gas_price,
        gas_adjustment: network_constants.default_gas_adjustment,
        native_denom: network_constants.native_denom.clone(),
        contracts: Default::default(), // Assuming contracts field exists and can be defaulted
    };

    // Create client with network config
    // Since we can't set the wallet after client creation, we need to pass it while creating
    // the client. As a workaround, we can create a mock client here and in real code,
    // implement a proper solution.
    let client = MantraDexClient::new(network_config)
        .await
        .map_err(|e| TuiError::Sdk(e.to_string()))?;

    Ok(client)
}

/// Load network constants from config
pub fn load_network_constants(config: &TuiConfig) -> Result<NetworkConstants> {
    let network = &config.network;
    
    Ok(NetworkConstants {
        network_name: network.network_name.clone(),
        network_id: network.network_id.clone(),
        default_rpc: network.rpc_url.clone(),
        default_gas_price: network.gas_price,
        default_gas_adjustment: network.gas_adjustment,
        native_denom: network.native_denom.clone(),
    })
} 