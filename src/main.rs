mod demand;
mod simulation;

fn main() {
    // Initialize the simulation
    let data = simulation::initialize_simulation();
    simulation::run(data);
}
