// No console window behind the toy.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod cards;
mod gpu_types;
mod party;
mod settings;

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod gpu;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    env_logger::init();
    if win::already_running() {
        return;
    }
    if let Err(e) = app::run() {
        log::error!("KittyParty stopped: {e}");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("KittyParty is a Windows desktop toy.");
}
