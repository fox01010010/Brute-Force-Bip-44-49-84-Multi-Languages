use anyhow::{Context, Result};
use bip39::{Language, Mnemonic};
use bitcoin::address::{Address, NetworkChecked, NetworkUnchecked};
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::{Network, PublicKey};
use clap::Parser;
use itertools::Itertools;
use std::str::FromStr;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(about = "Try permutations of 12 or 24 BIP-39 words to match a BTC address", version)]
struct Args {
    /// Target Bitcoin address (Base58 for legacy, Bech32 for SegWit)
    target_address: String,

    /// Exactly 12 or 24 words (unordered or partially ordered)
    words: Vec<String>,

    /// Maximum number of permutations to test
    #[arg(long, default_value_t = 1_000_000)]
    max_permutations: usize,

    /// BIP-39 wordlist language (english, portuguese, spanish, french, italian, czech, korean, japanese, chinese-simplified, chinese-traditional)
    #[arg(long, short, default_value = "english")]
    language: String,

    /// Derivation index (e.g., 0, 1, 2, 3, etc.)
    #[arg(long, default_value_t = 0)]
    derivation: u32,

    /// Use BIP44 (legacy addresses - starts with 1)
    #[arg(long, conflicts_with_all = ["bip49", "bip84"])]
    bip44: bool,

    /// Use BIP49 (P2SH-wrapped SegWit addresses - starts with 3)
    #[arg(long, conflicts_with_all = ["bip44", "bip84"])]
    bip49: bool,

    /// Use BIP84 (native SegWit addresses - starts with bc1)
    #[arg(long, conflicts_with_all = ["bip44", "bip49"])]
    bip84: bool,
}

#[derive(Debug, Clone, Copy)]
enum AddressType {
    Bip44, // Legacy P2PKH (1...)
    Bip49, // P2SH-wrapped SegWit (3...)
    Bip84, // Native SegWit P2WPKH (bc1...)
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

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate word count
    if args.words.len() != 12 && args.words.len() != 24 {
        anyhow::bail!(
            "Expected exactly 12 or 24 words, got {}",
            args.words.len()
        );
    }

    // Parse target address
    let target_address_unchecked = args
        .target_address
        .parse::<Address<NetworkUnchecked>>()
        .context("Invalid target Bitcoin address")?;

    let target_address: Address<NetworkChecked> = target_address_unchecked
        .require_network(Network::Bitcoin.into())
        .context("This tool currently only supports mainnet addresses")?;

    // Determine address type
    let address_type = if args.bip84 {
        AddressType::Bip84
    } else if args.bip49 {
        AddressType::Bip49
    } else if args.bip44 {
        AddressType::Bip44
    } else {
        // Auto-detect based on address format
        if args.target_address.starts_with("bc1") {
            println!("Auto-detected BIP84 (Native SegWit) address");
            AddressType::Bip84
        } else if args.target_address.starts_with('3') {
            println!("Auto-detected BIP49 (P2SH-wrapped SegWit) address");
            AddressType::Bip49
        } else if args.target_address.starts_with('1') {
            println!("Auto-detected BIP44 (Legacy) address");
            AddressType::Bip44
        } else {
            anyhow::bail!(
                "Cannot auto-detect address type. Please specify --bip44, --bip49, or --bip84"
            );
        }
    };

    println!("Configuration:");
    println!("  Address type: {}", address_type.name());
    println!("  Derivation index: {}", args.derivation);
    println!("  Word count: {}", args.words.len());
    
    // Auto-detect language or use specified one
    let language = if args.language == "english" {
        // Try to auto-detect language
        match detect_language(&args.words) {
            Some(detected_lang) => {
                println!("  Language: {} (auto-detected)", language_name(detected_lang));
                detected_lang
            }
            None => {
                println!("  Language: english (default)");
                Language::English
            }
        }
    } else {
        let lang = parse_language(&args.language)?;
        println!("  Language: {}", args.language);
        lang
    };
    
    println!("  Max permutations: {}", format_number(args.max_permutations));
    println!();

    let start = Instant::now();
    let found = search_permutations(
        &args.words,
        &target_address,
        args.max_permutations,
        language,
        address_type,
        args.derivation,
    )?;
    let elapsed = start.elapsed();

    if !found {
        println!(
            "No matching mnemonic found within the first {} permutations (elapsed: {:?})",
            format_number(args.max_permutations),
            elapsed
        );
    }

    Ok(())
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}G", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn parse_language(lang: &str) -> Result<Language> {
    match lang.to_lowercase().as_str() {
        "english" => Ok(Language::English),
        "portuguese" => Ok(Language::Portuguese),
        "spanish" => Ok(Language::Spanish),
        "french" => Ok(Language::French),
        "italian" => Ok(Language::Italian),
        "czech" => Ok(Language::Czech),
        "korean" => Ok(Language::Korean),
        "japanese" => Ok(Language::Japanese),
        "chinese-simplified" => Ok(Language::SimplifiedChinese),
        "chinese-traditional" => Ok(Language::TraditionalChinese),
        _ => anyhow::bail!(
            "Unknown language: {}. Supported: english, portuguese, spanish, french, italian, czech, korean, japanese, chinese-simplified, chinese-traditional",
            lang
        ),
    }
}

fn language_name(lang: Language) -> &'static str {
    match lang {
        Language::English => "english",
        Language::Portuguese => "portuguese",
        Language::Spanish => "spanish",
        Language::French => "french",
        Language::Italian => "italian",
        Language::Czech => "czech",
        Language::Korean => "korean",
        Language::Japanese => "japanese",
        Language::SimplifiedChinese => "chinese-simplified",
        Language::TraditionalChinese => "chinese-traditional",
    }
}

fn detect_language(words: &[String]) -> Option<Language> {
    // List of languages to try
    let languages = [
        Language::English,
        Language::Portuguese,
        Language::Spanish,
        Language::French,
        Language::Italian,
        Language::Czech,
        Language::Korean,
        Language::Japanese,
        Language::SimplifiedChinese,
        Language::TraditionalChinese,
    ];

    // Count how many words are valid in each language
    let mut best_language = None;
    let mut best_count = 0;

    for &lang in &languages {
        let mut valid_count = 0;
        
        for word in words {
            // Get the wordlist for this language
            let wordlist = lang.word_list();
            
            // Check if the word exists in this wordlist
            if wordlist.iter().any(|w| w.eq_ignore_ascii_case(word)) {
                valid_count += 1;
            }
        }

        // If we found more valid words in this language, update best
        if valid_count > best_count {
            best_count = valid_count;
            best_language = Some(lang);
        }

        // If all words are valid, we found the language
        if valid_count == words.len() {
            return Some(lang);
        }
    }

    // Only return a language if we found at least half the words
    if best_count >= words.len() / 2 {
        best_language
    } else {
        None
    }
}

fn search_permutations(
    words: &[String],
    target: &Address<NetworkChecked>,
    max_permutations: usize,
    language: Language,
    address_type: AddressType,
    derivation_index: u32,
) -> Result<bool> {
    let derivation_path_str = address_type.derivation_path(derivation_index);
    let derivation_path: DerivationPath = DerivationPath::from_str(&derivation_path_str)
        .context("Failed to parse derivation path")?;

    println!("Using derivation path: {}\n", derivation_path_str);

    let secp = bitcoin::secp256k1::Secp256k1::new();

    for (i, perm) in words
        .iter()
        .cloned()
        .permutations(words.len())
        .take(max_permutations)
        .enumerate()
    {
        if i % 100000 == 0 && i > 0 {
            println!("Checked {} permutations...", format_number(i));
        }

        let phrase = perm.join(" ");

        let mnemonic = match Mnemonic::parse_in_normalized(language, &phrase) {
            Ok(m) => m,
            Err(_) => continue, // skip invalid mnemonics
        };

        let seed = mnemonic.to_seed("");

        let master_xprv = Xpriv::new_master(Network::Bitcoin, &seed)
            .context("Failed to create master xprv")?;

        let child_xprv = master_xprv.derive_priv(&secp, &derivation_path)?;

        let child_priv = child_xprv.private_key;
        let child_pub_key = child_priv.public_key(&secp);

        // Generate address based on type
        let addr: Address<NetworkChecked> = match address_type {
            AddressType::Bip44 => {
                let child_pub = PublicKey::new(child_pub_key);
                Address::p2pkh(&child_pub, Network::Bitcoin)
            }
            AddressType::Bip49 => {
                // BIP49: P2WPKH-nested-in-P2SH
                let compressed = bitcoin::CompressedPublicKey::from_slice(&child_pub_key.serialize())
                    .context("Failed to create compressed public key")?;
                Address::p2shwpkh(&compressed, Network::Bitcoin)
            }
            AddressType::Bip84 => {
                // BIP84: Native SegWit (P2WPKH)
                let compressed = bitcoin::CompressedPublicKey::from_slice(&child_pub_key.serialize())
                    .context("Failed to create compressed public key")?;
                Address::p2wpkh(&compressed, Network::Bitcoin)
            }
        };

        if &addr == target {
            println!("✓ FOUND MATCHING MNEMONIC!");
            println!();
            println!("Mnemonic phrase:");
            println!("  {}", phrase);
            println!();
            println!("Details:");
            println!("  Permutation index: {}", i);
            println!("  Address type: {}", address_type.name());
            println!("  Derivation path: {}", derivation_path_str);
            println!("  Derived address: {}", addr);
            println!();
            return Ok(true);
        }
    }

    Ok(false)
}