# Test Script: Liquidity Provision Test

## Setup
- Network: mantra-dukong
- Wallet: use test wallet with sufficient balance for both assets

## Steps
1. **Check wallet balance** for ATOM and USDC
2. **Get pool information** for ATOM/USDC pair
   - pool_id: 1
3. **Fetch current pool liquidity** and store as `pool_liquidity_before`
4. **Provide liquidity** to the pool
   - asset_a_amount: 100
   - asset_b_amount: 100
5. **Fetch updated pool liquidity** and store as `pool_liquidity_after`
6. **Get LP token balance** for the pool
7. **Verify liquidity** was added successfully
   - Assert: `pool_liquidity_after` > `pool_liquidity_before`

## Expected Results
- Liquidity provision should complete successfully
- LP tokens should be received
- Pool should reflect increased liquidity

## Metadata
- Author: Claude Code
- Version: 1.0
- Category: Liquidity Management