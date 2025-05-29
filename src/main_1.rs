// src/main.rs
use solana_sdk::signature::{Keypair, Signer};
use rayon::prelude::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use serde::Serialize;

#[derive(Serialize)]
struct VanityKeypair {
    publicKey: String,
    secretKey: Vec<u8>,
}

fn main() {
    let target_suffix = "send";
    let target_count = 5000;
    let counter = Arc::new(AtomicUsize::new(0));
    let found = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    (0..1_000_000_000usize).into_par_iter().for_each(|_| {
        if found.load(Ordering::Relaxed) >= target_count {
            return;
        }

        let keypair = Keypair::new();
        let pubkey = keypair.pubkey().to_string();

        if pubkey.ends_with(target_suffix) {
            let count = found.fetch_add(1, Ordering::SeqCst);
            println!("🎯 {} => {}", count + 1, pubkey);

            let filename = format!("{}.json", pubkey);
            let secret_bytes = keypair.to_bytes().to_vec();

            let vanity = VanityKeypair {
                publicKey: pubkey,
                secretKey: secret_bytes,
            };

            let json = serde_json::to_string_pretty(&vanity).expect("Failed to serialize");

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(filename)
                .expect("Failed to create file");

            writeln!(file, "{}", json).expect("Failed to write to file");
        }

        let c = counter.fetch_add(1, Ordering::Relaxed);
        if c % 100_000 == 0 {
            println!("Checked {} keypairs...", c);
        }
    });

    println!(
        "Finished in {:.2?} | Total attempts: {}",
        start.elapsed(),
        counter.load(Ordering::Relaxed)
    );
}