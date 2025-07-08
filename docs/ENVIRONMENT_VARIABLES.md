# Yagna Environment Variables Documentation

This document provides a comprehensive reference for all environment variables used in the Yagna node configuration.

## Core Service Configuration

### Yagna Service
- **`YAGNA_DATADIR`** - Application working directory path
  - Default: Platform-specific (`.` in template)
  - Purpose: Root directory for all Yagna data storage

- **`YAGNA_API_URL`** - Default HOST:PORT for all REST APIs
  - Default: `http://127.0.0.1:7465`
  - Purpose: Base URL for all API endpoints

- **`YAGNA_MARKET_URL`** - Market API specific URL
  - Default: Derived from `YAGNA_API_URL`
  - Purpose: Override for Market service API endpoint

- **`YAGNA_ACTIVITY_URL`** - Activity API specific URL
  - Default: Derived from `YAGNA_API_URL`
  - Purpose: Override for Activity service API endpoint

- **`YAGNA_PAYMENT_URL`** - Payment API specific URL
  - Default: Derived from `YAGNA_API_URL`
  - Purpose: Override for Payment service API endpoint

- **`YAGNA_LOG_DIR`** - Directory for log files
  - Default: Uses `YAGNA_DATADIR` if unset, disabled if empty string
  - Purpose: Location for Yagna log storage

- **`YAGNA_API_ALLOW_ORIGIN`** - CORS allowed origins for API
  - Purpose: Configure cross-origin resource sharing

- **`YAGNA_HTTP_WORKERS`** - Number of HTTP workers
  - Default: Number of CPU cores (clamped between 1-256)
  - Purpose: Configure HTTP server worker threads

- **`YAGNA_APPKEY`** - Service REST API application key token
  - Required: Yes (generated during setup)
  - Purpose: Authentication token for API access

- **`YAGNA_TRACE_DB_LOCKS`** - Enable database lock tracing
  - Values: `"1"` to enable
  - Purpose: Debug database locking issues

### Service Bus (GSB)
- **`GSB_URL`** - Host and port for internal Service Bus
  - Default: `tcp://127.0.0.1:7464`
  - Purpose: Internal communication backbone

- **`GSB_PING_TIMEOUT`** - Seconds between GSB heartbeats
  - Default: `60`
  - Purpose: Service health monitoring interval

## Network Configuration

### Network Type
- **`YA_NET_TYPE`** - Network type selection
  - Values: `"central"` or `"hybrid"`
  - Default: `"central"` (varies by build features)
  - Purpose: Choose networking architecture

### P2P Networking
- **`YA_NET_BIND_URL`** - P2P client bind address
  - Default: `udp://0.0.0.0:11500`
  - Purpose: Local P2P networking endpoint

- **`YA_NET_RELAY_HOST`** - Relay server address
  - Default: `127.0.0.1:7464`
  - Purpose: Relay server for hybrid networking

- **`YA_NET_BROADCAST_SIZE`** - Broadcast size
  - Default: `5`
  - Purpose: Network broadcast configuration

- **`YA_NET_PUB_BROADCAST_SIZE`** - Public broadcast size
  - Default: `30`
  - Purpose: Public network broadcast size

- **`YA_NET_SESSION_EXPIRATION`** - Session expiration time
  - Default: `15s`
  - Purpose: P2P session timeout

- **`YA_NET_SESSION_REQUEST_TIMEOUT`** - Session request timeout
  - Default: `3s`
  - Purpose: P2P session establishment timeout

### Central Network
- **`CENTRAL_NET_HOST`** - Central net host address
  - Default: `3.249.139.167:7464`
  - Purpose: Central network coordination server

## Market Service

- **`YAGNA_MARKET_AGREEMENT_STORE_DAYS`** - Agreement cleanup period
  - Default: `90` days
  - Purpose: Database cleanup for old agreements

- **`YAGNA_MARKET_EVENT_STORE_DAYS`** - Event cleanup period
  - Default: `1` day
  - Purpose: Database cleanup for market events

- **`SUBNET`** - Subnetwork identifier
  - Purpose: Filter nodes by network identifier (e.g., `"testnet"`)

## Payment Service

### General Payment Configuration
- **`PAYMENT_SHUTDOWN_TIMEOUT_SECS`** - Payment service shutdown timeout
  - Default: `10` seconds
  - Purpose: Graceful shutdown timeout

- **`ACCOUNT_LIST`** - Path to accounts.json file
  - Default: `${YAGNA_DATADIR}/accounts.json`
  - Purpose: Account configuration file location

- **`DEBIT_NOTE_INTERVAL`** - Debit note interval duration
  - Purpose: Payment processing frequency

### ERC20 Driver
- **`ERC20_SENDOUT_INTERVAL_SECS`** - Payment gathering interval
  - Default: `10` seconds
  - Purpose: Batch payment processing frequency

### Blockchain Node Configuration
- **`MAINNET_GETH_ADDR`** - Ethereum mainnet node addresses
  - Default: DNS lookup from `mainnet.rpc-node.dev.golem.network`
  - Example: `https://geth.golem.network:55555`

- **`GOERLI_GETH_ADDR`** - Goerli testnet node addresses
  - Example: `https://rpc.ankr.com/eth_goerli`

- **`HOLESKY_GETH_ADDR`** - Holesky testnet node addresses
  - Example: `https://rpc.ankr.com/eth_holesky`

- **`POLYGON_GETH_ADDR`** - Polygon mainnet node addresses
  - Example: `https://bor.golem.network,https://polygon-rpc.com`

- **`MUMBAI_GETH_ADDR`** - Mumbai testnet node addresses
  - Example: `https://matic-mumbai.chainstacklabs.com`

### Contract Addresses
- **`MAINNET_GLM_CONTRACT_ADDRESS`** - GLM token contract on mainnet
  - Default: `0x7DD9c5Cba05E151C895FDe1CF355C9A1D5DA6429`

- **`HOLESKY_TGLM_CONTRACT_ADDRESS`** - tGLM token contract on Holesky
  - Default: `0xd94e3DC39d4Cad1DAd634e7eb585A57A19dC7EFE`

- **`*_TGLM_FAUCET_ADDRESS`** - Faucet contract addresses for testnets
  - Purpose: Test token distribution

### Transaction Configuration
- **`ERC20_HOLESKY_REQUIRED_CONFIRMATIONS`** - Block confirmations for Holesky
  - Default: `3`
  - Purpose: Transaction finality threshold

- **`ERC20_MAINNET_REQUIRED_CONFIRMATIONS`** - Block confirmations for mainnet
  - Default: `5`
  - Purpose: Transaction finality threshold

- **`POLYGON_MAX_GAS_PRICE_DYNAMIC`** - Dynamic gas price limit for Polygon
  - Purpose: Gas price control

- **`POLYGON_GAS_PRICE_METHOD`** - Gas price calculation method for Polygon
  - Purpose: Gas price strategy

- **`POLYGON_PRIORITY`** - Priority setting for Polygon transactions
  - Purpose: Transaction priority configuration

### Development/Testing
- **`YAGNA_DEV_SKIP_ALLOCATION_VALIDATION`** - Skip allocation validation
  - Values: `"1"` or `"true"`
  - Purpose: Development mode configuration

## Activity Service

- **`INACTIVITY_LIMIT_SECONDS`** - Activity inactivity threshold
  - Default: `10` seconds (minimum: `2s`)
  - Purpose: Timeout for inactive activities

- **`UNRESPONSIVE_LIMIT_SECONDS`** - Activity unresponsive threshold
  - Default: `5` seconds (minimum: `2s`)
  - Purpose: Mark activities as unresponsive

- **`PROCESS_KILL_TIMEOUT_SECONDS`** - Grace period for SIGTERM→SIGKILL
  - Default: `5` seconds (minimum: `1s`)
  - Purpose: Graceful process termination timeout

## Exe-Unit Configuration

- **`EXE_UNIT_PATH`** - Path to ExeUnits descriptor file
  - Default: `../exe-unit/resources/local-debug-exeunits-descriptor.json`
  - Purpose: Runtime configuration for execution units

- **`EXE_UNIT_SEC_KEY`** - Security key for exe-unit
  - Purpose: Secure communication with execution units

- **`EXE_UNIT_REQUESTOR_PUB_KEY`** - Requestor's public key
  - Purpose: Cryptographic verification

- **`IAS_SERVICE_ADDRESS`** - Intel Attestation Service address for SGX
  - Purpose: SGX attestation configuration

## Metrics Service

- **`YAGNA_METRICS_URL`** - Metrics push URL
  - Default: `https://metrics.golem.network:9092/`
  - Purpose: Telemetry data destination

- **`YAGNA_METRICS_JOB_NAME`** - Metrics job identifier
  - Default: `"community.1"`
  - Purpose: Metrics categorization

- **`YAGNA_METRICS_GROUP`** - Arbitrary node grouping label
  - Purpose: Node categorization for metrics

## Logging Configuration

- **`RUST_LOG`** - Rust logging configuration
  - Example: `debug,tokio_core=info,tokio_reactor=info,hyper=info,reqwest=info`
  - Purpose: Control log levels per module

- **`LOG_FILES_UNCOMPRESSED`** - Number of uncompressed log files to keep
  - Default: `1`
  - Purpose: Log file retention policy

- **`LOG_FILES_COMPRESSED`** - Number of compressed log files to keep
  - Default: `10`
  - Purpose: Compressed log retention policy

- **`LOG_ROTATE_AGE`** - Log rotation frequency
  - Values: `"DAY"` or `"HOUR"` (case insensitive)
  - Default: `"day"`
  - Purpose: Log rotation schedule

- **`LOG_ROTATE_SIZE`** - Log rotation size in bytes
  - Default: `1073741824` (1GiB)
  - Purpose: Size-based log rotation trigger

## Consent Management

- **`YA_CONSENT_PATH`** - Path to consent configuration file
  - Purpose: Privacy and consent settings

- **`YA_CONSENT_STATS`** - Stats consent setting
  - Values: `"allow"` or `"deny"`
  - Purpose: Statistics collection consent

## Provider Agent

- **`DISABLE_AUTO_CLEANUP`** - Disable automatic cleanup of provider logs
  - Values: `true` to disable
  - Purpose: Preserve logs for debugging

- **`NODE_NAME`** - Human readable node identity
  - Required: Yes (set during configuration)
  - Purpose: Network identification

## Development/Build Variables

- **`DATABASE_URL`** - Database connection URL
  - Purpose: Database configuration for development

- **`OPENSSL_STATIC`** - Enable static OpenSSL linking
  - Purpose: Build configuration

- **`NOTIFY_SOCKET`** - SystemD notification socket
  - Purpose: Unix system integration

## Configuration Files

The following files contain environment variable templates and examples:

- **`.env-template`** - Complete environment variable template with comments
- **`core/payment/.env`** - Payment service specific configuration
- **`core/persistence/.env`** - Database configuration

## Usage Notes

1. **Defaults**: Most variables have sensible defaults and are optional
2. **Validation**: Numeric values often have minimum/maximum constraints
3. **Network-specific**: Many variables are prefixed by network name (MAINNET_, HOLESKY_, etc.)
4. **Service isolation**: Each service has its own variable namespace
5. **Type conversion**: Automatic parsing for numeric values, durations, and URLs
6. **Fallback behavior**: Missing variables typically fall back to computed defaults

## Security Considerations

- Keep `YAGNA_APPKEY` secure - it provides full API access
- Be cautious with blockchain node URLs - they affect payment processing
- Contract addresses should only be overridden for testing purposes
- Log levels should be set appropriately for production vs development