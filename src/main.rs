mod app;
mod cat;
mod ecs;
mod platform;
mod render;
mod spatial;
mod util;

fn main() {
    env_logger::init();
    log::info!("PetToy starting up");

    if let Err(e) = app::run() {
        log::error!("Fatal error: {e}");
        std::process::exit(1);
    }
}
