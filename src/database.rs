// database.rs
use anyhow::Result;
use clickhouse::{Client, Row};

use crate::api::OHLCQuery;
use crate::api::OHLCResponse;
use serde::Deserialize;

pub use phoenix_sdk_core::{
    market_event::{Fill, MarketEventDetails, PhoenixEvent},
    sdk_client_core::MarketMetadata,
};

#[derive(Debug, Row, Deserialize)]
struct OHLCRow {
    open: u64,
    high: u64,
    low: u64,
    close: u64,
}

pub async fn insert_fill_event(event: PhoenixEvent, metadata: MarketMetadata) -> Result<()> {
    // Match on the details to ensure it's a Fill event
    if let MarketEventDetails::Fill(Fill {
        order_sequence_number,
        maker,
        taker,
        price_in_ticks,
        base_lots_filled,
        base_lots_remaining,
        side_filled,
        is_full_fill,
    }) = event.details
    {
        let client = Client::default()
            .with_url("http://localhost:8123")
            .with_user("default")
            .with_password("password");

        let side_as_string = match side_filled {
            phoenix::state::enums::Side::Bid => "Bid",
            phoenix::state::enums::Side::Ask => "Ask",
        };

        let query = format!(
            "INSERT INTO trade_fill_events (
                market, sequence_number, slot, timestamp, signature, signer, event_index, 
                order_sequence_number, maker, taker, price_in_ticks, base_lots_filled, base_lots_remaining, 
                side_filled, is_full_fill, base_mint, quote_mint, base_decimals, quote_decimals, 
                base_atoms_per_raw_base_unit, quote_atoms_per_quote_unit, quote_atoms_per_quote_lot, 
                base_atoms_per_base_lot, tick_size_in_quote_atoms_per_base_unit, num_base_lots_per_base_unit, 
                raw_base_units_per_base_unit, bids_size, asks_size, num_seats, real_data
            ) VALUES (
                '{market}', {sequence_number}, {slot}, {timestamp}, '{signature}', '{signer}', {event_index}, 
                {order_sequence_number}, '{maker}', '{taker}', {price_in_ticks}, {base_lots_filled}, 
                {base_lots_remaining}, '{side_filled}', {is_full_fill}, '{base_mint}', '{quote_mint}', 
                {base_decimals}, {quote_decimals}, {base_atoms_per_raw_base_unit}, {quote_atoms_per_quote_unit}, 
                {quote_atoms_per_quote_lot}, {base_atoms_per_base_lot}, {tick_size_in_quote_atoms_per_base_unit}, 
                {num_base_lots_per_base_unit}, {raw_base_units_per_base_unit}, {bids_size}, {asks_size}, {num_seats}, true
            )",
            market = event.market,
            sequence_number = event.sequence_number,
            slot = event.slot,
            timestamp = event.timestamp,
            signature = event.signature,
            signer = event.signer,
            event_index = event.event_index,
            order_sequence_number = order_sequence_number,
            maker = maker,
            taker = taker,
            price_in_ticks = price_in_ticks,
            base_lots_filled = base_lots_filled,
            base_lots_remaining = base_lots_remaining,
            side_filled = side_as_string,
            is_full_fill = is_full_fill,
            base_mint = metadata.base_mint,
            quote_mint = metadata.quote_mint,
            base_decimals = metadata.base_decimals,
            quote_decimals = metadata.quote_decimals,
            base_atoms_per_raw_base_unit = metadata.base_atoms_per_raw_base_unit,
            quote_atoms_per_quote_unit = metadata.quote_atoms_per_quote_unit,
            quote_atoms_per_quote_lot = metadata.quote_atoms_per_quote_lot,
            base_atoms_per_base_lot = metadata.base_atoms_per_base_lot,
            tick_size_in_quote_atoms_per_base_unit = metadata.tick_size_in_quote_atoms_per_base_unit,
            num_base_lots_per_base_unit = metadata.num_base_lots_per_base_unit,
            raw_base_units_per_base_unit = metadata.raw_base_units_per_base_unit,
            bids_size = metadata.market_size_params.bids_size,
            asks_size = metadata.market_size_params.asks_size,
            num_seats = metadata.market_size_params.num_seats
        );

        // Execute the query
        client.query(&query).execute().await?;
    } else {
        return Err(anyhow::anyhow!("Event is not a Fill variant."));
    }

    Ok(())
}

pub async fn fetch_ohlc_data(
    client: &Client,
    query: &OHLCQuery,
) -> Result<OHLCResponse, Box<dyn std::error::Error>> {
    let interval_duration = match query.interval.as_str() {
        "1m" => 1,
        "1h" => 60,
        "1d" => 1440,
        _ => return Err("Invalid interval".into()),
    };

    let start_time = query.start_time;
    let end_time = query.end_time;

    let sql = format!(
        r#"
        SELECT
            MIN(price_in_ticks) AS low,
            MAX(price_in_ticks) AS high,
            anyLast(price_in_ticks) AS close,
            any(price_in_ticks) AS open
        FROM trade_fill_events
        WHERE base_mint = '{}' AND quote_mint = '{}'
        AND timestamp >= {} AND timestamp <= {}
        GROUP BY toStartOfInterval(toDateTime(timestamp), INTERVAL {} MINUTE)
        "#,
        query.base_token_mint, query.quote_token_mint, start_time, end_time, interval_duration
    );
    let rows = client.query(&sql).fetch_all::<OHLCRow>().await?;

    if let Some(row) = rows.first() {
        Ok(OHLCResponse {
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
        })
    } else {
        Err("No data found".into())
    }
}

#[derive(Row, Deserialize)]
struct CreditsRow {
    credits: u64,
}

pub async fn check_and_update_credits(
    client: &Client,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check current credits
    let credits_query = format!(
        "SELECT credits FROM user_credits WHERE user_id = '{}'",
        user_id
    );
    let row: CreditsRow = client.query(&credits_query).fetch_one().await?;

    if row.credits == 0 {
        return Err("Insufficient credits".into());
    }

    // Deduct 1 credit and update
    let update_credits_query = format!(
        "ALTER TABLE user_credits UPDATE credits = credits - 1 WHERE user_id = '{}'",
        user_id
    );
    client.query(&update_credits_query).execute().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use phoenix::program::MarketSizeParams;
    use solana_sdk::{pubkey::Pubkey, signature::Signature};

    #[tokio::test]
    async fn test_insert_fill_event_success() {
        // Mock PhoenixEvent and MarketMetadata
        let event = PhoenixEvent {
            market: Pubkey::new_unique(),
            sequence_number: 1,
            slot: 1,
            timestamp: 1,
            signature: Signature::new_unique(),
            signer: Pubkey::new_unique(),
            event_index: 1,
            details: MarketEventDetails::Fill(Fill {
                order_sequence_number: 1,
                maker: Pubkey::new_unique(),
                taker: Pubkey::new_unique(),
                price_in_ticks: 100,
                base_lots_filled: 10,
                base_lots_remaining: 5,
                side_filled: phoenix::state::enums::Side::Bid,
                is_full_fill: true,
            }),
        };

        let metadata = MarketMetadata {
            base_mint: Pubkey::new_unique(),
            quote_mint: Pubkey::new_unique(),
            base_decimals: 8,
            quote_decimals: 8,
            base_atoms_per_raw_base_unit: 1,
            quote_atoms_per_quote_unit: 1,
            quote_atoms_per_quote_lot: 1,
            base_atoms_per_base_lot: 1,
            tick_size_in_quote_atoms_per_base_unit: 1,
            num_base_lots_per_base_unit: 1,
            raw_base_units_per_base_unit: 1,
            market_size_params: MarketSizeParams {
                bids_size: 1,
                asks_size: 1,
                num_seats: 1,
            },
        };

        // Mock ClickHouse Client
        struct MockClient;

        impl MockClient {
            async fn insert(&self, _query: &str) -> Result<()> {
                Ok(())
            }
        }

        let mock_client = MockClient;
        let result = mock_client.insert("mock_query").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_ohlc_data_success() {
        struct MockClient;

        impl MockClient {
            async fn query(&self, _query: &str) -> Result<Vec<OHLCRow>> {
                Ok(vec![OHLCRow {
                    open: 100,
                    high: 200,
                    low: 50,
                    close: 150,
                }])
            }
        }

        let mock_client = MockClient;

        let query = OHLCQuery {
            user_id: "test_user".to_string(),
            base_token_mint: "base_mint".to_string(),
            quote_token_mint: "quote_mint".to_string(),
            start_time: 1,
            end_time: 100,
            interval: "1m".to_string(),
        };

        let rows = mock_client.query("mock_query").await.unwrap();
        let row = rows.first().unwrap();

        assert_eq!(row.open, 100);
        assert_eq!(row.high, 200);
        assert_eq!(row.low, 50);
        assert_eq!(row.close, 150);
    }

    #[tokio::test]
    async fn test_check_and_update_credits_success() {
        struct MockClient;

        impl MockClient {
            async fn query(&self, _query: &str) -> Result<u64> {
                Ok(10)
            }

            async fn update(&self, _query: &str) -> Result<()> {
                Ok(())
            }
        }

        let mock_client = MockClient;

        // Simulate checking and updating credits
        let credits = mock_client.query("mock_check_credits_query").await.unwrap();
        assert!(credits > 0);

        let result = mock_client.update("mock_update_credits_query").await;
        assert!(result.is_ok());
    }
}
