//! P4B RED-first fixed-point accounting tests.

use crate::model_accounting::{
    ModelAccountingError, ModelPriceSnapshot, UsdMicros, calculate_cost_usd_micros,
    calculate_reservation_usd_micros, validate_price_snapshot,
};
use serea_protocol::CostClass;

fn micros(value: u64) -> UsdMicros {
    UsdMicros::new(value).unwrap()
}

#[test]
fn cost_rounds_each_component_up_independently() {
    assert_eq!(
        calculate_cost_usd_micros(0, 0, 900_000, 800_000),
        Ok(micros(0))
    );
    assert_eq!(calculate_cost_usd_micros(9, 7, 0, 0), Ok(micros(0)));
    assert_eq!(
        calculate_cost_usd_micros(2, 3, 500_000, 1_000_000),
        Ok(micros(4))
    );
    assert_eq!(calculate_cost_usd_micros(1, 0, 1, 0), Ok(micros(1)));
    // Both 1/1e6 components round up separately: 1 + 1, not 1.
    assert_eq!(calculate_cost_usd_micros(1, 1, 1, 1), Ok(micros(2)));
}

#[test]
fn cost_accepts_large_safe_values_and_rejects_result_overflow() {
    assert_eq!(
        calculate_cost_usd_micros(1_000_000, 1_000_000, i64::MAX as u64, 0),
        Ok(micros(i64::MAX as u64))
    );
    assert_eq!(
        calculate_cost_usd_micros(i64::MAX as u64, 0, 1_000_000, 0),
        Ok(micros(i64::MAX as u64))
    );
    assert_eq!(
        calculate_cost_usd_micros(i64::MAX as u64, 0, u64::MAX, 0),
        Err(ModelAccountingError::Overflow)
    );
    assert_eq!(
        calculate_cost_usd_micros(i64::MAX as u64, 1, 1_000_000, 1_000_000),
        Err(ModelAccountingError::Overflow)
    );
}

#[test]
fn reservation_uses_maximum_context_and_output_capacity() {
    assert_eq!(
        calculate_reservation_usd_micros(8_192, 2_048, 3_000_000, 6_000_000),
        Ok(micros(36_864))
    );
    assert_eq!(
        calculate_reservation_usd_micros(8_192, 2_048, 0, 0),
        Ok(micros(0))
    );
    assert_eq!(
        calculate_reservation_usd_micros(u64::MAX, u64::MAX, u64::MAX, u64::MAX),
        Err(ModelAccountingError::Overflow)
    );
}

#[test]
fn free_price_requires_zero_rates_and_unknown_is_not_free() {
    let free = ModelPriceSnapshot::new(CostClass::Free, "price-1", 0, 0);
    assert_eq!(validate_price_snapshot(&free), Ok(()));
    let contradictory = ModelPriceSnapshot::new(CostClass::Free, "price-1", 1, 0);
    assert_eq!(
        validate_price_snapshot(&contradictory),
        Err(ModelAccountingError::InvalidPriceSnapshot)
    );
    let paid = ModelPriceSnapshot::new(CostClass::Paid, "price-1", 1, 1);
    assert_eq!(validate_price_snapshot(&paid), Ok(()));
}
