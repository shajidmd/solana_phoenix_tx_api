# Solana Phoenix Transaction API

## Overview
This project provides an API for processing and querying transaction data for the Phoenix Program from the Solana blockchain, with support for OHLC (Open, High, Low, Close) data computation using a ClickHouse database backend.

## Features
- Fetch and parse transactions for the Phoenix Program from the Solana blockchain.
- Insert parsed transaction events into a ClickHouse database.
- Provide OHLC data for queried token pairs via an HTTP API.
- Implement rate limiting and user credit checks.

## Setup Instructions

### Prerequisites
1. **Docker** and **Docker Compose** installed.
2. **Rust** installed (for development and testing).
3. An active internet connection for accessing the Solana RPC and Helius API.

### Installing Dependencies
Dependencies are managed using Docker. Build and set up the project with:
```bash
# Build the Docker images
docker-compose build
```

### Running the Services
To start the associated services (ClickHouse, UI):
```bash
# Start the services
docker-compose up -d
```
### Running the Testcases
To Run Testcases:
```bash
cargo test
```


### Running the Application
To start the API:
```bash
cargo run
```

The API will be accessible at `http://localhost:8080`. ClickHouse UI will be available at `http://localhost:5521`.

### Testing
Run tests locally using the Rust test suite:
```bash
cargo test
```

Ensure you have Rust and its dependencies installed for this.

## API Usage

### Endpoints

#### `GET /ohlc`
Fetch OHLC data for a token pair.

**Query Parameters:**
- `user_id`: The user ID for rate limiting and credit checking.
- `base_token_mint`: The base token mint address.
- `quote_token_mint`: The quote token mint address.
- `start_time`: The start timestamp for the query.
- `end_time`: The end timestamp for the query.
- `interval`: The interval for OHLC data aggregation (`1m`, `1h`, `1d`).

**Example Request:**
```bash
curl "http://localhost:8080/ohlc?user_id=test_user&base_token_mint=base_mint&quote_token_mint=quote_mint&start_time=1634594909&end_time=1634594913&interval=1m"
```

**Response:**
```json
{
    "open": 3388,
    "high": 3392,
    "low": 3390,
    "close": 3392
}
```

### Using ClickHouse
Access ClickHouse via its UI:
```bash
http://localhost:5521
```
Use the default credentials provided in the `docker-compose.yml` file.

## ClickHouse Tables

### `trade_fill_events`
This table stores transaction fill event data from the Solana blockchain.

```sql
CREATE TABLE trade_fill_events (
    market String,
    sequence_number UInt64,
    slot UInt64,
    timestamp Int64, -- Store epoch time as Int64
    signature String,
    signer String,
    event_index UInt64,
    order_sequence_number UInt64,
    maker String,
    taker String,
    price_in_ticks UInt64,
    base_lots_filled UInt64,
    base_lots_remaining UInt64,
    side_filled String,
    is_full_fill Bool,
    base_mint String, -- Used for filtering
    quote_mint String, -- Used for filtering
    base_decimals UInt32,
    quote_decimals UInt32,
    base_atoms_per_raw_base_unit UInt64,
    quote_atoms_per_quote_unit UInt64,
    quote_atoms_per_quote_lot UInt64,
    base_atoms_per_base_lot UInt64,
    tick_size_in_quote_atoms_per_base_unit UInt64,
    num_base_lots_per_base_unit UInt64,
    raw_base_units_per_base_unit UInt32,
    bids_size UInt64,
    asks_size UInt64,
    num_seats UInt64,
    real_data Bool
) 
ENGINE = MergeTree
PARTITION BY (base_mint, quote_mint, toYYYYMM(toDateTime(timestamp))) -- Convert timestamp to DateTime
ORDER BY (base_mint, quote_mint, timestamp) -- Cluster data for efficient range queries
SETTINGS index_granularity = 8192;
```

### `user_credits`
This table tracks user credit usage for rate limiting and billing purposes.

```sql
CREATE TABLE user_credits (
    user_id String,           -- Unique identifier for the user
    credits UInt64,           -- Number of available credits for the user
    last_updated DateTime     -- Timestamp for the last update of the credits
) 
ENGINE = MergeTree
PARTITION BY toYYYYMM(last_updated)
ORDER BY (user_id)
SETTINGS index_granularity = 8192;
```

## Assumptions
- The ClickHouse server is always available and pre-configured.
- Helius API key is set as an environment variable (`HELIUS_API_KEY`).
- Transaction processing is done periodically in parallel with API requests.

## Architecture Decisions
1. **Dockerized Setup**: Ensures reproducibility and easy deployment.
2. **Axum for API**: Lightweight and performant framework suitable for Rust applications.
3. **ClickHouse for OLAP**: High-performance database optimized for analytical queries like OHLC computations.
4. **Rate Limiting**: Implemented in-memory using `tokio::sync::Mutex` for per-user limits.
5. **Helius API**: Used for fetching Solana transaction data.
6. **Modular Design**: Separate modules for `api`, `database`, and `solana` logic to maintain clean code.

## Notable Implementation Details
1. **Rate Limiting**:
   - Each user is limited to 10 requests per minute.
   - Limits reset after 60 seconds.

2. **Credits Check**:
   - Each API call checks and deducts user credits from ClickHouse.
   - Ensures fair resource usage.

3. **Error Handling**:
   - Comprehensive error types defined for transaction parsing and database operations.
   - API responses include meaningful error messages.

4. **Concurrent Tasks**:
   - Fetch and parse transactions concurrently with serving API requests using `tokio::join!`.

5. **Testing**:
   - Mocks implemented for database and API clients to isolate test cases from external dependencies.

## Environment Variables
- `HELIUS_API_KEY`: API key for accessing Helius services.
- `CH_UI_PORT`: Optional port configuration for the ClickHouse UI.

## Development Tips
- Use `cargo fmt` and `cargo clippy` to maintain code quality.
- Logs are stored in the `logs/` directory if volume mapping is enabled in `docker-compose.yml`.

## Troubleshooting
1. **ClickHouse Connection Issues**:
   - Ensure ClickHouse is running and accessible at `http://localhost:8123`.
   - Verify credentials match those in `docker-compose.yml`.

2. **Rate Limit Exceeded**:
   - Wait for 60 seconds for the rate limit to reset.

3. **Helius API Errors**:
   - Confirm the `HELIUS_API_KEY` environment variable is set correctly.
   - Check network connectivity to the Helius RPC endpoint.

