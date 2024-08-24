use core::hash::{Hash, Hasher};
use md5::{Md5, Digest};

pub fn compute_hash<T: Hash>(val: T) -> u64 {
    let mut hasher = HasherWrapper(Md5::new());
    val.hash(&mut hasher);
    hasher.finish()
}

struct HasherWrapper(Md5);

impl Hasher for HasherWrapper {
    fn finish(&self) -> u64 {
        let Self(hasher) = self;
        let hashed = hasher.clone().finalize();
        u64::from_le_bytes(hashed[..8].try_into().unwrap())
    }
    fn write(&mut self, bytes: &[u8]) {
        let Self(hasher) = self;
        hasher.update(bytes);
    }
}
