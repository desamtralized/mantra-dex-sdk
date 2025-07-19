# Test Script: Complex DEX Scenario Test

## Setup
- Network: mantra-dukong
- Wallet: use test wallet with sufficient balance

## Steps
1. **Validate network** connectivity
2. **Check wallet balance** for OM, USDY, and USDT
3. **Get available pools** and identify best trading pairs
4. **Execute swap** of 50 OM for USDC with 2% slippage
   - pool_id: o.uom.usdc.pool
   - offer_asset_denom: uom
   - offer_asset_amount: 50000000
   - ask_asset_denom: factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDC
5. **Provide liquidity** to USDY/USDT pool
   - pool_id: p.10
   - asset_a_denom: factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY
   - asset_a_amount: 50000000
   - asset_b_denom: ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239
   - asset_b_amount: 50000000
6. **Get LP token balance** for the USDY/USDT pool (p.10)
7. **Execute another swap** of 25 USDY for USDT with 1% slippage
   - pool_id: p.10
   - offer_asset_denom: factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY
   - offer_asset_amount: 25000000
   - ask_asset_denom: ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239
8. **Withdraw liquidity** from USDY/USDT pool
   - pool_id: p.10
   - lp_amount: 50000000
9. **Verify final balances** show expected amounts
10. **Get all LP token balances** for summary

## Expected Results
- All operations should complete successfully
- Final balances should reflect all trades and liquidity operations
- No failed transactions
- LP tokens should be properly managed

## Metadata
- Author: Claude Code
- Version: 1.0
- Category: Complex Trading