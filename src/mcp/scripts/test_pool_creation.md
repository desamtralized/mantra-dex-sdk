# Test Script: Pool Creation Test

## Setup
- Network: mantra-dukong
- Wallet: use admin wallet with pool creation privileges

## Steps
1. **Validate network** connectivity
2. **Get contract addresses** for verification
3. **Create pool** for new asset pair and capture returned pool_id
   - pool_type: constant_product
   - asset_a_denom: uom
   - asset_a_amount: 1000000000 (1000 OM)
   - asset_b_denom: factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY
   - asset_b_amount: 1000000000 (1000 USDY)
   - Store returned pool_id as `created_pool_id`
4. **Verify pool creation** was successful
5. **Get pool information** using the stored `created_pool_id`

## Expected Results
- Pool creation should complete successfully
- New pool should be accessible via queries
- Pool should have correct asset configuration

## Metadata
- Author: Claude Code
- Version: 1.0
- Category: Pool Management