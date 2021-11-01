pub mod extensions;
pub mod lookup;

pub mod config;
mod threads;
mod uncompressed;

pub use extensions::*;

pub fn install() {
    uncompressed::install();
    threads::install();
}