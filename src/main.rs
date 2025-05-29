use bs58;
use deadpool_postgres::{Manager, Pool};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rayon::prelude::*;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time;

#[derive(Clone)]
struct Vanity {
    pubkey: String,
    privkey_b58: String,
}

// Custom iterator to control the generation loop
struct ConditionalIter {
    found_atomic: Arc<AtomicUsize>,
    target_count_val: usize,
}

impl Iterator for ConditionalIter {
    type Item = (); // The item type doesn't matter, it's a signal for work

    fn next(&mut self) -> Option<Self::Item> {
        if self.found_atomic.load(Ordering::Relaxed) >= self.target_count_val {
            None // Stop iteration
        } else {
            Some(()) // More work potentially available
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("Failed to load .env");
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");

    let target_suffix = "send";
    let target_count = 10000;

    let tls = MakeTlsConnector::new(
        TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("Failed to build TLS connector")
    );

    let mgr = Manager::new(db_url.parse().unwrap(), tls);
    let pool = Pool::builder(mgr).max_size(16).build().unwrap();
    let pool = Arc::new(pool);

    let (tx, mut rx) = mpsc::channel::<Vanity>(500);

    // 🔄 async flush loop
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut buffer = Vec::new();
        let mut interval = time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                Some(item) = rx.recv() => {
                    buffer.push(item);
                    if buffer.len() >= 100 {
                        flush(&pool_clone, &mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        flush(&pool_clone, &mut buffer).await;
                    }
                }
            }
        }
    });

    // 🧠 start parallel keypair gen using Rayon
    let counter = Arc::new(AtomicUsize::new(0));
    let found = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    // Create the conditional iterator
    let conditional_iter = ConditionalIter {
        found_atomic: found.clone(),
        target_count_val: target_count,
    };

    conditional_iter
        .par_bridge()
        .for_each_with(tx.clone(), |tx, _| { // The `_` here will be of type `()`
            // This check ensures tasks stop if target is met while they are queued/starting
            if found.load(Ordering::Relaxed) >= target_count {
                return;
            }

            let kp = Keypair::new();
            let pubkey = kp.pubkey().to_string();

            if pubkey.ends_with(target_suffix) {
                let count = found.fetch_add(1, Ordering::SeqCst);
                println!("🎯 {} => {}", count + 1, pubkey);

                let privkey_b58 = bs58::encode(kp.to_bytes()).into_string();
                let vanity = Vanity {
                    pubkey,
                    privkey_b58,
                };

                // send to async flusher (non-blocking)
                let _ = tx.blocking_send(vanity);
            }

            let c = counter.fetch_add(1, Ordering::Relaxed);
            if c % 100_000 == 0 {
                println!("Checked {} keypairs...", c);
            }
        });

    println!("✨ Done generating in {:.2?}", start.elapsed());
}

async fn flush(pool: &Pool, buffer: &mut Vec<Vanity>) {
    if buffer.is_empty() {
        return;
    }

    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("DB error: {}", e);
            return;
        }
    };

    let mut query = String::from("INSERT INTO users (\"publicKey\", \"privateKey\") VALUES ");
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![];

    for (i, entry) in buffer.iter().enumerate() {
        let idx = i * 2;
        query += &format!("(${}, ${}),", idx + 1, idx + 2);
        params.push(&entry.pubkey);
        params.push(&entry.privkey_b58);
    }

    query.pop(); // remove trailing comma
    query += " ON CONFLICT DO NOTHING;";

    if let Err(e) = client.execute(query.as_str(), &params).await {
        eprintln!("❌ Insert failed: {}", e);
    } else {
        println!("💾 Flushed {} entries to DB", buffer.len());
        buffer.clear();
    }
}
