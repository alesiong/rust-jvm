use std::borrow::{Borrow, Cow};
use std::fmt::{Debug, Display};
use std::mem;
use std::sync::Arc;

use cesu8_str::java as cesu8_java;

#[derive(Hash, Eq, PartialEq)]
#[repr(transparent)]
pub(crate) struct JavaStr {
    inner: [u8],
}

impl JavaStr {
    /// SAFETY: must from JVM class file
    pub(crate) unsafe fn new(inner: &[u8]) -> &Self {
        unsafe { &*(inner as *const [u8] as *const JavaStr) }
    }

    pub fn to_java_string(&self) -> JavaString {
        JavaString {
            inner: self.inner.to_owned(),
        }
    }

    pub fn to_str(&self) -> Cow<'_, str> {
        let java_str =
            cesu8_java::JavaStr::from_java_cesu8(&self.inner).expect("must be valid utf8 string");
        cesu8_java::from_java_cesu8(java_str)
    }

    pub fn from_str(s: &str) -> Cow<'_, JavaStr> {
        match cesu8_java::from_utf8(s) {
            Cow::Borrowed(b) => Cow::Borrowed(unsafe { Self::new(b.as_bytes()) }),

            Cow::Owned(o) => Cow::Owned(JavaString {
                inner: o.into_bytes(),
            }),
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq)]
#[repr(transparent)]
pub(crate) struct JavaString {
    inner: Vec<u8>,
}

impl JavaString {
    /// SAFETY: must from JVM class file
    pub(crate) unsafe fn new(inner: Vec<u8>) -> Self {
        Self { inner }
    }
}

impl Into<Arc<JavaStr>> for &JavaStr {
    fn into(self) -> Arc<JavaStr> {
        let arc = Arc::<[u8]>::from(&self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const JavaStr) }
    }
}

impl Into<Arc<JavaStr>> for JavaString {
    fn into(self) -> Arc<JavaStr> {
        let arc = Arc::<[u8]>::from(self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const JavaStr) }
    }
}

impl Debug for JavaStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // TODO: resolve surrogate pairs
        Debug::fmt(&String::from_utf8_lossy(&self.inner), f)
    }
}

impl Display for JavaStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&String::from_utf8_lossy(&self.inner), f)
    }
}

impl ToOwned for JavaStr {
    type Owned = JavaString;

    fn to_owned(&self) -> Self::Owned {
        self.to_java_string()
    }
}

impl Borrow<JavaStr> for JavaString {
    fn borrow(&self) -> &JavaStr {
        unsafe { JavaStr::new(&self.inner) }
    }
}
