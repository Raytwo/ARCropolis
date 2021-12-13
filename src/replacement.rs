pub mod extensions;
pub mod lookup;

pub mod config;
mod stream;
mod threads;
mod uncompressed;
pub mod unshare;
pub mod preprocess;

pub use extensions::*;

pub fn install() {
    stream::install();
    threads::install();
    uncompressed::install();
}