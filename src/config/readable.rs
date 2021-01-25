use std::ops::Deref;

use parking_lot::RwLockReadGuard;

use crate::config::Config;

pub struct ReadableConfig<'rwlock>(RwLockReadGuard<'rwlock, Config>);

impl<'rwlock> ReadableConfig<'rwlock> {
    pub fn new(config: RwLockReadGuard<'rwlock, Config>) -> Self {
        ReadableConfig(config)
    }
}

impl Deref for ReadableConfig<'_> {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}