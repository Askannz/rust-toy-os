use super::hash::compute_hash;
use core::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u64);

impl ContentId {
    pub fn from_hash<T: Hash>(val: T) -> Self {
        Self(compute_hash(val))
    }
}

#[derive(Debug)]
pub struct TrackedContent<T> {
    inner: T,
    content_id: ContentId,
}

impl<T> TrackedContent<T> {
    pub fn new<P: UuidProvider>(inner: T, uuid_provider: &mut P) -> Self {
        Self {
            inner,
            content_id: uuid_provider.make_id(),
        }
    }

    pub fn mutate<'a, P: UuidProvider>(&'a mut self, uuid_provider: &mut P) -> &'a mut T {
        self.content_id = uuid_provider.make_id();
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
    fn make_id(&mut self) -> ContentId;
}

pub struct IncrementalUuidProvider {
    next: u64,
}

impl UuidProvider for IncrementalUuidProvider {
    fn make_id(&mut self) -> ContentId {
        let content_id = ContentId(self.next);
        self.next += 1;
        if self.next == u64::MAX {
            log::warn!("Reached max content ID, wrapping around")
        }
        content_id
    }
}

impl IncrementalUuidProvider {
    pub fn new() -> Self {
        Self { next: 0 }
    }
}
