use std::{
    alloc::Layout,
    ops::{Index, IndexMut, Range},
    ptr::null,
};

#[derive(Debug)]
#[repr(C)]
pub struct CppVector<T> {
    start: *mut T,
    end: *mut T,
    eos: *mut T,
}

impl<T> Default for CppVector<T> {
    fn default() -> Self {
        CppVector::new()
    }
}

impl<T> CppVector<T> {
    unsafe fn realloc(&mut self) {
        let current_capacity = self.eos.offset_from(self.start) as usize;
        let current_len = self.end.offset_from(self.start) as usize;
        let layout = Layout::from_size_align(current_capacity * 2 * std::mem::size_of::<T>(), 1).unwrap();
        let (new_start, new_eos) = {
            let start = std::alloc::alloc(layout) as *mut T;
            (start, start.add(current_capacity * 2))
        };
        std::ptr::copy_nonoverlapping(self.start, new_start, current_len);
        std::alloc::dealloc(
            self.start as _,
            Layout::from_size_align(current_capacity * std::mem::size_of::<T>(), 1).unwrap(),
        );
        self.start = new_start;
        self.end = self.start.add(current_len);
        self.eos = new_eos;
    }

    pub fn new() -> Self {
        Self {
            start: null::<T>() as _,
            end: null::<T>() as _,
            eos: null::<T>() as _,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let layout = Layout::from_size_align(cap * std::mem::size_of::<T>(), 1).unwrap();
        let (start, eos) = unsafe {
            let start = std::alloc::alloc(layout) as *mut T;
            (start, start.add(cap))
        };
        Self { start, end: start, eos }
    }

    pub fn push(&mut self, val: T) {
        unsafe {
            if self.end.add(1) > self.eos {
                self.realloc();
            }
            *self.end = val;
            self.end = self.end.add(1);
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        unsafe {
            if self.end.add(additional) > self.eos {
                self.realloc();
            }
        }
    }

    pub fn iter(&self) -> CppVectorIterator<T> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> CppVectorIteratorMut<T> {
        self.into_iter()
    }

    pub fn len(&self) -> usize {
        ((self.end as usize) - (self.start as usize)) / std::mem::size_of::<T>()
    }

    pub fn as_ptr(&self) -> *const T {
        self.start
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.start
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe {
            let len = self.end.offset_from(self.start) as usize;
            std::slice::from_raw_parts(self.start, len)
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            let len = self.end.offset_from(self.start) as usize;
            std::slice::from_raw_parts_mut(self.start, len)
        }
    }

    pub fn extend_from_slice(&mut self, slice: &[T])
    where
        T: Copy + Clone,
    {
        unsafe {
            if self.end.add(slice.len()) > self.eos {
                self.realloc();
                self.extend_from_slice(slice);
            } else {
                std::ptr::copy_nonoverlapping(slice.as_ptr(), self.end, slice.len());
                self.end = self.end.add(slice.len());
            }
        }
    }
}

impl<T: Copy + Clone> CppVector<T> {
    pub fn from_slice(slice: &[T]) -> Self {
        let layout = Layout::from_size_align(slice.len() * std::mem::size_of::<T>(), 1).unwrap();
        let (start, eos) = unsafe {
            let start = std::alloc::alloc(layout) as *mut T;
            (start, start.add(slice.len()))
        };
        let new_slice = unsafe { std::slice::from_raw_parts_mut(start, slice.len()) };
        new_slice.copy_from_slice(slice);
        Self { start, end: eos, eos }
    }
}

impl<T: Clone> CppVector<T> {
    pub fn clone_from_slice(slice: &[T]) -> Self {
        let layout = Layout::from_size_align(slice.len() * std::mem::size_of::<T>(), 1).unwrap();
        let (start, eos) = unsafe {
            let start = std::alloc::alloc(layout) as *mut T;
            (start, start.add(slice.len()))
        };
        let new_slice = unsafe { std::slice::from_raw_parts_mut(start, slice.len()) };
        new_slice.clone_from_slice(slice);
        Self { start, end: eos, eos }
    }
}

impl<T> Index<usize> for CppVector<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T> Index<Range<usize>> for CppVector<T> {
    type Output = [T];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T> IndexMut<usize> for CppVector<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<T> IndexMut<Range<usize>> for CppVector<T> {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<'a, T> IntoIterator for &'a CppVector<T> {
    type IntoIter = CppVectorIterator<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        CppVectorIterator { vector: self, index: 0 }
    }
}

impl<'a, T> IntoIterator for &'a mut CppVector<T> {
    type IntoIter = CppVectorIteratorMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self) -> Self::IntoIter {
        CppVectorIteratorMut { vector: self, index: 0 }
    }
}

pub struct CppVectorIterator<'a, T> {
    vector: &'a CppVector<T>,
    index: isize,
}

impl<'a, T> Iterator for CppVectorIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        unsafe {
            if self.vector.start.offset(self.index) != self.vector.end {
                self.index += 1;
                Some(&* self.vector.start.offset(self.index - 1))
            } else {
                None
            }
        }
    }
}

pub struct CppVectorIteratorMut<'a, T> {
    vector: &'a mut CppVector<T>,
    index: isize,
}

impl<'a, T> Iterator for CppVectorIteratorMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        unsafe {
            if self.vector.start.offset(self.index) != self.vector.end {
                self.index += 1;
                Some(&mut *self.vector.start.offset(self.index - 1))
            } else {
                None
            }
        }
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum LoadType {
    Directory = 0x0,
    File = 0x1,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LoadInfo {
    pub ty: LoadType,
    pub filepath_index: u32,
    pub directory_index: u32,
    pub files_to_load: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ListNode {
    pub next: *mut ListNode,
    pub prev: *mut ListNode,
    pub data: LoadInfo,
}

impl<'a> IntoIterator for &'a ResList {
    type IntoIter = ResListIter<'a>;
    type Item = &'a LoadInfo;

    fn into_iter(self) -> Self::IntoIter {
        ResListIter { list: self, count: 0 }
    }
}

impl<'a> IntoIterator for &'a mut ResList {
    type IntoIter = ResListIterMut<'a>;
    type Item = &'a mut LoadInfo;

    fn into_iter(self) -> Self::IntoIter {
        ResListIterMut { list: self, count: 0 }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ResList {
    pub size: usize,
    pub next: *mut ListNode,
    pub end: *mut ListNode,
}

impl ResList {
    pub fn get_node(&self, idx: usize) -> Option<&ListNode> {
        if idx >= self.size {
            None
        } else {
            let mut node = self.next;
            for _ in 0..idx {
                node = unsafe { (*node).next };
            }
            unsafe { Some(&*node) }
        }
    }

    pub fn node_iter(&self) -> NodeIter {
        NodeIter { list: self, count: 0 }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn insert(&mut self, value: LoadInfo) {
        unsafe {
            let node = skyline::libc::malloc(std::mem::size_of::<ListNode>()) as *mut ListNode;
            (*node).prev = &mut self.next as *mut *mut ListNode as *mut ListNode;
            (*node).next = self.next;
            self.next = node;
            (*node).data = value;
            self.size += 1;
        }
    }

    pub fn get(&self, idx: usize) -> Option<&LoadInfo> {
        if idx >= self.size {
            None
        } else {
            let mut node = self.next;
            for _ in 0..idx {
                node = unsafe { (*node).next };
            }
            unsafe { Some(&(*node).data) }
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut LoadInfo> {
        if idx >= self.size {
            None
        } else {
            let mut node = self.next;
            for _ in 0..idx {
                node = unsafe { (*node).next };
            }
            unsafe { Some(&mut (*node).data) }
        }
    }

    pub fn iter(&self) -> ResListIter<'_> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> ResListIterMut<'_> {
        self.into_iter()
    }
}

pub struct NodeIter<'a> {
    list: &'a ResList,
    count: usize,
}

pub struct ResListIter<'a> {
    list: &'a ResList,
    count: usize,
}

pub struct ResListIterMut<'a> {
    list: &'a mut ResList,
    count: usize,
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a ListNode;

    fn next(&mut self) -> Option<&'a ListNode> {
        self.count += 1;
        self.list.get_node(self.count - 1)
    }
}

impl<'a> Iterator for ResListIter<'a> {
    type Item = &'a LoadInfo;

    fn next(&mut self) -> Option<&'a LoadInfo> {
        self.count += 1;
        self.list.get(self.count - 1)
    }
}

impl<'a> Iterator for ResListIterMut<'a> {
    type Item = &'a mut LoadInfo;

    fn next(&mut self) -> Option<&'a mut LoadInfo> {
        unsafe {
            self.count += 1;
            std::mem::transmute(self.list.get_mut(self.count - 1))
        }
    }
}
