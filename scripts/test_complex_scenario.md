# Test Script: Complex DEX Scenario Test

## Setup
- Network: mantra-dukong
- Wallet: use test wallet with sufficient balance

## Steps
1. **Validate network** connectivity
2. **Check wallet balance** for ATOM, USDC, and USDT
3. **Get available pools** and identify best trading pairs
4. **Execute swap** of 50 ATOM for USDC with 2% slippage
5. **Provide liquidity** to USDC/USDT pool
   - asset_a_amount: 500
   - asset_b_amount: 500
6. **Get LP token balance** for the USDC/USDT pool
7. **Execute another swap** of 25 USDC for USDT with 1% slippage
8. **Withdraw liquidity** from USDC/USDT pool
   - lp_amount: 50
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