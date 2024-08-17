#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentId(u64);

impl ContentId {

    pub fn new() -> Self {
        ContentId(0)
    }
    
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

#[derive(Debug)]
pub struct TrackedContent<T> {
    inner: T,
    content_id: ContentId,
}

impl<T> TrackedContent<T> {

    pub fn new(inner: T) -> Self {
        Self { inner, content_id: ContentId::new() }
    }

    pub fn mutate<'a>(&'a mut self) -> &'a mut T {
        self.content_id.increment();
        &mut self.inner
    }

    pub fn as_ref<'a>(&'a self) -> &'a T {
        &self.inner
    }

    pub fn get_id(&self) -> ContentId {
        self.content_id
    }
}
