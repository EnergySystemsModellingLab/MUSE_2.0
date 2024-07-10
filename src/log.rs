use std::env;

/// Initialise the program logger
pub fn init() {
    // Log at the info level by default, unless the user has specified otherwise
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }

    // Initialise logger
    env_logger::init();
}
