use good_lp::{
    highs, solvers::highs::HighsSolution, variables, Constraint, Expression, ProblemVariables,
    Solution, SolverModel,
};

fn solve(
    vars: ProblemVariables,
    objective: Expression,
    constraints: Vec<Constraint>,
) -> Option<HighsSolution> {
    let mut model = vars.maximise(objective).using(highs);
    for constraint in constraints.into_iter() {
        model.add_constraint(constraint);
    }

    model.solve().ok()
}

fn solve_toy_problem() -> (f64, f64) {
    variables! {
    vars:
        a <= 1;
        2 <= b <= 4;
    }

    let objective = 10 * (a - b / 5) - b;
    let constraints = vec![(a + 2.) << b, (1 + a) >> (4. - b)];

    let solution = solve(vars, objective, constraints).unwrap();

    (solution.value(a), solution.value(b))
}

pub fn run() {
    let (a, b) = solve_toy_problem();

    println!("Calculated solution: a = {}, b = {}", a, b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_float_eq::*;

    #[test]
    fn test_solve() {
        let (a, b) = solve_toy_problem();
        assert_f64_near!(a, 1.);
        assert_f64_near!(b, 3.);
    }
}
