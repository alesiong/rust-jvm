use std::{
    borrow::{Borrow, Cow},
    fmt::{Debug, Display},
    mem,
    ops::Deref,
    sync::Arc,
};

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

    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
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

    pub fn to_str_arc(self: Arc<Self>) -> Arc<str> {
        let java_str =
            cesu8_java::JavaStr::from_java_cesu8(&self.inner).expect("must be valid utf8 string");
        match cesu8_java::from_java_cesu8(java_str) {
            // SAFETY: Arc<JavaStr> and Arc<str> has the same layout
            Cow::Borrowed(_) => unsafe { mem::transmute::<Arc<JavaStr>, Arc<str>>(self) },
            Cow::Owned(string) => Arc::from(string),
        }
    }

    pub fn from_str(s: &str) -> Cow<'_, JavaStr> {
        match cesu8_java::from_utf8(s) {
            Cow::Borrowed(b) => Cow::Borrowed(unsafe { Self::new(b.as_bytes()) }),

            Cow::Owned(o) => Cow::Owned(JavaString {
                inner: o.into_bytes(),
            }),
        }
    }

    /// (bytes, has_multibyte)
    pub fn to_java_string_bytes(&self, compact: bool) -> (Cow<'_, [u8]>, bool) {
        let (num_chars, mut is_latin1, mut has_multibyte) = self.calculate_unicode_info();
        if !compact {
            has_multibyte = true;
            is_latin1 = false;
        }
        if !has_multibyte {
            return (Cow::Borrowed(&self.inner), false);
        }

        // need allocation
        let mut bytes: Vec<u8> = Vec::with_capacity(num_chars * (if is_latin1 { 1 } else { 2 }));

        if is_latin1 {
            self.convert_to_unicode(bytes.as_mut_ptr(), num_chars);
        } else {
            self.convert_to_unicode(bytes.as_mut_ptr() as *mut u16, num_chars);
        }

        unsafe {
            bytes.set_len(bytes.capacity());
        }

        (Cow::Owned(bytes), has_multibyte)
    }

    pub fn to_java_string_bytes_arc(self: Arc<Self>, compact: bool) -> (Arc<[u8]>, bool) {
        let (cow, has_multibyte) = self.to_java_string_bytes(compact);

        let arc = match cow {
            // SAFETY: JavaStr and [u8] has the same layout
            Cow::Borrowed(_) => unsafe { mem::transmute::<Arc<Self>, Arc<[u8]>>(self) },
            Cow::Owned(string) => Arc::from(string),
        };

        (arc, has_multibyte)
    }

    ///  return: length in unicode chars, is_latin1, has_multibyte
    pub fn calculate_unicode_info(&self) -> (usize, bool, bool) {
        let mut num_chars = 0;
        let mut has_multibyte = false;
        let mut is_latin1 = true;
        let mut prev: u8 = 0;

        for &c in &self.inner {
            if (c & 0xC0) == 0x80 {
                // Multibyte, check if valid latin1 character.
                has_multibyte = true;
                if prev > 0xC3 {
                    is_latin1 = false;
                }
            } else {
                num_chars += 1;
            }
            prev = c;
        }

        (num_chars, is_latin1, has_multibyte)
    }

    fn convert_to_unicode<T>(&self, bytes: *mut T, max_len: usize)
    where
        T: TryFrom<u16> + From<u8> + Copy,
        <T as TryFrom<u16>>::Error: Debug,
    {
        let mut index = 0;
        let mut len = 0;

        // ASCII case loop optimization
        while index < self.inner.len() && len < max_len {
            let ch = self.inner[index];
            if ch > 0x7F {
                break;
            }
            unsafe {
                bytes.add(len).write(ch.into());
            }
            len += 1;
            index += 1;
        }

        // Handle multi-byte sequences
        while index < self.inner.len() && len < max_len {
            let (unicode_char, bytes_consumed) = Self::next_utf8_char(&self.inner, index);
            unsafe {
                bytes
                    .add(len)
                    .write(unicode_char.try_into().expect("must be one-byte char"));
            }
            len += 1;
            index += bytes_consumed;

            // Safety check to prevent infinite loop
            if bytes_consumed == 0 {
                index += 1;
            }
        }

        debug_assert_eq!(len, max_len);
    }

    /// Parse next UTF-8 character from the given position
    /// Returns (character, bytes_consumed)
    fn next_utf8_char(bytes: &[u8], start: usize) -> (u16, usize) {
        if start >= bytes.len() {
            return (0, 0);
        }

        let ptr = &bytes[start..];
        let ch = ptr[0];

        match ch >> 4 {
            // ASCII case (0x0 to 0x7)
            0x0..=0x7 => (ch as u16, 1),

            // Invalid patterns
            0x8 | 0x9 | 0xA | 0xB | 0xF => {
                // Return the byte as-is and advance by 1
                (ch as u16, 1)
            }

            // 2-byte sequence: 110xxxxx 10xxxxxx
            0xC | 0xD => {
                if ptr.len() >= 2 {
                    let ch2 = ptr[1];
                    if (ch2 & 0xC0) == 0x80 {
                        let high_five = (ch & 0x1F) as u16;
                        let low_six = (ch2 & 0x3F) as u16;
                        let result = (high_five << 6) + low_six;
                        return (result, 2);
                    }
                }
                // Invalid sequence, return first byte
                (ch as u16, 1)
            }

            // 3-byte sequence: 1110xxxx 10xxxxxx 10xxxxxx
            0xE => {
                if ptr.len() >= 3 {
                    let ch2 = ptr[1];
                    let ch3 = ptr[2];
                    if (ch2 & 0xC0) == 0x80 && (ch3 & 0xC0) == 0x80 {
                        let high_four = (ch & 0x0F) as u16;
                        let mid_six = (ch2 & 0x3F) as u16;
                        let low_six = (ch3 & 0x3F) as u16;
                        let result = (((high_four << 6) + mid_six) << 6) + low_six;
                        return (result, 3);
                    }
                }
                // Invalid sequence, return first byte
                (ch as u16, 1)
            }

            _ => unreachable!(),
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

impl From<&JavaStr> for Arc<JavaStr> {
    fn from(val: &JavaStr) -> Self {
        let arc = Arc::<[u8]>::from(&val.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const JavaStr) }
    }
}

impl From<JavaString> for Arc<JavaStr> {
    fn from(val: JavaString) -> Self {
        let arc = Arc::<[u8]>::from(val.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const JavaStr) }
    }
}

impl Debug for JavaStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // TODO: resolve surrogate pairs
        Debug::fmt(&String::from_utf8_lossy(&self.inner), f)
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

impl Deref for JavaString {
    type Target = JavaStr;
    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}
