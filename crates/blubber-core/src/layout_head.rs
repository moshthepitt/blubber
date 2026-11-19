//! Code-constructed descriptor heads. Integer tables remain the deployed data.
// Verus generates specification-only enum accessors without documentation.
#![cfg_attr(verus_only, allow(missing_docs))]
use crate::{CheckedLayout, FixedField, KeyDomain, KeyLayout, Layout, Leaf, VariableField};
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, Debug)]
/// Immediate node metadata decoded from native descriptors or certified words.
pub enum LayoutHead<'a> {
    /// Fixed scalar or raw byte leaf.
    Leaf(Leaf),
    /// Variable-length UTF-8 bytes.
    Str,
    /// Packed fixed prefix followed by variable fields.
    Struct {
        /// Packed prefix width.
        fixed_bytes: u32,
        /// Fixed fields in wire order.
        fixed: Fields<'a>,
        /// Variable fields in wire order.
        variable: Fields<'a>,
    },
    /// Tagged optional child.
    Option {
        /// Present value descriptor.
        value: Layout,
    },
    /// Count-prefixed sequence.
    List {
        /// Element descriptor.
        element: Layout,
    },
    /// Sorted key/value collection.
    Map {
        /// Fixed key encoding.
        key: KeyLayout,
        /// Value descriptor.
        value: Layout,
    },
}
#[derive(Clone, Copy, Debug)]
/// Borrowed field metadata, without allocating a decoded descriptor array.
pub enum Fields<'a> {
    /// Native fixed field descriptors.
    Fixed(&'static [FixedField]),
    /// Native variable field descriptors.
    Variable(&'static [VariableField]),
    /// Fields inherited from a certified integer table.
    Flat {
        /// Parent witness.
        layout: &'a CheckedLayout<'static>,
        /// Selects the variable field span when true.
        variable: bool,
    },
}
#[derive(Clone, Copy, Debug)]
/// One field decoded from its parent descriptor.
pub struct Field {
    /// Declaration ordinal, independent of wire order.
    pub declaration: u16,
    /// Packed fixed offset; zero for a variable field.
    pub offset: u32,
    /// Inherited child descriptor.
    pub layout: Layout,
}
impl Fields<'_> {
    /// Mathematical field sequence.
    pub open spec fn view(self) -> Seq<Field> {
        match self {
            Self::Fixed(fields) => Seq::new(fields.len() as nat, |i:int| Field {
                declaration: fields[i].declaration, offset: fields[i].offset,
                layout: *fields[i].layout,
            }),
            Self::Variable(fields) => Seq::new(fields.len() as nat, |i:int| Field {
                declaration: fields[i].declaration, offset: 0, layout: *fields[i].layout,
            }),
            Self::Flat { layout, variable } => {
                let w = layout.view(); let r = layout.root_index();
                let n = w[r + if variable { 4int } else { 3int }] as nat;
                let first = w[r+5] + if variable { 8*w[r+3] } else { 0int };
                Seq::new(n, |i:int| Field { declaration: w[first+8*i+1] as u16,
                    offset: w[first+8*i+2], layout: Layout::Flat {
                        checked: layout.retarget(w[first+8*i+3] as int),
                    },
                })
            },
        }
    }
    /// Mathematical number of fields.
    pub open spec fn len_spec(self) -> usize { self.view().len() as usize }
    #[verifier::when_used_as_spec(len_spec)]
    #[inline(always)]
    /// Number of fields in this span.
    pub fn len(self) -> (result: usize)
        ensures result == self.view().len(),
    {
        match self {
            Self::Fixed(fields) => fields.len(),
            Self::Variable(fields) => fields.len(),
            Self::Flat { layout, variable } => layout.header(if variable { 4 } else { 3 }) as usize,
        }
    }
    /// Whether the mathematical span is empty.
    pub open spec fn is_empty_spec(self) -> bool { self.view().len() == 0 }
    #[verifier::when_used_as_spec(is_empty_spec)]
    #[inline(always)]
    /// Whether this span contains no fields.
    pub fn is_empty(self) -> (result: bool)
        ensures result == (self.view().len() == 0),
    { self.len() == 0 }

    #[verifier::when_used_as_spec(get_spec)]
    #[inline(always)]
    /// Decodes a field; panics when the index is outside this span.
    pub fn get(self, index: usize) -> (result: Field)
        requires index < self.view().len(),
        ensures result == self.view()[index as int],
    {
        match self {
            Self::Fixed(fields) => Field { declaration: fields[index].declaration,
                offset: fields[index].offset, layout: *fields[index].layout },
            Self::Variable(fields) => Field { declaration: fields[index].declaration,
                offset: 0, layout: *fields[index].layout },
            Self::Flat { layout, variable } => {
                let (declaration, offset, child) = layout.field(index, variable);
                Field { declaration, offset, layout: Layout::Flat { checked: child } }
            },
        }
    }
    /// Reads a declaration ordinal without decoding unused child metadata.
    #[inline(always)]
    pub fn declaration(self,index:usize) -> (result:u16)
        requires index < self.view().len(),
        ensures result == self.view()[index as int].declaration,
    {
        match self {
            Self::Fixed(fields) => fields[index].declaration,
            Self::Variable(fields) => fields[index].declaration,
            Self::Flat {layout,variable} => layout.field_word(index,variable,1) as u16,
        }
    }
    /// Mathematical field at an admitted index.
    pub open spec fn get_spec(self, index: usize) -> Field { self.view()[index as int] }
}

#[verifier::opaque]
/// Mathematical immediate descriptor node.
pub open spec fn head_spec<'a>(layout: &'a Layout) -> LayoutHead<'a> {
    match layout {
        Layout::Leaf(leaf) => LayoutHead::Leaf(*leaf),
        Layout::Str => LayoutHead::Str,
        Layout::Struct { fixed_bytes, fixed, variable } => LayoutHead::Struct {
            fixed_bytes: *fixed_bytes, fixed: Fields::Fixed(fixed), variable: Fields::Variable(variable),
        },
        Layout::Option { value } => LayoutHead::Option { value: **value },
        Layout::List { element } => LayoutHead::List { element: **element },
        Layout::Map { key, value } => LayoutHead::Map { key: *key, value: **value },
        Layout::Flat { checked } => {
            let w = checked.view(); let r = checked.root_index();
            let child = Layout::Flat { checked: checked.retarget(w[w[r+5] as int+3] as int) };
            match w[r] {
                1 => LayoutHead::Leaf(Leaf::Unsigned(w[r+1] as u8)),
                2 => LayoutHead::Leaf(Leaf::Signed(w[r+1] as u8)),
                3 => LayoutHead::Leaf(Leaf::Bool),
                4 => LayoutHead::Leaf(Leaf::Raw(w[r+1])),
                5 => LayoutHead::Str,
                6 => LayoutHead::Option { value: child },
                7 => LayoutHead::List { element: child },
                8 => LayoutHead::Map { key: KeyLayout { width: w[r+6], domain: match w[r+7] {
                    1 => KeyDomain::Unsigned, 2 => KeyDomain::Signed,
                    3 => KeyDomain::Bool, _ => KeyDomain::Raw,
                } }, value: child },
                _ => LayoutHead::Struct { fixed_bytes: w[r+1],
                    fixed: Fields::Flat { layout: checked, variable: false },
                    variable: Fields::Flat { layout: checked, variable: true },
                },
            }
        },
    }
}
impl Layout {
    /// Classifies a node without decoding unused child or key metadata.
    #[inline(always)]
    pub fn tag(&self)->(result:u32)
        ensures match self.head() {
            LayoutHead::Leaf(_)=>1<=result<=4,
            LayoutHead::Str=>result==5,
            LayoutHead::Option{..}=>result==6,
            LayoutHead::List{..}=>result==7,
            LayoutHead::Map{..}=>result==8,
            LayoutHead::Struct{..}=>result==9,
        },
    {
        proof {reveal(head_spec);}
        match self {
            Layout::Flat{checked}=>{checked.witness();checked.header(0)},
            Layout::Leaf(_)=>1,
            Layout::Str=>5,
            Layout::Option{..}=>6,
            Layout::List{..}=>7,
            Layout::Map{..}=>8,
            Layout::Struct{..}=>9,
        }
    }

    /// Mathematical immediate descriptor node.
    pub open spec fn head_spec(&self) -> LayoutHead<'_> { head_spec(self) }
    #[verifier::when_used_as_spec(head_spec)]
    #[inline(always)]
    /// Decodes immediate metadata, reusing a flat witness for children.
    pub fn head(&self) -> (result: LayoutHead<'_>)
        ensures result == head_spec(self),
    {
        proof { reveal(head_spec); }
        match self {
            Layout::Leaf(leaf) => LayoutHead::Leaf(*leaf),
            Layout::Str => LayoutHead::Str,
            Layout::Struct { fixed_bytes, fixed, variable } => LayoutHead::Struct {
                fixed_bytes: *fixed_bytes, fixed: Fields::Fixed(fixed), variable: Fields::Variable(variable),
            },
            Layout::Option { value } => LayoutHead::Option { value: **value },
            Layout::List { element } => LayoutHead::List { element: **element },
            Layout::Map { key, value } => LayoutHead::Map { key: *key, value: **value },
            Layout::Flat { checked } => {
                checked.witness();
                match checked.header(0) {
                    1 => LayoutHead::Leaf(Leaf::Unsigned(checked.header(1) as u8)),
                    2 => LayoutHead::Leaf(Leaf::Signed(checked.header(1) as u8)),
                    3 => LayoutHead::Leaf(Leaf::Bool),
                    4 => LayoutHead::Leaf(Leaf::Raw(checked.header(1))),
                    5 => LayoutHead::Str,
                    9 => LayoutHead::Struct { fixed_bytes: checked.header(1),
                        fixed: Fields::Flat { layout: checked, variable: false },
                        variable: Fields::Flat { layout: checked, variable: true },
                    },
                    _ => {
                        let value = checked.only_child();
                        let child = Layout::Flat { checked: value };
                        match checked.header(0) {
                            6 => LayoutHead::Option { value: child },
                            7 => LayoutHead::List { element: child },
                            _ => LayoutHead::Map { key: KeyLayout {
                                width: checked.header(6), domain: match checked.header(7) {
                                    1 => KeyDomain::Unsigned, 2 => KeyDomain::Signed,
                                    3 => KeyDomain::Bool, _ => KeyDomain::Raw,
                                },
                            }, value: child },
                        }
                    },
                }
            },
        }
    }
}
}
