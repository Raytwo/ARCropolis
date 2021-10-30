pub mod lookup;
mod uncompressed;
mod threads;

pub fn install() {
    uncompressed::install();
    threads::install();
}