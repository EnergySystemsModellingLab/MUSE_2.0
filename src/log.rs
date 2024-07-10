use env_logger::Env;

/// Initialise the program logger
pub fn init() {
    // Log at the info level by default, unless the user has specified otherwise
    let env = Env::new()
        .filter_or("MUSE2_LOG_LEVEL", "info")
        .write_style("MUSE2_LOG_STYLE");

    // Initialise logger
    env_logger::init_from_env(env);
}
