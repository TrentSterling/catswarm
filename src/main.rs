mod app;
mod cat;
mod click;
mod daynight;
mod debug;
mod ecs;
mod heatmap;
mod mode;
mod platform;
mod render;
mod spatial;
mod toy;


fn main() {
    env_logger::init();
    log::info!("PetToy starting up");

    if let Err(e) = app::run() {
        log::error!("Fatal error: {e}");
        std::process::exit(1);
    }
}
