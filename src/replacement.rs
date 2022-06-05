pub mod extensions;
pub mod lookup;

pub mod addition;
pub mod config;
pub mod preprocess;
mod stream;
mod threads;
mod uncompressed;
pub mod unshare;

pub use extensions::*;
use owo_colors::OwoColorize;
use smash_arc::{Hash40, ArcLookup};

pub fn install() {
    stream::install();
    threads::install();
    uncompressed::install();
}
