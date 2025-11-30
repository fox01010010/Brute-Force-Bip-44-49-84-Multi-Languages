# 🧠 Brute-Force-Rust Bip-44-49-84-Multi-Languages

## Use Case

If you have 12/24 BIP-39 mnemonic words but don't remember the correct order, this tool will brute-force permutations to find the combination that derives to your known Bitcoin address-Bip-44-49-84.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.70 or later recommended)

## Building

### Windows

```powershell
cargo build --release
```

The binary will be located at `Brute-Force-Rust Bip-44-49-84-Multi-Languages\target\release\brute_force_mnemonics.exe`.

### Linux / macOS
**Clone the repository and enter the folder:**
```bash
git clone https://github.com/fox01010010/fox_crypto.git && cd Brute-Force-Rust Bip-44-49-84-Multi-Languages
```
 
```bash
cargo build --release
```

```bash
cd target/release
```

The binary will be located at `Brute-Force-Rust Bip-44-49-84-Multi-Languages/target/release/brute_force_mnemonics`.

## Usage

```
brute_force_mnemonics <TARGET_ADDRESS> <WORD1> <WORD2> ... <WORD12> [OPTIONS]
```

## Address Types 1, 3, bc1q and Language are Detected Automatically.

### 🧠 Arguments

| Argument | Description |
|----------|-------------|
| `TARGET_ADDRESS` | Target legacy Bitcoin address (Base58, starting with `1`, `3`, `bc1q`) |
| `WORD1..WORD12` | Exactly 12 BIP-39 words in any order |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--max-permutations` | `500000000` | Maximum number of permutations to test |
| `--derivation 0,1,2,3,4,5` | 
| `-h, --help` | | Print help |
| `-V, --version` | | Print version |

**Supported languages:** `english`, `portuguese`, `spanish`, `french`, `italian`, `czech`, `korean`, `japanese`, `chinese-simplified`, `chinese-traditional`

### Examples

📦**Windows:**
```powershell
.\target\release\brute_force_mnemonics.exe
```
**12 words bip44:**
```
brute_force_mnemonics.exe 19iRjyeGSW6hqMawQpELHCchwwM2EVoHYk puzzle stove pepper laugh before deal shrimp dash mean toy poverty team --derivation 0 --max-permutations 500000000
```
**24 words bip44:**
```
brute_force_mnemonics.exe 185kMXVpPMCE4ozkzWosDohvgLcSf9hJ8n ozone fashion dinosaur safe key trash innocent accuse giraffe robot old leopard secret spot buddy animal husband stage unusual congress upper knock hero rotate --derivation 0 --max-permutations 500000000
```
**12 words bip49:**
```
brute_force_mnemonics.exe 3NHae8WHBCXsPChBngS8MwGP4fz6cVbVng strategy load plug list dinosaur coast mean pledge close reduce they few --derivation 0 --max-permutations 500000000
```
**24 words bip49:**
```
brute_force_mnemonics.exe 3FoukJdvyLPmKnAW5NBzYYqCrSpuZ4wJuc boost bright purse crime quick decline talk network desert session say program click food tackle daughter milk all suggest notable culture defy that coconut --derivation 0 --max-permutations 500000000
```
**12 words bip84:**
```
brute_force_mnemonics.exe bc1q83eyddcayz40c73vkn5asl86duw54kz99d3sz4 waste float want romance soccer torch metal mule era purse yellow random --derivation 0 --max-permutations 500000000
```
**24 words bip84:**
```
brute_force_mnemonics.exe bc1qeyyrletmtv7k8lszthsqeqyd55nhf5a9tty3ry coil analyst shallow patrol crime sad fatal review risk present judge mix farm trust decline strike make will pave tongue slam since page goddess --derivation 0 --max-permutations 500000000
```
**12 words bip49 Language: portuguese:**
```
brute_force_mnemonics.exe 3CiLCrfNJ7ekbNQ9149beLu7HJuhTgMypk cabide seringa cogumelo pacato sonegar reduzida incolor roedor dentista decretar turbo circuito --derivation 1 --max-permutations 500000000
```


🐧**Linux / macOS:**
```bash
./target/release/brute_force_mnemonics
```
**12 words bip44:**
```
./brute_force_mnemonics 19iRjyeGSW6hqMawQpELHCchwwM2EVoHYk puzzle stove pepper laugh before deal shrimp dash mean toy poverty team --derivation 0 --max-permutations 500000000
```
**24 words bip44:**
```
./brute_force_mnemonics 185kMXVpPMCE4ozkzWosDohvgLcSf9hJ8n ozone fashion dinosaur safe key trash innocent accuse giraffe robot old leopard secret spot buddy animal husband stage unusual congress upper knock hero rotate --derivation 0 --max-permutations 500000000
```
**12 words bip49:**
```
./brute_force_mnemonics 3NHae8WHBCXsPChBngS8MwGP4fz6cVbVng strategy load plug list dinosaur coast mean pledge close reduce they few --derivation 0 --max-permutations 500000000
```
**24 words bip49:**
```
./brute_force_mnemonics 3FoukJdvyLPmKnAW5NBzYYqCrSpuZ4wJuc boost bright purse crime quick decline talk network desert session say program click food tackle daughter milk all suggest notable culture defy that coconut --derivation 0 --max-permutations 500000000
```
**12 words bip84:**
```
./brute_force_mnemonics bc1q83eyddcayz40c73vkn5asl86duw54kz99d3sz4 waste float want romance soccer torch metal mule era purse yellow random --derivation 0 --max-permutations 500000000
```
**24 words bip84:**
```
./brute_force_mnemonics bc1qeyyrletmtv7k8lszthsqeqyd55nhf5a9tty3ry coil analyst shallow patrol crime sad fatal review risk present judge mix farm trust decline strike make will pave tongue slam since page goddess --derivation 0 --max-permutations 500000000
```
**12 words bip49 Language: portuguese:**
```
./brute_force_mnemonics 3CiLCrfNJ7ekbNQ9149beLu7HJuhTgMypk cabide seringa cogumelo pacato sonegar reduzida incolor roedor dentista decretar turbo circuito --derivation 1 --max-permutations 500000000
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

## Original Code

(https://github.com/lmajowka/words-breaker)
