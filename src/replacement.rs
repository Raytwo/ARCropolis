pub mod extensions;
pub mod lookup;

pub mod addition;
// pub mod config;
pub mod preprocess;
mod stream;
mod threads;
mod uncompressed;
pub mod unshare;

pub use extensions::*;

pub fn install() {
    stream::install();
    threads::install();
    uncompressed::install();
}
