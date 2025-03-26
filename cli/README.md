# Mantra DEX CLI

A command-line interface for interacting with the Mantra DEX blockchain.

## Features

- Wallet management with password encryption
- Pool operations
- Token swaps
- Liquidity operations
- Balance queries

## Installation

### Building from source

1. Clone the repository:
```bash
git clone https://github.com/your-org/mantra-dex-sdk.git
cd mantra-dex-sdk
```

2. Build the CLI:
```bash
cargo build --release -p mantra-dex-cli
```

3. The binary will be available at `target/release/mantra-dex`.

## Configuration

The CLI uses a configuration file located at `~/.config/mantra-dex-cli/config.toml`. You can specify a custom configuration file with the `--config` option.

To set up your configuration:

1. Copy the example configuration file:
```bash
mkdir -p ~/.config/mantra-dex-cli
cp cli/config.toml.example ~/.config/mantra-dex-cli/config.toml
```

2. Edit the configuration file:
```bash
vim ~/.config/mantra-dex-cli/config.toml
```

3. Update the network settings.

## Wallet Security

The CLI uses strong encryption to protect wallet mnemonics:

- All wallet mnemonics are encrypted using ChaCha20-Poly1305 authenticated encryption
- Key derivation is performed using Argon2 password hashing
- Each wallet can have its own unique password
- Wallets must be unlocked with the correct password before use
- Mnemonics are never stored in plain text

## Usage

### Wallet Commands

Create a new wallet (you'll be prompted to set a password):
```bash
mantra-dex wallet create --name my_wallet
```

Import an existing wallet (you'll be prompted for the mnemonic and password):
```bash
mantra-dex wallet import --name my_wallet
```

List all wallets:
```bash
mantra-dex wallet list
```

Get wallet info (requires password):
```bash
mantra-dex wallet info
```

Set active wallet:
```bash
mantra-dex wallet use my_wallet
```

Export wallet mnemonic (requires password):
```bash
mantra-dex wallet export
```

### Pool Commands

List all pools:
```bash
mantra-dex pool list
```

Get pool info:
```bash
mantra-dex pool info pool123
```

### Swap Commands

Swap tokens (will prompt for wallet password):
```bash
mantra-dex swap --pool-id pool123 --offer-asset 1000:uom --ask-denom uusdt
```

Simulate a swap:
```bash
mantra-dex swap --pool-id pool123 --offer-asset 1000:uom --ask-denom uusdt --simulate
```

### Liquidity Commands

Provide liquidity:
```bash
mantra-dex liquidity provide --pool-id pool123 --assets 1000:uom,5000:uusdt
```

Withdraw liquidity:
```bash
mantra-dex liquidity withdraw --pool-id pool123 --amount 500
```

### Balance Commands

Check wallet balances:
```bash
mantra-dex balance
```

Filter by denom:
```bash
mantra-dex balance --denom uom
```

## License

[License details] 