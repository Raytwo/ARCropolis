use std::{
    convert::TryInto,
    num::NonZeroU16,
    path::{Component, Path},
};

#[derive(Default)]
pub struct Interner {
    strings: Vec<String>,
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct StrId(NonZeroU16);

#[repr(transparent)]
pub struct InternedPath<const MAX_COMPONENTS: usize>([Option<StrId>; MAX_COMPONENTS]);

impl Interner {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get(&self, id: StrId) -> &str {
        self.strings.get(id.0.get().saturating_sub(1) as usize).unwrap()
    }

    pub fn add(&mut self, string: String) -> StrId {
        let idx = if let Some(existing_string) = self.strings.iter().position(|s| *s == string) {
            existing_string + 1
        } else {
            self.strings.push(string);

            self.strings.len()
        };

        StrId(NonZeroU16::new(idx.try_into().unwrap()).unwrap())
    }

    pub fn add_path<const N: usize>(&mut self, path: &Path) -> InternedPath<N> {
        let component_count = path.components().count();
        if component_count > N {
            panic!("Path has {} components, only a max of {} are allowed.", component_count, N);
        }

        let components = path.components().filter_map(|component| {
            if let Component::Normal(component) = component {
                Some(component.to_string_lossy().into_owned())
            } else {
                None
            }
        });

        let mut path = InternedPath([None; N]);

        for (i, component) in components.enumerate() {
            path.0[i].replace(self.add(component));
        }

        path
    }
}

impl<const N: usize> InternedPath<N> {
    pub fn to_string(&self, interner: &Interner) -> String {
        let slashes = self.components(interner).count().saturating_sub(1);
        let length = self.components(interner).map(|c| c.len()).sum::<usize>() + slashes;

        let mut string = String::with_capacity(length);
        let mut comps = self.components(interner);

        if let Some(first_comp) = comps.next() {
            string.push_str(first_comp);
        }

        for component in comps {
            string.push('/');
            string.push_str(component);
        }

        string
    }

    pub fn components<'a>(&'a self, interner: &'a Interner) -> impl Iterator<Item = &'a str> + 'a {
        self.0.iter().filter_map(move |c| c.map(|comp| interner.get(comp)))
    }
}
