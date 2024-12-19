// solana.rs
use anyhow::Result;
use ellipsis_client::{EllipsisClient, EllipsisClientError};
use solana_sdk::signature::Keypair;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::env;
use std::fmt::Debug;
use std::str::FromStr;
use thiserror::Error;

use crate::database::insert_fill_event;
use solana_client::nonblocking::rpc_client::RpcClient as NonblockingRpcClient;
use solana_client::rpc_client::RpcClient as BlockingRpcClient;

use phoenix_sdk::sdk_client::SDKClient;

pub use phoenix_sdk_core::{
    market_event::{MarketEventDetails, PhoenixEvent},
    sdk_client_core::{MarketMetadata, SDKClientCore},
};

// AI Generated Error Handling -- Start
// Error handling in Rust, particularly with Result and Option types, can be verbose and nuanced and to save time on boilerplate implementations while focusing on the core logic of the assignment.
#[derive(Error, Debug)]
pub enum FetchError {
    // Errors related to public key processing
    #[error("Invalid Pubkey: {0}")]
    InvalidPubkey(String),

    // Errors when fetching signatures
    #[error("Error fetching signatures: {0}")]
    FetchSignaturesError(String),

    // Errors related to converting signature strings
    #[error("Error converting signature: {0}")]
    InvalidSignature(String),

    // Errors during Ellipsis client initialization
    #[error("Error initializing Ellipsis client")]
    ClientInitializationError,

    // Errors during SDK client initialization
    #[error("Error initializing SDK client")]
    SDKClientInitializationError,

    #[error("Failed to create EllipsisClient")]
    EllipsisClientError(#[from] EllipsisClientError),

    #[error("Failed to create SDKClient")]
    SDKClientError(#[from] anyhow::Error),

    // Errors related to database insertion
    #[error("Error inserting fill event into database")]
    InsertionError,

    // Errors during RPC client initialization
    #[error("Error initializing RPC client")]
    RPCInitializationError,
}
// AI Generated Error Handling -- End


pub async fn fetch_and_parse_transactions(
    client: &BlockingRpcClient,
    address: &str,
) -> Result<(), FetchError> {
    let pubkey =
        Pubkey::from_str(address).map_err(|_| FetchError::InvalidPubkey(address.to_string()))?;

    // Fetch signatures for the given address
    let signatures = client.get_signatures_for_address(&pubkey).map_err(|e| {
        FetchError::FetchSignaturesError(format!("Error fetching signatures: {:?}", e))
    })?;
    for signature_infox in signatures {
        // for signature_info in signature_infox {
        let signature_str = signature_infox.signature;

        // Convert signature string to a Signature object
        let signature = Signature::from_str(&signature_str)
            .map_err(|_| FetchError::InvalidSignature(signature_str.clone()))?;

        if let Err(err) = parse_fills(&signature).await {
            // Handle the specific error or propagate it
            match err {
                FetchError::InvalidSignature(_) => eprintln!("Invalid signature: {:?}", err),
                FetchError::InsertionError => eprintln!("Database insertion failed: {:?}", err),
                _ => eprintln!("An error occurred: {:?}", err),
            }
        }
    }
    Ok(()) // Return Ok(()) if everything succeeds
}

pub async fn parse_fills(signature: &Signature) -> Result<(), FetchError> {
    let phoenix_keypair: Keypair = Keypair::new();

    let helius_api_key =
        env::var("HELIUS_API_KEY").map_err(|_| FetchError::RPCInitializationError)?;

    let url_prefix = String::from("https://rpc.helius.xyz/?api-key=");
    let rpc_url: String = format!("{}{}", url_prefix, helius_api_key);

    let client = NonblockingRpcClient::new(rpc_url.to_string());

    let client = EllipsisClient::from_rpc(client, &phoenix_keypair)
        .map_err(|_| FetchError::ClientInitializationError)?;
    let sdk_client: SDKClient = SDKClient::new_from_ellipsis_client_with_all_markets(client)
        .await
        .map_err(|_| FetchError::SDKClientInitializationError)?;

    let events = sdk_client
        .parse_events_from_transaction(signature)
        .await
        .unwrap_or_default();

    for event in events {
        // Filter only Fill events
        if let MarketEventDetails::Fill(..) = event.details {
            // Fetch market metadata for the event's market
            let market_metadata = sdk_client
                .get_market_metadata(&event.market)
                .await
                .unwrap_or_default();

            // Insert the event
            if let Err(err) = insert_fill_event(event, market_metadata).await {
                eprintln!("Failed to insert event: {:?}", err);
            }
        }
    }
    Ok(())
}
