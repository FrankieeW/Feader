//! Local wallet login commands (EIP-191 Sign-In with Ethereum).

use hex::FromHex;
use siwe::{eip55, generate_nonce, Message, VerificationOpts};

use crate::db::AppDatabase;
use crate::models::{
    CreateWalletLoginChallengeRequest, VerifyWalletLoginRequest, WalletLoginChallenge,
    WalletSession,
};

/// Create a single-use SIWE challenge for local wallet login.
#[tauri::command]
pub fn create_wallet_login_challenge(
    request: CreateWalletLoginChallengeRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<WalletLoginChallenge, String> {
    database.create_wallet_login_challenge(&request.domain, &request.uri, &generate_nonce())
}

/// Return the current verified wallet session.
#[tauri::command]
pub fn get_wallet_session(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Option<WalletSession>, String> {
    database.current_wallet_session()
}

/// Verify a signed SIWE login message and persist the local wallet session.
#[tauri::command]
pub async fn verify_wallet_login(
    request: VerifyWalletLoginRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<WalletSession, String> {
    let message: Message = request
        .message
        .parse()
        .map_err(|error| format!("Invalid SIWE message: {error}"))?;
    let signature = decode_signature(&request.signature)?;
    let verification_opts = VerificationOpts {
        domain: Some(message.domain.clone()),
        nonce: Some(message.nonce.clone()),
        ..Default::default()
    };

    message
        .verify(&signature, &verification_opts)
        .await
        .map_err(|error| format!("Wallet signature verification failed: {error}"))?;

    database.consume_wallet_login_challenge(
        &message.nonce,
        &message.domain.to_string(),
        &message.uri.to_string(),
    )?;

    let address = eip55(&message.address);
    database.save_wallet_session(
        &address,
        message.chain_id,
        &request.message,
        &request.signature,
    )
}

/// Revoke the current local wallet session.
#[tauri::command]
pub fn disconnect_wallet_login(database: tauri::State<'_, AppDatabase>) -> Result<(), String> {
    database.disconnect_wallet_session()
}

fn decode_signature(signature: &str) -> Result<Vec<u8>, String> {
    let signature = signature
        .trim()
        .strip_prefix("0x")
        .unwrap_or(signature.trim());
    let bytes = Vec::from_hex(signature).map_err(|error| error.to_string())?;
    if bytes.len() != 65 {
        return Err("Wallet signature must be 65 bytes".to_string());
    }
    Ok(bytes)
}
