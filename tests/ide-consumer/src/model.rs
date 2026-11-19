//! Schema aliases across modules and a renamed Blubber dependency.

use crate::aliases::{Labels, Rows, Text};

/// A variable record declared against a cross-module alias.
#[wire::schema]
pub struct Record {
    /// Borrowed UTF-8 name.
    pub name: Text,
    /// Fixed numeric tags.
    pub tags: wire::List<u32>,
}

/// Account whose fields resolve through aliases in another module.
#[wire::schema(account)]
pub struct Account {
    /// Variable records.
    pub rows: Rows,
    /// Sorted variable map values.
    pub labels: Labels,
    /// UTF-8 title.
    pub title: Text,
}

impl<'a> RecordRef<'a> {
    /// Application helper preserving the original region lifetime.
    pub fn display_name(self) -> &'a str {
        self.name()
    }
}

impl RecordMut<'_> {
    /// Application helper editing sibling fields through one retained handle.
    pub fn annotate(&mut self, name: &str) -> wire::Result<()> {
        self.name_mut().set(name)?;
        self.tags_mut().push(7)
    }
}

impl AccountMut<'_> {
    /// Application helper using inherent generated methods with no trait import.
    pub fn append_named(&mut self, name: &str) -> wire::Result<()> {
        self.rows_mut().push(RecordInit { name, tags: &[] })
    }
}
