//! A regression test for the "simple_mc" example
mod regression;
use regression::run_regression_test;

#[test]
fn test_regression_simple_mc() {
    run_regression_test("simple_mc")
}
