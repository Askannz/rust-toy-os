use core::hash::Hash;
use super::hash::compute_hash;

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u64);

impl ContentId {
    pub fn from_hash<T: Hash>(val: T) -> Self {
        Self(compute_hash(val))
    }
}

#[derive(Debug)]
pub struct TrackedContent<P: UuidProvider, T> {
    inner: T,
    content_id: ContentId,
    _marker: core::marker::PhantomData<P>
}

impl<P: UuidProvider, T> TrackedContent<P, T> {

    pub fn new(inner: T) -> Self {
        Self { inner, content_id: P::make_id(), _marker: core::marker::PhantomData }
    }

    pub fn mutate<'a>(&'a mut self) -> &'a mut T {
        self.content_id = P::make_id();
        &mut self.inner
    }

    pub fn as_ref<'a>(&'a self) -> &'a T {
        &self.inner
    }

    pub fn get_id(&self) -> ContentId {
        self.content_id
    }
}

pub trait UuidProvider {
    fn make_id() -> ContentId;
}
