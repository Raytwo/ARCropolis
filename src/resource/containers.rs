#[repr(C)]
pub struct CppVector<T> {
    start: *mut T,
    end: *mut T,
    eos: *mut T,
}

impl<T> CppVector<T> {
    pub fn iter(&self) -> CppVectorIterator<T> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> CppVectorIteratorMut<T> {
        self.into_iter()
    }

    pub fn len(&self) -> usize {
        ((self.end as usize) - (self.start as usize)) / std::mem::size_of::<T>()
    }
}

impl<'a, T> IntoIterator for &'a CppVector<T> {
    type Item = &'a T;
    type IntoIter = CppVectorIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        CppVectorIterator {
            vector: self,
            index: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a mut CppVector<T> {
    type Item = &'a mut T;
    type IntoIter = CppVectorIteratorMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        CppVectorIteratorMut {
            vector: self,
            index: 0,
        }
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
                Some(std::mem::transmute::<*mut T, &'a T>(
                    self.vector.start.offset(self.index - 1),
                ))
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
                Some(std::mem::transmute::<*mut T, &'a mut T>(
                    self.vector.start.offset(self.index - 1),
                ))
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
    type Item = &'a LoadInfo;
    type IntoIter = ResListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        ResListIter {
            list: self,
            count: 0,
        }
    }
}

impl<'a> IntoIterator for &'a mut ResList {
    type Item = &'a mut LoadInfo;
    type IntoIter = ResListIterMut<'a>;
    fn into_iter(self) -> Self::IntoIter {
        ResListIterMut {
            list: self,
            count: 0,
        }
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
        NodeIter {
            list: self,
            count: 0,
        }
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
