mod utils;

use cosmwasm_std::Decimal;
use mantra_dex_sdk::Error;
use std::str::FromStr;
use std::time::Instant;
use utils::test_utils::create_test_client;

/// Test fee validation with valid fees (under 20% total)
#[tokio::test]
async fn test_valid_fee_structure() {
    let client = create_test_client().await;

    // Test valid fee structure (total = 5%)
    let result = client.create_validated_pool_fees(
        Decimal::percent(1),             // 1% protocol fee
        Decimal::percent(2),             // 2% swap fee
        Some(Decimal::percent(1)),       // 1% burn fee
        Some(vec![Decimal::percent(1)]), // 1% extra fee
    );

    assert!(result.is_ok(), "Valid fee structure should pass validation");

    let pool_fees = result.unwrap();
    assert_eq!(pool_fees.protocol_fee.share, Decimal::percent(1));
    assert_eq!(pool_fees.swap_fee.share, Decimal::percent(2));
    assert_eq!(pool_fees.burn_fee.share, Decimal::percent(1));
    assert_eq!(pool_fees.extra_fees.len(), 1);
    assert_eq!(pool_fees.extra_fees[0].share, Decimal::percent(1));
}

/// Test fee validation with maximum allowed fees (exactly 20%)
#[tokio::test]
async fn test_maximum_allowed_fees() {
    let client = create_test_client().await;

    // Test maximum allowed fee structure (total = 20%)
    let result = client.create_validated_pool_fees(
        Decimal::percent(5),             // 5% protocol fee
        Decimal::percent(10),            // 10% swap fee
        Some(Decimal::percent(3)),       // 3% burn fee
        Some(vec![Decimal::percent(2)]), // 2% extra fee
    );

    assert!(
        result.is_ok(),
        "Maximum allowed fee structure (20%) should pass validation"
    );
}

/// Test fee validation with excessive fees (over 20% total)
#[tokio::test]
async fn test_excessive_fee_structure() {
    let client = create_test_client().await;

    // Test excessive fee structure (total = 25%)
    let result = client.create_validated_pool_fees(
        Decimal::percent(10),            // 10% protocol fee
        Decimal::percent(10),            // 10% swap fee
        Some(Decimal::percent(3)),       // 3% burn fee
        Some(vec![Decimal::percent(2)]), // 2% extra fee
    );

    assert!(
        result.is_err(),
        "Excessive fee structure should fail validation"
    );

    if let Err(Error::FeeValidation(msg)) = result {
        assert!(
            msg.contains("exceed maximum allowed"),
            "Error should mention exceeding maximum"
        );
        assert!(
            msg.contains("0.25"),
            "Error should show the actual total (25%)"
        );
        assert!(
            msg.contains("0.2"),
            "Error should show the maximum allowed (20%)"
        );
    } else {
        panic!("Expected FeeValidation error");
    }
}

/// Test fee validation with multiple extra fees
#[tokio::test]
async fn test_multiple_extra_fees() {
    let client = create_test_client().await;

    // Test with multiple extra fees that sum to exactly 20% (should pass)
    let result = client.create_validated_pool_fees(
        Decimal::percent(5),       // 5% protocol fee
        Decimal::percent(5),       // 5% swap fee
        Some(Decimal::percent(2)), // 2% burn fee
        Some(vec![
            Decimal::percent(3), // 3% extra fee 1
            Decimal::percent(2), // 2% extra fee 2
            Decimal::percent(3), // 3% extra fee 3
        ]), // Total extra fees: 8%, Total: 20%
    );

    assert!(
        result.is_ok(),
        "Total fees of exactly 20% should pass validation (at the limit)"
    );

    // Test with multiple extra fees that exceed total
    let result = client.create_validated_pool_fees(
        Decimal::percent(5),       // 5% protocol fee
        Decimal::percent(5),       // 5% swap fee
        Some(Decimal::percent(2)), // 2% burn fee
        Some(vec![
            Decimal::percent(4), // 4% extra fee 1
            Decimal::percent(3), // 3% extra fee 2
            Decimal::percent(2), // 2% extra fee 3
        ]), // Total extra fees: 9%, Total: 21%
    );

    assert!(
        result.is_err(),
        "Excessive total fees should fail validation"
    );
}

/// Test fee validation with zero fees
#[tokio::test]
async fn test_zero_fees() {
    let client = create_test_client().await;

    // Test with all zero fees
    let result = client.create_validated_pool_fees(
        Decimal::zero(),       // 0% protocol fee
        Decimal::zero(),       // 0% swap fee
        Some(Decimal::zero()), // 0% burn fee
        Some(vec![]),          // No extra fees
    );

    assert!(result.is_ok(), "Zero fees should be valid");
}

/// Test fee validation with no optional fees
#[tokio::test]
async fn test_minimal_fee_structure() {
    let client = create_test_client().await;

    // Test with only required fees, no optional ones
    let result = client.create_validated_pool_fees(
        Decimal::percent(1), // 1% protocol fee
        Decimal::percent(2), // 2% swap fee
        None,                // No burn fee (defaults to 0%)
        None,                // No extra fees
    );

    assert!(result.is_ok(), "Minimal fee structure should be valid");

    let pool_fees = result.unwrap();
    assert_eq!(pool_fees.burn_fee.share, Decimal::zero());
    assert_eq!(pool_fees.extra_fees.len(), 0);
}

/// Test direct fee validation method
#[tokio::test]
async fn test_direct_fee_validation() {
    let client = create_test_client().await;

    // Create a valid fee structure manually
    let valid_fees = mantra_dex_std::fee::PoolFee {
        protocol_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(1),
        },
        swap_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(2),
        },
        burn_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(1),
        },
        extra_fees: vec![mantra_dex_std::fee::Fee {
            share: Decimal::percent(1),
        }],
    };

    let result = client.validate_pool_fees(&valid_fees);
    assert!(result.is_ok(), "Valid fees should pass direct validation");

    // Create an invalid fee structure manually
    let invalid_fees = mantra_dex_std::fee::PoolFee {
        protocol_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(10),
        },
        swap_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(10),
        },
        burn_fee: mantra_dex_std::fee::Fee {
            share: Decimal::percent(5),
        },
        extra_fees: vec![mantra_dex_std::fee::Fee {
            share: Decimal::percent(5),
        }],
    };

    let result = client.validate_pool_fees(&invalid_fees);
    assert!(
        result.is_err(),
        "Invalid fees should fail direct validation"
    );
}

/// Test enhanced fee structure validation and parsing (moved from migration tests)
#[tokio::test]
async fn test_enhanced_fee_structure() {
    let client = create_test_client().await;

    // Test fee structure with nested Fee objects
    let protocol_fee = Decimal::from_str("0.005").unwrap(); // 0.5%
    let swap_fee = Decimal::from_str("0.003").unwrap(); // 0.3%
    let burn_fee = Some(Decimal::from_str("0.001").unwrap()); // 0.1%
    let extra_fees = Some(vec![
        Decimal::from_str("0.001").unwrap(), // 0.1%
        Decimal::from_str("0.002").unwrap(), // 0.2%
    ]);

    // Test fee validation with new structure
    let result = client.create_validated_pool_fees(protocol_fee, swap_fee, burn_fee, extra_fees);

    match result {
        Ok(fees) => {
            assert_eq!(fees.protocol_fee.share, Decimal::from_str("0.005").unwrap());
            assert_eq!(fees.swap_fee.share, Decimal::from_str("0.003").unwrap());
            assert_eq!(fees.burn_fee.share, Decimal::from_str("0.001").unwrap());
            assert_eq!(fees.extra_fees.len(), 2);
            assert_eq!(
                fees.extra_fees[0].share,
                Decimal::from_str("0.001").unwrap()
            );
            assert_eq!(
                fees.extra_fees[1].share,
                Decimal::from_str("0.002").unwrap()
            );
        }
        Err(e) => {
            panic!("Enhanced fee structure validation failed: {:?}", e);
        }
    }

    // Test fee validation with excessive fees (should fail)
    let excessive_protocol_fee = Decimal::from_str("0.15").unwrap(); // 15%
    let excessive_swap_fee = Decimal::from_str("0.10").unwrap(); // 10%
    let excessive_burn_fee = Some(Decimal::from_str("0.05").unwrap()); // 5%
                                                                       // Total: 30% > 20% limit

    let result = client.create_validated_pool_fees(
        excessive_protocol_fee,
        excessive_swap_fee,
        excessive_burn_fee,
        None,
    );

    assert!(
        result.is_err(),
        "Fee validation should fail for excessive fees"
    );

    match result {
        Err(Error::FeeValidation(msg)) => {
            assert!(msg.contains("exceed maximum allowed"));
        }
        Err(e) => {
            panic!("Expected fee validation error, got: {:?}", e);
        }
        Ok(_) => {
            panic!("Fee validation should have failed for excessive fees");
        }
    }
}

/// Test fee validation performance (moved from migration tests)
#[tokio::test]
async fn test_fee_validation_performance() {
    let client = create_test_client().await;

    // Test fee validation performance
    let start = Instant::now();
    let protocol_fee = Decimal::from_str("0.01").unwrap();
    let swap_fee = Decimal::from_str("0.01").unwrap();
    let burn_fee = Some(Decimal::from_str("0.01").unwrap());

    for _ in 0..1000 {
        let _result = client.create_validated_pool_fees(protocol_fee, swap_fee, burn_fee, None);
    }
    let validation_time = start.elapsed();

    // Fee validation should be very fast (under 1 second for 1000 iterations)
    assert!(
        validation_time.as_millis() < 1000,
        "Fee validation too slow: {:?}",
        validation_time
    );
}
