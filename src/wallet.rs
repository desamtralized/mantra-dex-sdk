use bip32::DerivationPath;
use bip39::Mnemonic;
use cosmrs::{
    crypto::secp256k1::{Signature, SigningKey},
    crypto::PublicKey,
    tx::{BodyBuilder, Fee, Raw, SignDoc, SignerInfo},
    AccountId, Coin as CosmosCoin, Denom,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{error::Error, NetworkConstants};

/// HD Path prefix for Cosmos chains (BIP-44)
const HD_PATH_PREFIX: &str = "m/44'/118'/0'/0/";

/// Mantra wallet for managing key and signing transactions
pub struct MantraWallet {
    /// The signing account
    signing_account: cosmrs::crypto::secp256k1::SigningKey,
    /// The account prefix (mantra)
    account_prefix: String,
    /// The network constants
    network_constants: NetworkConstants,
}

/// Wallet info that can be serialized safely
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    /// The wallet address
    pub address: String,
    /// The public key as hex
    pub public_key: String,
}

impl MantraWallet {
    /// Create a new wallet from a mnemonic
    pub fn from_mnemonic(mnemonic: &str, account_index: u32, network_constants: &NetworkConstants) -> Result<Self, Error> {
        let mnemonic = Mnemonic::from_str(mnemonic)
            .map_err(|e| Error::Wallet(format!("Invalid mnemonic: {}", e)))?;

        let seed = mnemonic.to_seed("");
        let seed = bip32::Seed::new(seed);

        let path = format!("{}{}", HD_PATH_PREFIX, account_index);
        let path = DerivationPath::from_str(&path)
            .map_err(|e| Error::Wallet(format!("Invalid derivation path: {}", e)))?;

        let derived_key = bip32::XPrv::derive_from_path(seed.as_bytes(), &path)
            .map_err(|e| Error::Wallet(format!("Key derivation error: {}", e)))?;

        let derived_key_bytes = derived_key.to_bytes();
        let signing_account = SigningKey::from_slice(&derived_key_bytes)
            .map_err(|e| Error::Wallet(format!("Failed to create signing account: {}", e)))?;

        Ok(Self {
            signing_account,
            account_prefix: "mantra".to_string(),
            network_constants: network_constants.clone(),
        })
    }

    /// Generate a new random wallet
    pub fn generate(network_constants: &NetworkConstants) -> Result<(Self, String), Error> {
        use rand::{thread_rng, RngCore};

        // Generate 16 bytes (128 bits) of entropy for a 12-word mnemonic
        let mut entropy = [0u8; 16];
        thread_rng().fill_bytes(&mut entropy);

        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| Error::Wallet(format!("Failed to generate mnemonic: {}", e)))?;

        let phrase = mnemonic.to_string();
        let wallet = Self::from_mnemonic(&phrase, 0, &network_constants)?;

        Ok((wallet, phrase))
    }

    /// Get the wallet's address
    pub fn address(&self) -> Result<AccountId, Error> {
        self.signing_account
            .public_key()
            .account_id(&self.account_prefix)
            .map_err(|e| Error::Wallet(format!("Failed to get account ID: {}", e)))
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        self.signing_account.public_key()
    }

    /// Get access to the signing key
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_account
    }

    /// Sign a transaction doc
    pub fn sign_doc(&self, sign_doc: SignDoc) -> Result<Signature, Error> {
        let sign_doc_bytes = sign_doc
            .into_bytes()
            .map_err(|e| Error::Wallet(format!("Failed to convert sign doc to bytes: {}", e)))?;
        let signature = self
            .signing_account
            .sign(&sign_doc_bytes)
            .map_err(|e| Error::Wallet(format!("Signing error: {}", e)))?;
        Ok(signature)
    }

    /// Prepare and sign a transaction with body and auth info
    pub fn sign_tx(
        &self,
        account_number: u64,
        sequence: u64,
        chain_id: &str,
        fee: Fee,
        msgs: Vec<cosmrs::Any>,
        timeout_height: Option<u32>,
        memo: Option<String>,
    ) -> Result<Raw, Error> {
        // Create body builder and add messages
        let mut body_builder = BodyBuilder::new();
        body_builder.msgs(msgs);

        // Add memo if provided
        if let Some(memo_text) = memo {
            body_builder.memo(memo_text);
        }

        // Add timeout height if provided
        if let Some(height) = timeout_height {
            body_builder.timeout_height(height);
        }

        let tx_body = body_builder.finish();

        // Create signer info with sequence number
        let signer_info = SignerInfo::single_direct(Some(self.public_key()), sequence);

        // Create auth info with fee and signer info
        let auth_info = signer_info.auth_info(fee);

        // Create sign doc
        let chain_id = cosmrs::tendermint::chain::Id::from_str(chain_id)
            .map_err(|e| Error::Wallet(format!("Invalid chain ID: {}", e)))?;

        let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number)
            .map_err(|e| Error::Wallet(format!("Failed to create sign doc: {}", e)))?;

        // Sign the transaction
        sign_doc
            .sign(&self.signing_account)
            .map_err(|e| Error::Wallet(format!("Failed to sign transaction: {}", e)))
    }

    /// Get wallet info
    pub fn info(&self) -> WalletInfo {
        WalletInfo {
            address: self.address().unwrap().to_string(),
            public_key: self.signing_account.public_key().to_string(),
        }
    }

    /// Create a fee object for transactions
    pub fn create_fee(&self, amount: u64, gas_limit: u64, denom: &str) -> Result<Fee, Error> {
        let denom =
            Denom::from_str(denom).map_err(|e| Error::Wallet(format!("Invalid denom: {}", e)))?;

        let coin = CosmosCoin {
            amount: amount.into(),
            denom,
        };

        Ok(Fee::from_amount_and_gas(coin, gas_limit))
    }

    /// Create a default fee using the native token
    pub fn create_default_fee(&self, gas_limit: u64) -> Result<Fee, Error> {
        let gas_price = self.compute_gas_price()?;
        let amount = (gas_limit as f64 * gas_price) as u64;
        self.create_fee(amount, gas_limit, &self.network_constants.native_denom)
    }

    /// Calculate gas price with adjustment
    fn compute_gas_price(&self) -> Result<f64, Error> {
        Ok(self.network_constants.default_gas_price * self.network_constants.default_gas_adjustment)
    }
}
