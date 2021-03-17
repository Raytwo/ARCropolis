use std::ops::{Deref, DerefMut};

use parking_lot::RwLockWriteGuard;

use crate::config::Config;

pub struct WriteableConfig<'rwlock>(RwLockWriteGuard<'rwlock, Config>);

impl<'rwlock> WriteableConfig<'rwlock> {
    pub fn new(config: RwLockWriteGuard<'rwlock, Config>) -> Self {
        WriteableConfig(config)
    }
}

impl Deref for WriteableConfig<'_> {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl DerefMut for WriteableConfig<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

impl Drop for WriteableConfig<'_> {
    fn drop(&mut self) {
        self.deref_mut().save().unwrap();
    }
}
