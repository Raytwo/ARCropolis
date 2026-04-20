use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use smash_arc::Hash40;

use crate::PathExtension;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub root: PathBuf,
    pub size: usize,
}

impl FileEntry {
    pub fn full_path(&self, local: &Path) -> PathBuf {
        self.root.join(local)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FileEntryRef<'a> {
    pub root: &'a Path,
    pub size: usize,
}

impl<'a> FileEntryRef<'a> {
    pub fn full_path(&self, local: &Path) -> PathBuf {
        self.root.join(local)
    }
}

#[derive(Clone, Debug)]
struct InternalEntry {
    local: PathBuf,
    root_idx: u32,
    size: u32,
}

#[derive(Debug)]
pub struct Conflict {
    pub local: PathBuf,
    pub winning_root: PathBuf,
    pub losing_root: PathBuf,
}

#[derive(Default)]
pub struct PatchLayer {
    entries: Vec<InternalEntry>,
    roots: Vec<PathBuf>,
    root_lookup: HashMap<PathBuf, u32>,
    index: HashMap<Hash40, u32>,
    alternates: HashMap<PathBuf, Vec<FileEntry>>,
    conflicts: Vec<Conflict>,
}

impl PatchLayer {
    pub fn new() -> Self {
        Self::default()
    }

    fn intern_root(&mut self, root: PathBuf) -> u32 {
        if let Some(&idx) = self.root_lookup.get(&root) {
            return idx;
        }
        let idx = self.roots.len() as u32;
        self.roots.push(root.clone());
        self.root_lookup.insert(root, idx);
        idx
    }

    fn path_key(local: &Path) -> Hash40 {
        local.smash_hash().unwrap_or(Hash40(0))
    }

    fn to_ref(&self, idx: u32) -> FileEntryRef<'_> {
        let entry = &self.entries[idx as usize];
        FileEntryRef {
            root: self.roots[entry.root_idx as usize].as_path(),
            size: entry.size as usize,
        }
    }

    pub fn insert(&mut self, local: PathBuf, entry: FileEntry, hash: Option<Hash40>) {
        let root_idx = self.intern_root(entry.root);
        let size = entry.size as u32;
        let path_key = Self::path_key(&local);

        let entry_idx = if let Some(&idx) = self.index.get(&path_key) {
            let slot = &mut self.entries[idx as usize];
            slot.local = local;
            slot.root_idx = root_idx;
            slot.size = size;
            idx
        } else {
            let idx = self.entries.len() as u32;
            self.entries.push(InternalEntry { local, root_idx, size });
            self.index.insert(path_key, idx);
            idx
        };

        if let Some(h) = hash {
            if h != path_key {
                self.index.insert(h, entry_idx);
            }
        }
    }

    pub fn record_conflict(&mut self, conflict: Conflict) {
        self.conflicts.push(conflict);
    }

    pub fn get(&self, local: &Path) -> Option<FileEntryRef<'_>> {
        let idx = *self.index.get(&Self::path_key(local))?;
        Some(self.to_ref(idx))
    }

    pub fn entry_for_hash(&self, hash: Hash40) -> Option<(&Path, FileEntryRef<'_>)> {
        let idx = *self.index.get(&hash)?;
        let entry = &self.entries[idx as usize];
        Some((entry.local.as_path(), self.to_ref(idx)))
    }

    pub fn contains(&self, local: &Path) -> bool {
        self.index.contains_key(&Self::path_key(local))
    }

    pub fn iter_files(&self) -> impl Iterator<Item = (&Path, FileEntryRef<'_>)> {
        (0..self.entries.len() as u32).map(move |idx| {
            let entry = &self.entries[idx as usize];
            (entry.local.as_path(), self.to_ref(idx))
        })
    }

    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
    }

    pub fn alternates_mut(&mut self) -> &mut HashMap<PathBuf, Vec<FileEntry>> {
        &mut self.alternates
    }

    pub fn num_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn num_roots(&self) -> usize {
        self.roots.len()
    }

    pub fn estimated_bytes(&self) -> usize {
        let path_buf_size = |pb: &PathBuf| 24 + pb.capacity();
        let entries_bytes: usize = self
            .entries
            .iter()
            .map(|e| std::mem::size_of::<InternalEntry>() + path_buf_size(&e.local))
            .sum();
        let roots_bytes: usize = self.roots.iter().map(path_buf_size).sum();
        let root_lookup_bytes: usize = self
            .root_lookup
            .keys()
            .map(path_buf_size)
            .sum::<usize>()
            + self.root_lookup.len() * (std::mem::size_of::<u32>() + 32);
        let index_bytes: usize = self.index.len() * 32;
        entries_bytes + roots_bytes + root_lookup_bytes + index_bytes
    }
}
