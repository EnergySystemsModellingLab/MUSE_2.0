//! A regression test for the "simple" example
mod regression;
use regression::run_regression_test;

/// Don't run regression test while we're reworking the optimisation model.
///
/// Should be re-enabled when we're finished:
///  https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/594
#[ignore]
#[test]
fn test_regression_simple() {
    run_regression_test("simple")
}
