// main.rs
use crate::api::start_api_server;
use crate::solana::fetch_and_parse_transactions;
use dotenv::dotenv;
use solana_client::rpc_client::RpcClient;
use std::env;
use tokio;

mod api;
mod database;
mod solana;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from the .env file
    dotenv().ok();

    // Initialize Solana RPC Client
    let helius_api_key = env::var("HELIUS_API_KEY")?;
    let rpc_url = format!("https://rpc.helius.xyz/?api-key={}", helius_api_key);
    let client = RpcClient::new(rpc_url.to_string());

    // Address of the Phoenix DEX Program
    let phoenix_program_id = "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY";

    // Call the fetch_and_parse_transactions function
    // Run both tasks concurrently
    let fetch_and_parse_task = tokio::spawn(async move {
        match fetch_and_parse_transactions(&client, phoenix_program_id).await {
            Ok(_) => println!("Transactions fetched and processed successfully."),
            Err(err) => eprintln!("Error fetching or processing transactions: {:?}", err),
        }
    });

    // Start the API server
    let start_api_server_task = tokio::spawn(async {
        if let Err(err) = start_api_server().await {
            eprintln!("Error starting API server: {:?}", err);
        }
    });

    // Wait for either task to fail (ideally, they should run forever)
    let (fetch_result, api_result) = tokio::join!(fetch_and_parse_task, start_api_server_task);

    // Handle unexpected task exits
    if let Err(err) = fetch_result {
        eprintln!(
            "fetch_and_parse_transactions task exited unexpectedly: {:?}",
            err
        );
    }
    if let Err(err) = api_result {
        eprintln!("start_api_server task exited unexpectedly: {:?}", err);
    }

    Ok(())
}
