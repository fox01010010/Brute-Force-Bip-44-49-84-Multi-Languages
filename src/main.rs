use anyhow::{Context, Result};
use bip39::{Language, Mnemonic};
use bitcoin::address::{Address, NetworkChecked, NetworkUnchecked};
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::{Network, PublicKey};
use clap::Parser;
use rayon::prelude::*;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::thread;

// ── Thread-local Secp256k1 context ──────────────────────────────────────────
thread_local! {
    static SECP: bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All> =
        bitcoin::secp256k1::Secp256k1::new();
}

#[derive(Parser, Debug)]
#[command(about = "Try permutations of 12 or 24 BIP-39 words to match a BTC address", version)]
struct Args {
    /// Target Bitcoin address
    target_address: String,

    /// Exactly 12 or 24 words (unordered)
    words: Vec<String>,

    /// Maximum number of permutations to test
    #[arg(long)]
    max_permutations: Option<u64>,

    /// BIP-39 language
    #[arg(long, short, default_value = "english")]
    language: String,

    /// Derivation index
    #[arg(long, default_value_t = 0)]
    derivation: u32,

    #[arg(long, conflicts_with_all = ["bip49", "bip84"])]
    bip44: bool,

    #[arg(long, conflicts_with_all = ["bip44", "bip84"])]
    bip49: bool,

    #[arg(long, conflicts_with_all = ["bip44", "bip49"])]
    bip84: bool,
}

#[derive(Debug, Clone, Copy)]
enum AddressType {
    Bip44,
    Bip49,
    Bip84,
}

impl AddressType {
    fn derivation_path(&self, index: u32) -> String {
        match self {
            AddressType::Bip44 => format!("m/44'/0'/0'/0/{}", index),
            AddressType::Bip49 => format!("m/49'/0'/0'/0/{}", index),
            AddressType::Bip84 => format!("m/84'/0'/0'/0/{}", index),
        }
    }
    fn name(&self) -> &str {
        match self {
            AddressType::Bip44 => "BIP44 (Legacy P2PKH)",
            AddressType::Bip49 => "BIP49 (P2SH-wrapped SegWit)",
            AddressType::Bip84 => "BIP84 (Native SegWit)",
        }
    }
}

// ── Permutation by index (Lehmer code / factoradic) ─────────────────────────
// Given an index in [0, n!), returns the corresponding permutation of `items`.
// Each thread calls this directly — zero coordination needed.
fn permutation_at_index(items: &[String], mut index: u64) -> Vec<String> {
    let n = items.len();
    let mut available: Vec<&String> = items.iter().collect();
    let mut result = Vec::with_capacity(n);

    for i in (1..=n as u64).rev() {
        let f = factorial(i - 1);
        let pos = (index / f) as usize;
        result.push(available[pos].clone());
        available.remove(pos);
        index %= f;
    }
    result
}

fn factorial(n: u64) -> u64 {
    (1..=n).product::<u64>().max(1)
}

fn format_number(n: u64) -> String {
    if n == u64::MAX { return "ALL".to_string(); }
    if n >= 1_000_000_000 { format!("{:.1}G", n as f64 / 1e9) }
    else if n >= 1_000_000  { format!("{:.1}M", n as f64 / 1e6) }
    else if n >= 1_000      { format!("{:.1}K", n as f64 / 1e3) }
    else                     { n.to_string() }
}

fn parse_language(lang: &str) -> Result<Language> {
    match lang.to_lowercase().as_str() {
        "english"              => Ok(Language::English),
        "portuguese"           => Ok(Language::Portuguese),
        "spanish"              => Ok(Language::Spanish),
        "french"               => Ok(Language::French),
        "italian"              => Ok(Language::Italian),
        "czech"                => Ok(Language::Czech),
        "korean"               => Ok(Language::Korean),
        "japanese"             => Ok(Language::Japanese),
        "chinese-simplified"   => Ok(Language::SimplifiedChinese),
        "chinese-traditional"  => Ok(Language::TraditionalChinese),
        _ => anyhow::bail!("Unknown language: {}", lang),
    }
}

fn language_name(lang: Language) -> &'static str {
    match lang {
        Language::English            => "english",
        Language::Portuguese         => "portuguese",
        Language::Spanish            => "spanish",
        Language::French             => "french",
        Language::Italian            => "italian",
        Language::Czech              => "czech",
        Language::Korean             => "korean",
        Language::Japanese           => "japanese",
        Language::SimplifiedChinese  => "chinese-simplified",
        Language::TraditionalChinese => "chinese-traditional",
    }
}

fn detect_language(words: &[String]) -> Option<Language> {
    let languages = [
        Language::English, Language::Portuguese, Language::Spanish,
        Language::French,  Language::Italian,    Language::Czech,
        Language::Korean,  Language::Japanese,   Language::SimplifiedChinese,
        Language::TraditionalChinese,
    ];
    languages.iter().copied().max_by_key(|&lang| {
        let wl = lang.word_list();
        words.iter().filter(|w| wl.contains(&w.as_str())).count()
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.words.len() != 12 && args.words.len() != 24 {
        anyhow::bail!("Expected exactly 12 or 24 words, got {}", args.words.len());
    }

    let n = args.words.len();
    let total: u64 = factorial(n as u64);
    let max_permutations = args.max_permutations.unwrap_or(total).min(total);

    let target_address: Address<NetworkChecked> = args
        .target_address
        .parse::<Address<NetworkUnchecked>>()
        .context("Invalid target Bitcoin address")?
        .require_network(Network::Bitcoin.into())
        .context("Only mainnet addresses supported")?;

    let address_type = if args.bip84 {
        AddressType::Bip84
    } else if args.bip49 {
        AddressType::Bip49
    } else if args.bip44 {
        AddressType::Bip44
    } else if args.target_address.starts_with("bc1") {
        println!("Auto-detected BIP84 (Native SegWit)");
        AddressType::Bip84
    } else if args.target_address.starts_with('3') {
        println!("Auto-detected BIP49 (P2SH-wrapped SegWit)");
        AddressType::Bip49
    } else if args.target_address.starts_with('1') {
        println!("Auto-detected BIP44 (Legacy)");
        AddressType::Bip44
    } else {
        anyhow::bail!("Cannot auto-detect address type. Use --bip44, --bip49, or --bip84");
    };

    let language = if args.language == "english" {
        match detect_language(&args.words) {
            Some(l) => { println!("Language: {} (auto-detected)", language_name(l)); l }
            None    => { println!("Language: english (default)"); Language::English }
        }
    } else {
        let l = parse_language(&args.language)?;
        println!("Language: {}", args.language);
        l
    };

    let derivation_path_str = address_type.derivation_path(args.derivation);
    let derivation_path = DerivationPath::from_str(&derivation_path_str)
        .context("Failed to parse derivation path")?;

    println!("Address type      : {}", address_type.name());
    println!("Derivation path   : {}", derivation_path_str);
    println!("Total permutations: {}", format_number(total));
    println!("Will test         : {}", format_number(max_permutations));
    println!("CPU threads       : {}", rayon::current_num_threads());
    println!();

    // ── Shared state ────────────────────────────────────────────────────────
    let found   = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicU64::new(0));
    let start   = Instant::now();

    // ── Progress thread ─────────────────────────────────────────────────────
    let c2   = Arc::clone(&counter);
    let f2   = Arc::clone(&found);
    let done = Arc::new(AtomicBool::new(false));
    let done2 = Arc::clone(&done);

    let progress_handle = thread::spawn(move || {
        let mut last = 0u64;
        let mut t    = Instant::now();
        loop {
            thread::sleep(std::time::Duration::from_secs(5));
            if f2.load(Ordering::Relaxed) || done2.load(Ordering::Relaxed) { break; }
            let cur = c2.load(Ordering::Relaxed);
            let dt  = t.elapsed().as_secs_f64();
            if dt > 0.0 {
                println!("Progress: {} | Speed: {:.1}K/s",
                    format_number(cur),
                    (cur - last) as f64 / dt / 1000.0);
            }
            last = cur;
            t    = Instant::now();
        }
    });

    // ── Main parallel search ─────────────────────────────────────────────────
    // (0..max_permutations) is a plain integer range — Rayon splits it evenly
    // across all worker threads with no producer bottleneck.
    // Each thread calls permutation_at_index(idx) to get its permutation
    // directly via the Lehmer/factoradic algorithm.
    let words     = args.words.clone();
    let found_ref = Arc::clone(&found);
    let ctr_ref   = Arc::clone(&counter);

    let result = (0u64..max_permutations).into_par_iter().find_map_any(|idx| {
        if found_ref.load(Ordering::Relaxed) {
            return None;
        }

        ctr_ref.fetch_add(1, Ordering::Relaxed);

        let perm   = permutation_at_index(&words, idx);
        let phrase = perm.join(" ");

        let mnemonic = Mnemonic::parse_in_normalized(language, &phrase).ok()?;
        let seed     = mnemonic.to_seed("");

        let addr: Address<NetworkChecked> = SECP.with(|secp| {
            let master = Xpriv::new_master(Network::Bitcoin, &seed).ok()?;
            let child  = master.derive_priv(secp, &derivation_path).ok()?;
            let pubkey = child.private_key.public_key(secp);

            match address_type {
                AddressType::Bip44 => {
                    Some(Address::p2pkh(&PublicKey::new(pubkey), Network::Bitcoin))
                }
                AddressType::Bip49 => {
                    let c = bitcoin::CompressedPublicKey::from_slice(&pubkey.serialize()).ok()?;
                    Some(Address::p2shwpkh(&c, Network::Bitcoin))
                }
                AddressType::Bip84 => {
                    let c = bitcoin::CompressedPublicKey::from_slice(&pubkey.serialize()).ok()?;
                    Some(Address::p2wpkh(&c, Network::Bitcoin))
                }
            }
        })?;

        if &addr == &target_address {
            Some((phrase, idx))
        } else {
            None
        }
    });

    done.store(true, Ordering::Relaxed);
    let _ = progress_handle.join();

    let elapsed       = start.elapsed();
    let total_checked = counter.load(Ordering::Relaxed);
    let rate          = total_checked as f64 / elapsed.as_secs_f64() / 1000.0;

    if let Some((phrase, idx)) = result {
        found.store(true, Ordering::Relaxed);
        println!("\n✓ FOUND MATCHING MNEMONIC!");
        println!("  Mnemonic : {}", phrase);
        println!("  Index    : {}", idx);
        println!("  Address  : {}", target_address);
        println!("  Path     : {}", derivation_path_str);
        println!("  Elapsed  : {:?} ({:.1}K/s avg)", elapsed, rate);
    } else {
        println!("\n✗ No matching mnemonic found.");
        println!("  Checked  : {}", format_number(total_checked));
        println!("  Elapsed  : {:?}", elapsed);
        println!("  Avg speed: {:.1}K/s", rate);
    }

    Ok(())
}
