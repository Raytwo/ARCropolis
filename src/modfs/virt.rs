use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use smash_arc::Hash40;

use crate::fs::loaders::ApiCallback;

pub struct VirtualEntry {
    pub callback: ApiCallback,
    pub max_size: usize,
}

pub struct VirtualChain {
    entries: Vec<VirtualEntry>,
    cursor: AtomicUsize,
}

impl VirtualChain {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: AtomicUsize::new(0),
        }
    }

    pub fn push(&mut self, entry: VirtualEntry) {
        self.entries.insert(0, entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn max_size(&self) -> usize {
        self.entries.iter().map(|e| e.max_size).max().unwrap_or(0)
    }

    pub fn entries(&self) -> &[VirtualEntry] {
        &self.entries
    }

    pub fn take_next(&self) -> Option<&VirtualEntry> {
        loop {
            let i = self.cursor.load(Ordering::SeqCst);
            let entry = self.entries.get(i)?;
            if self.cursor.compare_exchange(i, i + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                return Some(entry);
            }
        }
    }

    pub fn release(&self) {
        let _ = self
            .cursor
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| if c > 0 { Some(c - 1) } else { None });
    }
}

#[derive(Default)]
pub struct VirtualLayer {
    chains: HashMap<Hash40, VirtualChain>,
}

impl VirtualLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, hash: Hash40, entry: VirtualEntry) {
        self.chains.entry(hash).or_insert_with(VirtualChain::new).push(entry);
    }

    pub fn chain(&self, hash: Hash40) -> Option<&VirtualChain> {
        self.chains.get(&hash)
    }

    pub fn contains(&self, hash: Hash40) -> bool {
        self.chains.contains_key(&hash)
    }

    pub fn max_size(&self, hash: Hash40) -> Option<usize> {
        self.chains.get(&hash).map(|c| c.max_size())
    }
}
