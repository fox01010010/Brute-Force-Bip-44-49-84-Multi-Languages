use anyhow::{Context, Result};
use bip39::{Language, Mnemonic};
use bitcoin::address::{Address, NetworkChecked, NetworkUnchecked};
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::hashes::{sha256, Hash};
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

    /// Maximum number of permutations to test (default: all)
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

// ── Fatorial (u128 para suportar 24! sem overflow) ───────────────────────────
fn factorial(n: u128) -> u128 {
    (1..=n).product::<u128>().max(1)
}

// ── Permutação por índice com bitmask (sem Vec::remove) ──────────────────────
//
// A abordagem anterior usava Vec::remove(pos) — O(n) por passo porque desloca
// todos os elementos à direita. Com um bitmask u32 (suporta n<=32),
// encontramos o k-ésimo elemento livre com um loop simples sem alocar memória.
//
// O parâmetro index chega como u64 (seguro para o Rayon) e é convertido para
// u128 aqui onde o fatorádico precisa de precisão total.
fn permutation_at_index(items: &[u16], mut index: u128) -> Vec<u16> {
    let n = items.len();
    debug_assert!(n <= 32, "bitmask suporta no maximo 32 itens");

    let mut result = Vec::with_capacity(n);
    let mut used: u32 = 0; // bit i = 1 → items[i] ja foi usado

    for i in (1..=n as u128).rev() {
        let f  = factorial(i - 1);
        let k  = (index / f) as usize; // queremos o k-esimo item ainda livre
        index %= f;

        // Encontra a posicao do k-esimo bit NAO-setado em `used`
        let mut count = 0usize;
        let mut pos   = 0usize;
        loop {
            if used & (1 << pos) == 0 {
                if count == k { break; }
                count += 1;
            }
            pos += 1;
        }

        used |= 1 << pos;
        result.push(items[pos]);
    }
    result
}

// ── Validacao rapida de checksum BIP-39 sem string nem parse ─────────────────
//
// Pipeline ANTIGO por permutacao:
//   join(" ") → Mnemonic::parse (busca na wordlist + SHA256) → to_seed (PBKDF2) → BIP32
//
// Com esta funcao, PBKDF2 + BIP32 so executam quando o checksum passa —
// o que ocorre em apenas 1/16 das permutacoes de 12 palavras
// e 1/256 das de 24 palavras.
//
// Matematica BIP-39: cada palavra = 11 bits de indice na wordlist
//   12 palavras → 132 bits = 128 bits entropia + 4 bits checksum
//   24 palavras → 264 bits = 256 bits entropia + 8 bits checksum
//   Checksum = primeiros N bits de SHA256(entropy)
fn checksum_valid(indices: &[u16]) -> bool {
    let n = indices.len();
    let entropy_bytes = n * 4 / 3; // 12→16, 24→32
    let checksum_bits = n / 3;     // 12→4,  24→8

    // Empacota os indices de 11 bits em bytes
    let mut buf = [0u8; 33]; // 32 bytes de entropia + 1 byte extra para os bits de checksum
    for (i, &idx) in indices.iter().enumerate() {
        let bit_start = i * 11;
        for b in 0..11usize {
            if (idx >> (10 - b)) & 1 == 1 {
                let pos = bit_start + b;
                buf[pos >> 3] |= 1u8 << (7 - (pos & 7));
            }
        }
    }

    let hash = sha256::Hash::hash(&buf[..entropy_bytes]);
    let mask  = 0xFFu8 << (8 - checksum_bits);
    (buf[entropy_bytes] & mask) == (hash[0] & mask)
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

    // ── Converte palavras para indices BIP-39 (feito uma vez, antes do loop) ─
    // O loop paralelo opera sobre [u16], sem nunca alocar Strings por permutacao.
    let wordlist = language.word_list(); // &'static [&'static str] — Send + Sync
    let word_indices: Vec<u16> = args.words.iter()
        .map(|w| {
            wordlist.iter()
                .position(|&wl| wl == w.as_str())
                .map(|i| i as u16)
                .with_context(|| format!(
                    "'{}' nao esta na wordlist BIP-39 ({})", w, language_name(language)))
        })
        .collect::<Result<Vec<_>>>()?;

    // Aviso sobre palavras duplicadas (geram permutacoes identicas = trabalho dobrado)
    {
        let mut sorted = word_indices.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() < n {
            println!("⚠ Atencao: {} palavra(s) duplicada(s) — algumas permutacoes serao identicas",
                n - sorted.len());
        }
    }

    // ── Limites de permutacao ────────────────────────────────────────────────
    // Rayon usa usize internamente; em 64-bit usize::MAX = u64::MAX ≈ 1.8e19.
    // 24! ≈ 6.2e23 transbordaria usize. Usamos u64 no iterador do Rayon
    // e convertemos para u128 somente dentro de permutation_at_index.
    let total_u128: u128 = factorial(n as u128);
    let total_u64:  u64  = total_u128.min(u64::MAX as u128) as u64;
    let max_perm:   u64  = args.max_permutations
        .unwrap_or(total_u64)
        .min(total_u64);

    let derivation_path_str = address_type.derivation_path(args.derivation);
    let derivation_path = DerivationPath::from_str(&derivation_path_str)
        .context("Failed to parse derivation path")?;

    let reject_rate = if n == 12 { "15/16 (~94%)" } else { "255/256 (~99.6%)" };
    let pass_rate   = if n == 12 { "1/16"          } else { "1/256" };

    println!("Address type      : {}", address_type.name());
    println!("Derivation path   : {}", derivation_path_str);
    println!("Total permutations: {}", format_number(total_u64));
    println!("Will test         : {}", format_number(max_perm));
    println!("CPU threads       : {}", rayon::current_num_threads());
    println!();
    println!("Etapas: [1] perm(u16+bitmask) → [2] checksum(SHA256) → [3] PBKDF2 → [4] BIP32+addr");
    println!("  [2] descarta {} sem string/PBKDF2 — so {} chegam em [3]", reject_rate, pass_rate);
    println!();

    // ── Estado compartilhado ─────────────────────────────────────────────────
    let found   = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicU64::new(0));
    let start   = Instant::now();

    // ── Thread de progresso ──────────────────────────────────────────────────
    let c2    = Arc::clone(&counter);
    let f2    = Arc::clone(&found);
    let done  = Arc::new(AtomicBool::new(false));
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

    // ── Busca paralela ───────────────────────────────────────────────────────
    //
    // Por permutacao:
    //   [1] permutation_at_index — u16 + bitmask, sem String, sem Vec::remove
    //   [2] checksum_valid       — empacota bits + SHA256 (~1µs) — rejeita a maioria
    //   [3] reconstroi phrase    — so para os ~1/16 ou ~1/256 que passaram
    //   [4] PBKDF2 + BIP32       — gargalo real (~1ms), mas chamado raramente
    let found_ref = Arc::clone(&found);
    let ctr_ref   = Arc::clone(&counter);

    let result = (0u64..max_perm).into_par_iter().find_map_any(|idx| {
        if found_ref.load(Ordering::Relaxed) { return None; }

        ctr_ref.fetch_add(1, Ordering::Relaxed);

        // [1] Permutacao como [u16] — sem alocacao de String
        let perm = permutation_at_index(&word_indices, idx as u128);

        // [2] Checksum rapido — sem join/parse de string
        //     Rejeita ~93.75% (12 words) ou ~99.6% (24 words)
        if !checksum_valid(&perm) { return None; }

        // [3] Reconstroi a frase somente para os raros checksum-validos
        let phrase: String = perm.iter()
            .map(|&i| wordlist[i as usize])
            .collect::<Vec<_>>()
            .join(" ");

        // [4] PBKDF2 (2048x HMAC-SHA512) — gargalo, mas chamado ~1/16 ou ~1/256 vezes
        let mnemonic = Mnemonic::parse_in_normalized(language, &phrase).ok()?;
        let seed      = mnemonic.to_seed("");

        // [5] Derivacao BIP32 + geracao de endereco
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
