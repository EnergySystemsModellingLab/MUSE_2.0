//! A regression test for the "muse1_default" example
mod regression;
use regression::run_regression_test;

#[test]
fn test_regression_two_regions() {
    run_regression_test("two_regions")
}
