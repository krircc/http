use std::convert::TryFrom;
use std::str::FromStr;
use std::{cmp, fmt, str};

use bytes::Bytes;

use super::{ErrorKind, InvalidUri, InvalidUriBytes};
use crate::byte_str::ByteStr;

/// Represents the path component of a URI
#[derive(Clone)]
pub struct PathAndQuery {
    pub(super) data: ByteStr,
    pub(super) query: u16,
}

const NONE: u16 = ::std::u16::MAX;

impl PathAndQuery {
    /// Attempt to convert a `PathAndQuery` from `Bytes`.
    ///
    /// This function will be replaced by a `TryFrom` implementation once the
    /// trait lands in stable.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate http;
    /// # use http::uri::*;
    /// extern crate bytes;
    ///
    /// use bytes::Bytes;
    ///
    /// # pub fn main() {
    /// let bytes = Bytes::from("/hello?world");
    /// let path_and_query = PathAndQuery::from_shared(bytes).unwrap();
    ///
    /// assert_eq!(path_and_query.path(), "/hello");
    /// assert_eq!(path_and_query.query(), Some("world"));
    /// # }
    /// ```
    pub fn from_shared(mut src: Bytes) -> Result<Self, InvalidUriBytes> {
        let mut query = NONE;
        let mut fragment = None;

        // block for iterator borrow
        //
        // allow: `...` pattersn are now `..=`, but we cannot update yet
        // because of minimum Rust version
        #[allow(warnings)]
        {
            let mut iter = src.as_ref().iter().enumerate();

            // path ...
            for (i, &b) in &mut iter {
                // See https://url.spec.whatwg.org/#path-state
                match b {
                    b'?' => {
                        debug_assert_eq!(query, NONE);
                        query = i as u16;
                        break;
                    }
                    b'#' => {
                        fragment = Some(i);
                        break;
                    }

                    // This is the range of bytes that don't need to be
                    // percent-encoded in the path. If it should have been
                    // percent-encoded, then error.
                    0x21 |
                    0x24..=0x3B |
                    0x3D |
                    0x40..=0x5F |
                    0x61..=0x7A |
                    0x7C |
                    0x7E => {},

                    _ => return Err(ErrorKind::InvalidUriChar.into()),
                }
            }

            // query ...
            if query != NONE {

                // allow: `...` pattersn are now `..=`, but we cannot update yet
                // because of minimum Rust version
                #[allow(warnings)]
                for (i, &b) in iter {
                    match b {
                        // While queries *should* be percent-encoded, most
                        // bytes are actually allowed...
                        // See https://url.spec.whatwg.org/#query-state
                        //
                        // Allowed: 0x21 / 0x24 - 0x3B / 0x3D / 0x3F - 0x7E
                        0x21 |
                        0x24..=0x3B |
                        0x3D |
                        0x3F..=0x7E => {},

                        b'#' => {
                            fragment = Some(i);
                            break;
                        }

                        _ => return Err(ErrorKind::InvalidUriChar.into()),
                    }
                }
            }
        }

        if let Some(i) = fragment {
            src.truncate(i);
        }

        Ok(PathAndQuery {
            data: unsafe { ByteStr::from_utf8_unchecked(src) },
            query: query,
        })
    }

    /// Convert a `PathAndQuery` from a static string.
    ///
    /// This function will not perform any copying, however the string is
    /// checked to ensure that it is valid.
    ///
    /// # Panics
    ///
    /// This function panics if the argument is an invalid path and query.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    /// let v = PathAndQuery::from_static("/hello?world");
    ///
    /// assert_eq!(v.path(), "/hello");
    /// assert_eq!(v.query(), Some("world"));
    /// ```
    #[inline]
    pub fn from_static(src: &'static str) -> Self {
        let src = Bytes::from_static(src.as_bytes());

        PathAndQuery::from_shared(src).unwrap()
    }

    pub(super) fn empty() -> Self {
        PathAndQuery {
            data: ByteStr::new(),
            query: NONE,
        }
    }

    pub(super) fn slash() -> Self {
        PathAndQuery {
            data: ByteStr::from_static("/"),
            query: NONE,
        }
    }

    pub(super) fn star() -> Self {
        PathAndQuery {
            data: ByteStr::from_static("*"),
            query: NONE,
        }
    }

    /// Returns the path component
    ///
    /// The path component is **case sensitive**.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                        |--------|
    ///                                             |
    ///                                           path
    /// ```
    ///
    /// If the URI is `*` then the path component is equal to `*`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    ///
    /// let path_and_query: PathAndQuery = "/hello/world".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.path(), "/hello/world");
    /// ```
    #[inline]
    pub fn path(&self) -> &str {
        let ret = if self.query == NONE {
            &self.data[..]
        } else {
            &self.data[..self.query as usize]
        };

        if ret.is_empty() {
            return "/";
        }

        ret
    }

    /// Returns the query string component
    ///
    /// The query component contains non-hierarchical data that, along with data
    /// in the path component, serves to identify a resource within the scope of
    /// the URI's scheme and naming authority (if any). The query component is
    /// indicated by the first question mark ("?") character and terminated by a
    /// number sign ("#") character or by the end of the URI.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                                   |-------------------|
    ///                                                             |
    ///                                                           query
    /// ```
    ///
    /// # Examples
    ///
    /// With a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQuery = "/hello/world?key=value&foo=bar".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.query(), Some("key=value&foo=bar"));
    /// ```
    ///
    /// Without a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQuery = "/hello/world".parse().unwrap();
    ///
    /// assert!(path_and_query.query().is_none());
    /// ```
    #[inline]
    pub fn query(&self) -> Option<&str> {
        if self.query == NONE {
            None
        } else {
            let i = self.query + 1;
            Some(&self.data[i as usize..])
        }
    }

    /// Returns the path and query as a string component.
    ///
    /// # Examples
    ///
    /// With a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQuery = "/hello/world?key=value&foo=bar".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.as_str(), "/hello/world?key=value&foo=bar");
    /// ```
    ///
    /// Without a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQuery = "/hello/world".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.as_str(), "/hello/world");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        let ret = &self.data[..];
        if ret.is_empty() {
            return "/";
        }
        ret
    }

    /// Converts this `PathAndQuery` back to a sequence of bytes
    #[inline]
    pub fn into_bytes(self) -> Bytes {
        self.into()
    }
}

impl TryFrom<Bytes> for PathAndQuery {
    type Error = InvalidUriBytes;
    #[inline]
    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        PathAndQuery::from_shared(bytes)
    }
}

impl<'a> TryFrom<&'a [u8]> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a [u8]) -> Result<Self, Self::Error> {
        PathAndQuery::from_shared(Bytes::copy_from_slice(s)).map_err(|e| e.0)
    }
}

impl<'a> TryFrom<&'a str> for PathAndQuery {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}

impl FromStr for PathAndQuery {
    type Err = InvalidUri;
    #[inline]
    fn from_str(s: &str) -> Result<Self, InvalidUri> {
        TryFrom::try_from(s)
    }
}

impl From<PathAndQuery> for Bytes {
    fn from(src: PathAndQuery) -> Bytes {
        src.data.into()
    }
}

impl fmt::Debug for PathAndQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for PathAndQuery {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.data.is_empty() {
            match self.data.as_bytes()[0] {
                b'/' | b'*' => write!(fmt, "{}", &self.data[..]),
                _ => write!(fmt, "/{}", &self.data[..]),
            }
        } else {
            write!(fmt, "/")
        }
    }
}

// ===== PartialEq / PartialOrd =====

impl PartialEq for PathAndQuery {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self.data == other.data
    }
}

impl Eq for PathAndQuery {}

impl PartialEq<str> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<'a> PartialEq<PathAndQuery> for &'a str {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self == &other.as_str()
    }
}

impl<'a> PartialEq<&'a str> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<PathAndQuery> for str {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<String> for PathAndQuery {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<PathAndQuery> for String {
    #[inline]
    fn eq(&self, other: &PathAndQuery) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialOrd for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<str> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}

impl PartialOrd<PathAndQuery> for str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.partial_cmp(other.as_str())
    }
}

impl<'a> PartialOrd<&'a str> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &&'a str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(*other)
    }
}

impl<'a> PartialOrd<PathAndQuery> for &'a str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.as_str())
    }
}

impl PartialOrd<String> for PathAndQuery {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<PathAndQuery> for String {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQuery) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_to_self_of_same_path() {
        let p1: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        let p2: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(p1, p2);
        assert_eq!(p2, p1);
    }

    #[test]
    fn not_equal_to_self_of_different_path() {
        let p1: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        let p2: PathAndQuery = "/world&foo=bar".parse().unwrap();
        assert_ne!(p1, p2);
        assert_ne!(p2, p1);
    }

    #[test]
    fn equates_with_a_str() {
        let path_and_query: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(&path_and_query, "/hello/world&foo=bar");
        assert_eq!("/hello/world&foo=bar", &path_and_query);
        assert_eq!(path_and_query, "/hello/world&foo=bar");
        assert_eq!("/hello/world&foo=bar", path_and_query);
    }

    #[test]
    fn not_equal_with_a_str_of_a_different_path() {
        let path_and_query: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        // as a reference
        assert_ne!(&path_and_query, "/hello&foo=bar");
        assert_ne!("/hello&foo=bar", &path_and_query);
        // without reference
        assert_ne!(path_and_query, "/hello&foo=bar");
        assert_ne!("/hello&foo=bar", path_and_query);
    }

    #[test]
    fn equates_with_a_string() {
        let path_and_query: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(path_and_query, "/hello/world&foo=bar".to_string());
        assert_eq!("/hello/world&foo=bar".to_string(), path_and_query);
    }

    #[test]
    fn not_equal_with_a_string_of_a_different_path() {
        let path_and_query: PathAndQuery = "/hello/world&foo=bar".parse().unwrap();
        assert_ne!(path_and_query, "/hello&foo=bar".to_string());
        assert_ne!("/hello&foo=bar".to_string(), path_and_query);
    }

    #[test]
    fn compares_to_self() {
        let p1: PathAndQuery = "/a/world&foo=bar".parse().unwrap();
        let p2: PathAndQuery = "/b/world&foo=bar".parse().unwrap();
        assert!(p1 < p2);
        assert!(p2 > p1);
    }

    #[test]
    fn compares_with_a_str() {
        let path_and_query: PathAndQuery = "/b/world&foo=bar".parse().unwrap();
        // by ref
        assert!(&path_and_query < "/c/world&foo=bar");
        assert!("/c/world&foo=bar" > &path_and_query);
        assert!(&path_and_query > "/a/world&foo=bar");
        assert!("/a/world&foo=bar" < &path_and_query);

        // by val
        assert!(path_and_query < "/c/world&foo=bar");
        assert!("/c/world&foo=bar" > path_and_query);
        assert!(path_and_query > "/a/world&foo=bar");
        assert!("/a/world&foo=bar" < path_and_query);
    }

    #[test]
    fn compares_with_a_string() {
        let path_and_query: PathAndQuery = "/b/world&foo=bar".parse().unwrap();
        assert!(path_and_query < "/c/world&foo=bar".to_string());
        assert!("/c/world&foo=bar".to_string() > path_and_query);
        assert!(path_and_query > "/a/world&foo=bar".to_string());
        assert!("/a/world&foo=bar".to_string() < path_and_query);
    }

    #[test]
    fn ignores_valid_percent_encodings() {
        assert_eq!("/a%20b", pq("/a%20b?r=1").path());
        assert_eq!("qr=%31", pq("/a/b?qr=%31").query().unwrap());
    }

    #[test]
    fn ignores_invalid_percent_encodings() {
        assert_eq!("/a%%b", pq("/a%%b?r=1").path());
        assert_eq!("/aaa%", pq("/aaa%").path());
        assert_eq!("/aaa%", pq("/aaa%?r=1").path());
        assert_eq!("/aa%2", pq("/aa%2").path());
        assert_eq!("/aa%2", pq("/aa%2?r=1").path());
        assert_eq!("qr=%3", pq("/a/b?qr=%3").query().unwrap());
    }

    fn pq(s: &str) -> PathAndQuery {
        s.parse().expect(&format!("parsing {}", s))
    }
}
