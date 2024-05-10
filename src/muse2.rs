use assert_float_eq::*; //assert_f64_near;
use good_lp::{default_solver, variables, Solution, SolverModel};

pub fn solve() -> Option<()> {
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
        .using(default_solver)
        .with(a + 2. << b) // or (a + 2).leq(b)
        .with(1 + a >> 4. - b)
        .solve()
        .ok()?;

    assert_f64_near!(solution.value(a), 1.); //, abs <= 1e-8);
    assert_f64_near!(solution.value(b), 3.); //, abs <= 1e-8);

    Some(())
}

pub fn run() {
    println!("Hello from MUSE 2.0!");

    solve().expect("Failed to compute solution");
}
