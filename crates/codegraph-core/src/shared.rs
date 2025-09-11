use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::borrow::Cow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, Index, Range, RangeFrom, RangeFull, RangeTo};
use std::sync::Arc;

/// SharedStr is an immutable, cheap-to-clone string that can be either
/// - a zero-copy slice of shared bytes (Arc<[u8]> or Bytes), or
/// - an owned boxed str
///
/// It derefs to `str` and implements serde as a string, so it is a mostly
/// drop-in replacement for `String` in read-heavy paths.
#[derive(Clone, Default)]
pub struct SharedStr {
    inner: SharedStrInner,
}

impl PartialEq for SharedStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SharedStr {}

impl Hash for SharedStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Borrow<str> for SharedStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone)]
enum SharedStrInner {
    Bytes(Bytes),
    Slice {
        data: Arc<[u8]>,
        start: usize,
        len: usize,
    },
    Owned(Box<str>),
}

impl Default for SharedStrInner {
    fn default() -> Self {
        SharedStrInner::Owned(Box::<str>::from(""))
    }
}

impl fmt::Debug for SharedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedStr({:?})", &self.as_str())
    }
}

impl fmt::Display for SharedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl SharedStr {
    pub fn as_str(&self) -> &str {
        match &self.inner {
            SharedStrInner::Bytes(b) => std::str::from_utf8(b).unwrap_or(""),
            SharedStrInner::Slice { data, start, len } => {
                let bytes = &data[*start..(*start + *len)];
                std::str::from_utf8(bytes).unwrap_or("")
            }
            SharedStrInner::Owned(s) => s.as_ref(),
        }
    }

    pub fn from_bytes(bytes: Bytes) -> Self {
        SharedStr {
            inner: SharedStrInner::Bytes(bytes),
        }
    }

    pub fn from_arc_slice(data: Arc<[u8]>, start: usize, end: usize) -> Self {
        SharedStr {
            inner: SharedStrInner::Slice {
                data,
                start,
                len: end.saturating_sub(start),
            },
        }
    }

    pub fn into_string(self) -> String {
        self.as_str().to_string()
    }
}

impl From<String> for SharedStr {
    fn from(s: String) -> Self {
        SharedStr {
            inner: SharedStrInner::Owned(s.into_boxed_str()),
        }
    }
}

impl From<&str> for SharedStr {
    fn from(s: &str) -> Self {
        SharedStr {
            inner: SharedStrInner::Owned(s.to_owned().into_boxed_str()),
        }
    }
}

impl From<Cow<'_, str>> for SharedStr {
    fn from(c: Cow<'_, str>) -> Self {
        match c {
            Cow::Borrowed(s) => SharedStr::from(s),
            Cow::Owned(s) => SharedStr::from(s),
        }
    }
}

impl SharedStr {
    /// Convert to a `Cow<str>`. Always borrowed as SharedStr stores immutable data.
    pub fn to_cow(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}

impl Deref for SharedStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Index<Range<usize>> for SharedStr {
    type Output = str;
    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.as_str()[index]
    }
}

impl Index<RangeTo<usize>> for SharedStr {
    type Output = str;
    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        &self.as_str()[index]
    }
}

impl Index<RangeFrom<usize>> for SharedStr {
    type Output = str;
    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        &self.as_str()[index]
    }
}

impl Index<RangeFull> for SharedStr {
    type Output = str;
    fn index(&self, _index: RangeFull) -> &Self::Output {
        self.as_str()
    }
}

impl Serialize for SharedStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SharedStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SharedStr::from(s))
    }
}
