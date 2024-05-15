use good_lp::{highs, variables, Solution, SolverModel};

pub fn solve() -> Option<(f64, f64)> {
    // Create variables in a readable format with a macro...
    variables! {
    vars:
        a <= 1;
        2 <= b <= 4;
    }

    // ... or add variables programmatically
    // vars.add(variable().min(2).max(9));

    let solution = vars
        .maximise(10 * (a - b / 5) - b)
        .using(highs)
        .with(a + 2. << b) // or (a + 2).leq(b)
        .with(1 + a >> 4. - b)
        .solve()
        .ok()?;

    Some((solution.value(a), solution.value(b)))
}

pub fn run() {
    println!("Hello from MUSE 2.0!");

    let (a, b) = solve().expect("Failed to compute solution");

    println!("Calculated solution: a = {}, b = {}", a, b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_float_eq::*;

    #[test]
    fn test_solve() {
        let (a, b) = solve().unwrap();
        assert_f64_near!(a, 1.);
        assert_f64_near!(b, 3.);
    }
}
