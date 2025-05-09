mod raw;
pub use raw::*;

use alloc::string::String;
use hashbrown::{HashMap, hash_map};

use crate::{Document, DocumentBuffer, ValueRef, access, codec::StringRange};

mod arg;
mod entry;
mod node;
mod value;

pub use arg::*;
pub use entry::*;
pub use node::*;
pub use value::*;

/// Builder for [`Document`](crate::Document)s.
///
/// The builder can be used as a mutable document, i.e. it also supports
/// reading and deserialization.
#[derive(Clone, Debug)]
pub struct Builder<'a> {
    root: Node<'a>,
    auto_intern_limit: usize,
}

impl Default for Builder<'_> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Builder<'a> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            root: Node::empty(),
            auto_intern_limit: 128,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.root = Node::empty();
    }

    /// Create a mutable builder from an immutable document.
    #[must_use]
    pub fn from_document(doc: &'a Document) -> Self {
        let root = Node::from_document(doc.root());
        let mut builder = Self::new();
        builder.root = root;
        builder
    }

    #[must_use]
    pub fn auto_intern_limit(&self) -> usize {
        self.auto_intern_limit
    }

    #[inline]
    pub fn set_auto_intern_limit(&mut self, limit: usize) -> &mut Self {
        self.auto_intern_limit = limit;
        self
    }

    #[inline]
    pub fn set_root(&mut self, node: Node<'a>) {
        self.root = node;
    }

    #[inline]
    #[must_use]
    pub fn root(&self) -> &Node<'a> {
        &self.root
    }

    #[inline]
    pub fn root_mut(&mut self) -> &mut Node<'a> {
        &mut self.root
    }

    pub fn with_root(&mut self, f: impl FnOnce(&mut Node<'a>)) -> &mut Self {
        f(&mut self.root);
        self
    }

    #[must_use]
    pub fn build(&self) -> DocumentBuffer {
        let mut cache = BuildCache::default();
        self.build_with_cache(&mut cache)
    }

    pub fn build_with_cache(&self, cache: &mut BuildCache) -> DocumentBuffer {
        let root = &self.root;
        if root.is_empty() {
            return DocumentBuffer::default();
        }
        cache.set_auto_intern_limit(self.auto_intern_limit);

        // This recursively serializes the document to the binary format.
        cache.raw.set_root(root);

        cache.raw.build()
    }

    #[inline]
    #[must_use]
    pub fn into_static(self) -> Builder<'static> {
        Builder {
            root: self.root.into_static(),
            auto_intern_limit: self.auto_intern_limit,
        }
    }
}

/// Allocation cache for [`Builder`], to amortize allocations between calls to
/// [`build()`](Builder::build).
#[derive(Clone, Default)]
pub struct BuildCache {
    raw: RawBuilder,
}

impl BuildCache {
    #[inline]
    pub fn reset(&mut self) {
        self.raw.clear();
    }

    /// Reset the cache and reclaim any memory that it is currently consuming.
    #[inline]
    pub fn deallocate(&mut self) {
        *self = Self::default();
    }

    fn set_auto_intern_limit(&mut self, limit: usize) {
        self.raw.strings.limit = limit;
    }
}

#[derive(Default, Clone)]
struct Strings {
    buffer: String,
    interned: HashMap<String, StringRange>,
    limit: usize,
}

impl Strings {
    #[inline]
    fn clear(&mut self) {
        self.buffer.clear();
        self.interned.clear();
    }

    #[inline]
    fn add_string(&mut self, s: &str) -> StringRange {
        if s.is_empty() {
            return StringRange::EMPTY;
        }
        if s.len() <= self.limit {
            return self.add_string_intern(s);
        }

        let start = self.buffer.len() as u32;
        let len = s.len() as u32;
        self.buffer.push_str(s);
        StringRange { start, len }
    }

    #[inline]
    fn add_string_intern(&mut self, s: &str) -> StringRange {
        if s.is_empty() {
            return StringRange::EMPTY;
        }

        match self.interned.entry_ref(s) {
            hash_map::EntryRef::Occupied(entry) => *entry.get(),
            hash_map::EntryRef::Vacant(entry) => {
                let start = self.buffer.len() as u32;
                let len = s.len() as u32;
                self.buffer.push_str(s);
                *entry.insert(StringRange { start, len })
            }
        }
    }
}

impl<'a, 'c: 'a> access::NodeRef<'a> for &'a Node<'c> {
    type ChildrenIter<'b>
        = core::slice::Iter<'b, Node<'c>>
    where
        Self: 'b,
        'c: 'b;

    type ArgsIter<'b>
        = core::slice::Iter<'b, Arg<'c>>
    where
        Self: 'b,
        'c: 'b;

    #[inline]
    fn name(&self) -> &'a str {
        &self.name
    }

    #[inline]
    fn ty(&self) -> &'a str {
        &self.ty
    }

    #[inline]
    fn children(&self) -> Self::ChildrenIter<'a> {
        self.children.iter()
    }

    #[inline]
    fn args(&self) -> Self::ArgsIter<'a> {
        self.args.iter()
    }
}

impl<'a> access::ArgRef<'a> for &'a Arg<'_> {
    #[inline]
    fn name(&self) -> &'a str {
        self.name.as_deref().unwrap_or_default()
    }

    #[inline]
    fn value(&self) -> ValueRef<'a> {
        (&self.value).into()
    }
}

impl<'a, 'b> From<&'a Entry<'b>> for access::EntryRef<&'a Arg<'b>, &'a Node<'b>> {
    #[inline]
    fn from(value: &'a Entry<'b>) -> Self {
        match value {
            Entry::Arg(arg) => access::EntryRef::Arg(arg),
            Entry::Child(node) => access::EntryRef::Child(node),
        }
    }
}
