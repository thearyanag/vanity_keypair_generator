# Solana Vanity Address Generator

This is a high-performance Solana vanity address generator written in Rust. It leverages parallel processing with Rayon and asynchronous database operations with Tokio and Deadpool to efficiently find and store keypairs matching a desired suffix.

## Features

- **Parallel Keypair Generation**: Uses Rayon to generate and check keypairs in parallel across all available CPU cores.
- **Asynchronous Database Flushing**: Uses Tokio and Deadpool for non-blocking database writes to a PostgreSQL database.
- **Configurable Suffix and Target Count**: Easily set the desired public key suffix and the number of addresses to find.
- **Secure Connection**: Uses native-tls for secure TLS connections to the database, requiring a CA certificate.
- **Environment Variable Configuration**: Database URL is configured via a `.env` file.

## Prerequisites

- Rust toolchain
- PostgreSQL database
- A CA certificate file for TLS connection to the database (e.g., `do-certificate.crt`)

## Setup

1.  **Clone the repository:**
    ```bash
    git clone <repository_url>
    cd solana_vanity_generator
    ```
2.  **Create a `.env` file** in the root directory with your database connection string:
    ```env
    DATABASE_URL="postgresql://user:password@host:port/database?sslmode=require"
    ```
3.  **Place your CA certificate** (e.g., `do-certificate.crt`) in the root directory of the project. This file is specified in `src/main.rs`.
4.  **Database Schema**: Ensure you have a table named `users` in your PostgreSQL database with the following (or similar) schema:
    ```sql
    CREATE TABLE users (
        "publicKey" VARCHAR(255) PRIMARY KEY,
        "privateKey" VARCHAR(255) NOT NULL
    );
    ```

## Configuration

Inside `src/main.rs`, you can configure the following parameters within the `main` function:

-   `target_suffix`: The desired suffix for the Solana public keys (e.g., "send").
-   `target_count`: The number of vanity addresses to generate before stopping (e.g., 10000).
-   `Certificate File`: The path to the CA certificate is hardcoded as `"do-certificate.crt"`.
-   `Database Pool Size`: The `max_size` for the Deadpool PostgreSQL connection pool can be adjusted (e.g., `max_size(16)`).
-   `Flush Buffer Size`: The number of keypairs to buffer before flushing to the database can be changed (e.g., `buffer.len() >= 100`).
-   `Flush Interval`: The time interval for flushing the buffer to the database can be changed (e.g., `Duration::from_secs(60)`).


## Usage

1.  **Build the project:**
    ```bash
    cargo build --release
    ```
2.  **Run the generator:**
    ```bash
    ./target/release/solana_vanity_generator
    ```

The generator will start searching for keypairs. Found keypairs will be printed to the console and saved to the configured PostgreSQL database. Statistics on the number of checked keypairs will also be periodically printed.

## How It Works

1.  **Initialization**:
    *   Loads the CA certificate for TLS.
    *   Reads the `DATABASE_URL` from the `.env` file.
    *   Sets up a `deadpool-postgres` connection pool.
    *   Creates a Tokio MPSC channel for sending found `Vanity` keypairs from generator threads to the database flushing task.

2.  **Asynchronous Database Flushing Task**:
    *   A separate Tokio task runs an asynchronous loop.
    *   It receives `Vanity` structs from the MPSC channel and buffers them.
    *   The buffer is flushed to the database (inserting multiple rows in a single `INSERT` statement) when:
        *   The buffer reaches a certain size (e.g., 100 entries).
        *   A time interval elapses (e.g., every 60 seconds).
    *   Uses `ON CONFLICT DO NOTHING` to handle potential duplicate public keys.

3.  **Parallel Keypair Generation (Rayon)**:
    *   A large range of numbers (effectively infinite for practical purposes) is iterated over in parallel using Rayon's `into_par_iter()`.
    *   In each parallel task:
        *   A new Solana `Keypair` is generated.
        *   The public key is checked if it ends with the `target_suffix`.
        *   If a match is found:
            *   The `found` counter is atomically incremented.
            *   The public key and its base58 encoded private key are packaged into a `Vanity` struct.
            *   The `Vanity` struct is sent to the database flushing task via the MPSC channel using `blocking_send` (as Rayon threads are not async-aware by default, but the channel send itself is quick).
        *   A counter for checked keypairs is atomically incremented and progress is printed periodically.
    *   Generation stops once `target_count` keypairs are found.

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open an issue.
