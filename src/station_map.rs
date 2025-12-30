use std::{
    borrow::Borrow,
    hash::{BuildHasher, Hasher},
};

use crate::{
    mmap_allocator::{AllocatorOptions, MmapAllocator},
    memops::memeq64_unchecked,
};

/// A wrapper type that provides comparisons optimized
/// for strings that are <64 bytes.
#[repr(transparent)]
pub struct StationNameKeyView {
    name: str,
}

impl StationNameKeyView {
    pub fn new(s: &str) -> &Self {
        // Hack to allow comparing &str against StationNameKey
        // using a custom comparator in HashMap lookups
        // without having to allocate a StationNameKey.
        unsafe { &*(s as *const str as *const StationNameKeyView) }
    }

    pub fn hash_u64(&self) -> u64 {
        hash64(self.name.as_bytes())
    }
}

// Taken from FxHash implementation.
const SEED: u64 = 0xf1357aea2e62a9c5;

#[cfg_attr(feature = "profiled", inline(never))]
pub fn hash64(bytes: &[u8]) -> u64 {
    unsafe {
        let len = bytes.len();
        let p = bytes.as_ptr();

        // Just pick out four bytes more or less at random.
        // This is somehow about as slow as FxHash.
        // Might be better to read 8 bytes instead
        // to be a little more robust.
        let b0 = *p as u64;
        let b1 = *p.add(len / 4) as u64;
        let b2 = *p.add(len / 2) as u64;
        let b3 = *p.add(len - 1) as u64;

        let x: u64 = (b0 << 56) | (b1 << 48) | (b2 << 40) | (b3 << 32);
        let mut hash = x ^ (len as u64);
        hash = hash.wrapping_mul(SEED);
        hash ^= hash >> 32;
        hash = hash.wrapping_mul(SEED);
        hash ^= hash >> 32;
        hash
    }
}

impl Borrow<StationNameKeyView> for StationNameKey {
    fn borrow(&self) -> &StationNameKeyView {
        StationNameKeyView::new(self.name.as_str())
    }
}

impl PartialEq for StationNameKeyView {
    fn eq(&self, other: &Self) -> bool {
        unsafe { memeq64_unchecked(self.name.as_bytes(), other.name.as_bytes()) }
    }
}

impl Eq for StationNameKeyView {}

impl std::hash::Hash for StationNameKeyView {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash_u64());
    }
}

const INLINE_STRING_SIZE: usize = 56;

#[repr(packed)]
struct InlineString {
    data: [u8; INLINE_STRING_SIZE],
    len: usize,
}

impl InlineString {
    fn new(s: &str) -> Self {
        let mut data: [u8; INLINE_STRING_SIZE] = [0; _];
        (unsafe { data.get_unchecked_mut(..s.as_bytes().len()) }).copy_from_slice(s.as_bytes());
        InlineString { data, len: s.len() }
    }

    fn as_str(&self) -> &str {
        unsafe {
            let s = self.data.get_unchecked(..self.len);
            std::str::from_utf8_unchecked(s)
        }
    }
}

pub struct StationNameKey {
    name: InlineString,
}

impl StationNameKey {
    pub fn new(name: &str) -> Self {
        StationNameKey {
            name: InlineString::new(name),
        }
    }

    pub fn view(&self) -> &StationNameKeyView {
        self.borrow()
    }
}

impl PartialEq for StationNameKey {
    fn eq(&self, other: &Self) -> bool {
        let view: &StationNameKeyView = self.borrow();
        view == other.borrow()
    }
}

impl Eq for StationNameKey {}

impl std::hash::Hash for StationNameKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let view: &StationNameKeyView = self.borrow();
        view.hash(state);
    }
}

impl Into<String> for StationNameKey {
    fn into(self) -> String {
        self.name.as_str().to_owned()
    }
}

#[derive(Default)]
pub struct NopHasher(u64);

impl Hasher for NopHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write_u64(&mut self, i: u64) {
        debug_assert!(self.0 == 0);
        self.0 = i;
    }

    fn write(&mut self, _: &[u8]) {
        panic!("Generic write unsupported for NopHasher");
    }
}

#[derive(Default)]
pub struct NopHasherBuilder {}

impl BuildHasher for NopHasherBuilder {
    type Hasher = NopHasher;
    fn build_hasher(&self) -> Self::Hasher {
        NopHasher::default()
    }
}

pub type StationMap<V> = hashbrown::HashMap<StationNameKey, V, NopHasherBuilder, MmapAllocator>;

pub struct StationMapOptions {
    pub request_hugepage: bool,
    pub capacity: usize,
}

pub fn new_station_map<V>(opts: &StationMapOptions) -> StationMap<V> {
    StationMap::<V>::with_capacity_and_hasher_in(
        opts.capacity,
        NopHasherBuilder::default(),
        MmapAllocator::new(&AllocatorOptions {
            request_hugepage: opts.request_hugepage,
        }),
    )
}
