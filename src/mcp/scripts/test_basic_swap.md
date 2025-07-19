# Test Script: Basic Swap Test

## Setup
- Network: mantra-dukong  
- Wallet: use test wallet with sufficient balance

## Steps
1. **Check wallet balance** for OM and USDT
2. **Get available pools** and find USDY/USDT pair
3. **Execute swap** of 10 USDY for USDT with 1% slippage
   - pool_id: p.10
   - offer_asset_denom: factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY
   - offer_asset_amount: 10000000 (10 USDY in micro units)
   - ask_asset_denom: ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239
4. **Monitor transaction** until confirmed
5. **Verify balance** shows received USDT

## Expected Results
- Swap should complete successfully
- Balance should reflect the trade
- Transaction should confirm within 30 seconds

## Metadata
- Author: Claude Code
- Version: 1.0
- Category: Basic Trading