use super::hash::compute_hash;
use core::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u64);

impl ContentId {
    pub fn from_hash<T: Hash>(val: &T) -> Self {
        Self(compute_hash(val))
    }
}

#[derive(Debug, Clone)]
pub struct TrackedContent<T> {
    inner: T,
    content_id: ContentId,
}

impl<T> TrackedContent<T> {
    pub fn new(inner: T, uuid_provider: &mut UuidProvider) -> Self {
        Self {
            inner,
            content_id: uuid_provider.make_id(),
        }
    }

    pub fn new_with_id(inner: T, content_id: ContentId)-> Self {
        Self {
            inner,
            content_id,
        }
    }

    pub fn mutate<'a>(&'a mut self, uuid_provider: &mut UuidProvider) -> &'a mut T {
        self.content_id = uuid_provider.make_id();
        &mut self.inner
    }

    pub fn as_ref<'a>(&'a self) -> &'a T {
        &self.inner
    }

    pub fn get_id(&self) -> ContentId {
        self.content_id
    }

    pub fn to_inner(self) -> (T, ContentId) {
        let Self { inner, content_id } = self;
        (inner, content_id)
    }
}

impl<T: Hash> TrackedContent<T> {

    pub fn new_from_hash(inner: T) -> Self {

        let cid = ContentId::from_hash(&inner);

        Self {
            inner,
            content_id: cid
        }
    }

}

pub struct UuidProvider {
    next: u64,
}

impl UuidProvider {
    fn make_id(&mut self) -> ContentId {
        let content_id = ContentId(self.next);
        if self.next == u64::MAX {
            log::warn!("Reached max content ID, wrapping around")
        }
        self.next += 1;
        content_id
    }
}

impl UuidProvider {
    pub fn new() -> Self {
        Self { next: 0 }
    }
}
