# 🧠 Brute-Force-Rust Bip-44-49-84-Multi-Languages


## Use Case

If you have 12/24 BIP-39 mnemonic words but don't remember the correct order, this tool will brute-force permutations to find the combination that derives to your known Bitcoin address-Bip-44-49-84.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (rustc 1.95.0 or later recommended)

## 🐧 Linux / macOS (recomendado).

Execute no terminal:
- curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
- source $HOME/.cargo/env

## 🪟 Windows

- Baixe o instalador em:
- 👉 https://rustup.rs
- Execute o .exe e siga o padrão (Enter → Enter).

## 📌 Verificar versões atuais
- rustc --version
- cargo --version
- rustup --version

## Building

### Windows

```powershell
cargo build --release
```

The binary will be located at `Brute-Force-Rust Bip-44-49-84-Multi-Languages\target\release\brute_force_mnemonics.exe`.

### Linux / macOS

```
cargo build --release
```
## Ganho de velocidade
```
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

The binary will be located at `Brute-Force-Rust Bip-44-49-84-Multi-Languages/target/release/brute_force_mnemonics`.

## Usage

```
brute_force_mnemonics <TARGET_ADDRESS> <WORD1> <WORD2> ... <WORD12> [OPTIONS]
```

### Arguments 🧠

| Argument | Description |
|----------|-------------|
| `TARGET_ADDRESS` | Target legacy Bitcoin address (Base58, starting with `1`) |
| `WORD1..WORD12` | Exactly 12 BIP-39 words in any order |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--max-permutations` | `500000000` | Maximum number of permutations to test |
| `-h, --help` | | Print help |
| `-V, --version` | | Print version |

**Supported languages:** `english`, `portuguese`, `spanish`, `french`, `italian`, `czech`, `korean`, `japanese`, `chinese-simplified`, `chinese-traditional`

### Examples

📦**Windows:**
```
.\target\release\brute_force_mnemonics.exe
```
**12 words bip44:**
```
brute_force_mnemonics.exe 19iRjyeGSW6hqMawQpELHCchwwM2EVoHYk puzzle stove pepper laugh before deal shrimp dash mean toy poverty team --derivation 0 --max-permutations 479000000
```
**24 words bip44:**
```
brute_force_mnemonics.exe 185kMXVpPMCE4ozkzWosDohvgLcSf9hJ8n ozone fashion dinosaur safe key trash innocent accuse giraffe robot old leopard secret spot buddy animal husband stage unusual congress upper knock hero rotate --derivation 0 --max-permutations 479000000
```

🐧**Linux / macOS:**
```
./target/release/brute_force_mnemonics
```
**12 words bip44:**
```
./brute_force_mnemonics 19iRjyeGSW6hqMawQpELHCchwwM2EVoHYk puzzle stove pepper laugh before deal shrimp dash mean toy poverty team --derivation 0 --max-permutations 479000000
```
**24 words bip44:**
```
./brute_force_mnemonics 185kMXVpPMCE4ozkzWosDohvgLcSf9hJ8n ozone fashion dinosaur safe key trash innocent accuse giraffe robot old leopard secret spot buddy animal husband stage unusual congress upper knock hero rotate --derivation 0 --max-permutations 479000000
```

## How It Works

1. Generates permutations of the 12 provided words
2. For each permutation, validates it as a BIP-39 mnemonic
3. Derives the Bitcoin address using derivation path `m/44'/0'/0'/0/0`|`m/49'/0'/0'/0/0`|`m/84'/0'/0'/0/0`
4. Compares the derived P2PKH address against the target
5. Stops and outputs the correct phrase when a match is found

## Performance Notes

- 12 words have 479,001,600 (12!) possible permutations
- The default limit of 1,000,000 permutations covers ~0.2% of all possibilities
- Progress is logged every 1,000 permutations
- Invalid BIP-39 checksums are skipped automatically

## License

MIT
