//! Integer-only schema tables and const certification of exact immutable words.
//!
//! Every record occupies eight words. Field records precede their owning node;
//! child nodes precede the field records. Word indices are absolute in the table.
#![cfg_attr(verus_only, allow(missing_docs))]
use vstd::prelude::*;

verus! {

/// Number of words in every node or field record.
pub const RECORD_WORDS: usize = 8;

/// Private metadata certificate for a table, root word index and depth budget.
/// Certification establishes metadata only, independently of account contents.
#[derive(Clone, Copy, Debug)]
pub struct CheckedLayout<'a> {
    words: &'a [u32],
    root: u32,
    budget: usize,
}

/// A supported integer width.
pub open spec fn number_width(width: int) -> bool {
    width == 1 || width == 2 || width == 4 || width == 8 || width == 16
}

/// Fixed kind is determined by the explicit node tag and struct field counts.
pub open spec fn is_fixed(words: Seq<u32>, at: int) -> bool {
    1 <= words[at] <= 4 || (words[at] == 9 && words[at + 4] == 0)
}

/// Certified sufficient condition for a fixed type with no invalid bit patterns.
/// A zero struct flag retains the ordinary validator for older table producers.
pub open spec fn unrestricted(words: Seq<u32>, at: int) -> bool {
    words[at] == 1 || words[at] == 2 || words[at] == 4
        || (words[at] == 9 && words[at+6] == 1)
}

/// Every field of a flagged fixed struct is itself unrestricted.
#[verifier::opaque]
pub open spec fn unrestricted_fields(words: Seq<u32>, start: int, count: int) -> bool {
    forall|i:int| 0 <= i < count ==>
        #[trigger] unrestricted(words, words[start+8*i+3] as int)
}

/// Local field-record relation. Field children are earlier aligned node records.
pub open spec fn field_valid(words: Seq<u32>, at: int) -> bool {
    &&& 0 <= at && at % 8 == 0 && at + 8 <= words.len()
    &&& words[at] == 10
    &&& words[at + 1] < 65535
    &&& words[at + 3] < at
    &&& words[at + 3] % 8 == 0
    &&& 1 <= words[words[at + 3] as int] <= 9
    &&& words[at + 4] < 65535 && words[at + 5] == 0
    &&& words[at + 6] == 0 && words[at + 7] == 0
}

/// A struct's two declaration-ordered field spans merge to dense ordinals.
pub open spec fn struct_fields(
    words: Seq<u32>, at: int, start: int, nf: int, nv: int,
    i: int, j: int, offset: int, depth: int,
) -> bool
    decreases nf + nv - i - j,
{
    if !(0 <= i <= nf && 0 <= j <= nv) { false }
    else if i == nf && j == nv { offset == words[at + 1] }
    else if i < nf && (j == nv || words[start + 8*i + 1] <= words[start + 8*(nf+j) + 1]) {
        let f = start + 8*i;
        let child = words[f + 3] as int;
        &&& field_valid(words, f) && child < start
        &&& words[f + 1] == i + j && words[f + 2] == offset
        &&& words[start+8*(i+j)+4] == i
        &&& is_fixed(words, child) && words[child + 1] > 0
        &&& offset + words[child + 1] <= u32::MAX
        &&& words[child + 2] < depth
        &&& struct_fields(words, at, start, nf, nv, i+1, j,
                offset + words[child + 1], depth)
    } else {
        let f = start + 8*(nf+j);
        let child = words[f + 3] as int;
        &&& field_valid(words, f) && child < start
        &&& words[f + 1] == i + j && words[f + 2] == 0
        &&& words[start+8*(i+j)+4] == nf+j
        &&& !is_fixed(words, child) && words[child + 2] < depth
        &&& struct_fields(words, at, start, nf, nv, i, j+1, offset, depth)
    }
}

proof fn struct_field_member(
    words: Seq<u32>, at: int, start: int, nf: int, nv: int,
    i: int, j: int, offset: int, depth: int, index: int,
)
    requires struct_fields(words, at, start, nf, nv, i, j, offset, depth),
        (i <= index < nf || nf+j <= index < nf+nv),
    ensures field_valid(words, start+8*index)
        && words[start+8*index+3] < start,
    decreases nf+nv-i-j,
{
    reveal_with_fuel(struct_fields, 2);
    if i < nf && (j == nv || words[start+8*i+1] <= words[start+8*(nf+j)+1]) {
        if index != i {
            struct_field_member(words, at, start, nf, nv, i+1, j,
                offset+words[words[start+8*i+3] as int+1], depth, index);
        }
    } else if index != nf+j {
        struct_field_member(words, at, start, nf, nv, i, j+1, offset, depth, index);
    }
}

proof fn struct_declaration_member(
    words: Seq<u32>, at: int, start: int, nf: int, nv: int,
    i: int, j: int, offset: int, depth: int, declaration: int,
)
    requires struct_fields(words,at,start,nf,nv,i,j,offset,depth),
        i+j <= declaration < nf+nv,
    ensures {
        let index=words[start+8*declaration+4] as int;
        0 <= index < nf+nv && words[start+8*index+1]==declaration
    },
    decreases nf+nv-i-j,
{
    reveal_with_fuel(struct_fields,2);
    if i<nf && (j==nv || words[start+8*i+1] <= words[start+8*(nf+j)+1]) {
        if declaration>i+j {
            struct_declaration_member(words,at,start,nf,nv,i+1,j,
                offset+words[words[start+8*i+3] as int+1],depth,declaration);
        }
    } else if declaration>i+j {
        struct_declaration_member(words,at,start,nf,nv,i,j+1,offset,depth,declaration);
    }
}

/// Depth of the child selected by one field record.
pub open spec fn field_depth(words: Seq<u32>, start: int, index: int) -> int {
    words[words[start + 8*index + 3] as int + 2] as int
}

/// Local node relation, including exact header domains and child spans.
pub open spec fn node_valid(words: Seq<u32>, at: int, budget: int) -> bool {
    &&& 0 <= at && at % 8 == 0 && at + 8 <= words.len()
    &&& 1 <= words[at + 2] <= budget
    &&& match words[at] {
        1 | 2 | 3 | 4 | 5 => {
            &&& words[at + 2] == 1
            &&& words[at + 3] == 0 && words[at + 4] == 0 && words[at + 5] == 0
            &&& words[at + 6] == 0 && words[at + 7] == 0
            &&& match words[at] {
                1 | 2 => number_width(words[at + 1] as int),
                3 => words[at + 1] == 1,
                4 => words[at + 1] > 0,
                _ => words[at + 1] == 0,
            }
        },
        6 | 7 | 8 => {
            let f = at - 8;
            let child = words[f + 3] as int;
            &&& at >= 8 && field_valid(words, f) && child < f
            &&& words[f + 1] == 0 && words[f + 2] == 0 && words[f+4] == 0
            &&& words[at + 1] == 0 && words[at + 3] == 0
            &&& words[at + 4] == 1 && words[at + 5] == f
            &&& words[at + 2] == words[child + 2] + 1
            &&& if words[at] == 8 {
                match words[at + 7] {
                    1 | 2 => number_width(words[at + 6] as int),
                    3 => words[at + 6] == 1,
                    4 => words[at + 6] > 0,
                    _ => false,
                }
            } else { words[at + 6] == 0 && words[at + 7] == 0 }
        },
        9 => {
            let nf = words[at + 3] as int;
            let nv = words[at + 4] as int;
            let start = words[at + 5] as int;
            &&& 0 < nf + nv <= 65535
            &&& start + 8*(nf+nv) == at
            &&& words[at + 6] <= 1 && words[at + 7] == 0
            &&& (words[at+6] == 1 ==> nv == 0 && unrestricted_fields(words, start, nf))
            &&& (nv > 0 || words[at + 1] > 0)
            &&& struct_fields(words, at, start, nf, nv, 0, 0, 0, words[at + 2] as int)
            &&& exists|i:int| 0 <= i < nf+nv
                && #[trigger] field_depth(words, start, i) + 1 == words[at + 2]
        },
        _ => false,
    }
}

/// Complete table validity, with an exact admitted root and node-depth budget.
pub open spec fn layout_valid(words: Seq<u32>, root: int, budget: int) -> bool {
    &&& 0 < budget <= crate::MAX_DEPTH
    &&& words.len() > 0 && words.len() % 8 == 0 && words.len() <= u32::MAX
    &&& 0 <= root < words.len() && root % 8 == 0
    &&& 1 <= words[root] <= 9
    &&& forall|i:int| 0 <= i < words.len()/8 ==>
        (#[trigger] node_valid(words, 8*i, budget) || field_valid(words, 8*i))
}

impl<'a> CheckedLayout<'a> {
    #[verifier::type_invariant]
    spec fn invariant(self) -> bool { self.valid() }

    /// Returns the certified metadata fact without rechecking table words.
    #[inline(always)]
    pub fn witness(self)
        ensures self.valid(), node_valid(self.view(), self.root_index(), self.depth_budget()),
    { proof {
        use_type_invariant(&self);
        assert(node_valid(self.words@, 8*(self.root/8), self.budget as int)
            || field_valid(self.words@, 8*(self.root/8)));
    } }

    /// Selects an aligned node from this already certified table.
    /// Only the requested root's bounds and tag are inspected.
    pub const fn subroot(self, root: usize) -> (result: Option<Self>)
        ensures result.is_some() == (root < self.view().len() && root % 8 == 0
            && 1 <= self.view()[root as int] <= 9),
            result.is_some() ==> result.unwrap().valid()
                && result.unwrap().view() == self.view()
                && result.unwrap().root_index() == root
                && result.unwrap().depth_budget() == self.depth_budget(),
    {
        proof { use_type_invariant(&self); }
        if root >= self.words.len() || !root.is_multiple_of(8)
            || self.words[root] < 1 || self.words[root] > 9 { return None; }
        Some(Self { words: self.words, root: root as u32, budget: self.budget })
    }

    /// Reads one word from the root's eight-word node header.
    #[inline(always)]
    pub const fn header(self, slot: usize) -> (result: u32)
        requires slot < 8,
        ensures result == self.view()[self.root_index() + slot], self.valid(),
    {
        proof { use_type_invariant(&self); }
        self.words[(self.root as usize).wrapping_add(slot)]
    }


    /// Exposes the checked root's local record relation to kernel proofs.
    pub proof fn metadata(self)
        requires self.valid(),
        ensures node_valid(self.view(), self.root_index(), self.depth_budget()),
    {
        assert(node_valid(self.words@, 8*(self.root/8), self.budget as int)
            || field_valid(self.words@, 8*(self.root/8)));
    }

    /// Reuses complete table validity at another aligned node.
    pub proof fn retarget_valid(self, root: int)
        requires self.valid(), 0 <= root < self.view().len(), root % 8 == 0,
            1 <= self.view()[root] <= 9,
        ensures self.retarget(root).valid(), self.retarget(root).view() == self.view(),
            self.retarget(root).root_index() == root,
            self.retarget(root).depth_budget() == self.depth_budget(),
            node_valid(self.view(), root, self.depth_budget()),
    {
        assert(node_valid(self.words@, 8*(root/8), self.budget as int)
            || field_valid(self.words@, 8*(root/8)));
    }

    /// The same table and budget with another mathematical root.
    pub closed spec fn retarget(self, root: int) -> Self {
        Self { words: self.words, root: root as u32, budget: self.budget }
    }

    /// Looks up a struct declaration through the certified ordinal-to-wire map.
    /// This reads metadata only and never traverses or validates account bytes.
    #[inline(always)]
    pub fn declaration_index(self,declaration:u16)->(result:Option<usize>)
        ensures result.is_some() == (self.view()[self.root_index()]==9
                && declaration < self.view()[self.root_index()+3] + self.view()[self.root_index()+4]),
            result.is_some() ==> result.unwrap()
                == self.view()[self.view()[self.root_index()+5] + 8*(declaration as int) + 4],
            result.is_some() ==> result.unwrap()
                < self.view()[self.root_index()+3] + self.view()[self.root_index()+4],
            result.is_some() ==> self.view()[self.view()[self.root_index()+5]
                + 8*(result.unwrap() as int) + 1] == declaration,
    {
        self.witness();
        if self.header(0)!=9 {return None;}
        let nf=self.header(3) as usize;
        let nv=self.header(4) as usize;
        if declaration as usize>=nf+nv {return None;}
        let start=self.header(5) as usize;
        proof {
            struct_declaration_member(self.words@,self.root as int,start as int,
                nf as int,nv as int,0,0,0,self.words[self.root as usize+2] as int,declaration as int);
            vstd::arithmetic::div_mod::lemma_small_mod(declaration as nat*8,usize::MAX as nat+1);
        }
        let offset=(declaration as usize).wrapping_mul(8);
        proof {assert(offset as int==8*(declaration as int));}
        Some(self.words[start.wrapping_add(offset).wrapping_add(4)] as usize)
    }

    /// Local field selection facts inherited from complete table certification.
    pub proof fn field_metadata(self, index: int, variable: bool)
        requires self.valid(), 0 <= index < self.view()[self.root_index() + if variable { 4int } else { 3int }],
        ensures node_valid(self.view(), self.root_index(), self.depth_budget()), {
            let f = self.view()[self.root_index()+5] + 8*(index + if variable { self.view()[self.root_index()+3] as int } else { 0int });
            0 <= f && f+8 <= self.view().len() && field_valid(self.view(),f)
        },
    {
        self.metadata();
        let w=self.words@; let r=self.root as int;
        let nf=w[r+3] as int; let nv=w[r+4] as int; let start=w[r+5] as int;
        let ordinal=index+if variable {nf} else {0int};
        if w[r]==9 { struct_field_member(w,r,start,nf,nv,0,0,0,w[r+2] as int,ordinal); }
    }

    /// Reads one integer field word after checking its field ordinal.
    /// Invalid ordinals panic before any child witness can be constructed.
    #[inline(always)]
    pub fn field_word(self, index: usize, variable: bool, slot: usize) -> (result: u32)
        requires slot < 8, index < self.view()[self.root_index() + if variable {4int} else {3int}],
        ensures result == self.view()[self.view()[self.root_index()+5]
            + 8*(index + if variable {self.view()[self.root_index()+3] as int} else {0int}) + slot],
    {
        proof { use_type_invariant(&self); self.field_metadata(index as int,variable); }
        assert!(index < self.header(if variable {4} else {3}) as usize, "schema field out of bounds");
        let start=self.header(5) as usize;
        let ordinal=index.wrapping_add(if variable {self.header(3) as usize} else {0});
        proof {
            assert(index + if variable {self.view()[self.root_index()+3] as int} else {0int} <= 65535);
            assert(ordinal as int == index + if variable {self.view()[self.root_index()+3] as int} else {0int});
            assert(start + 8*(ordinal as int) + slot < self.words@.len());
        }
        proof { vstd::arithmetic::div_mod::lemma_small_mod(ordinal as nat * 8, usize::MAX as nat + 1); }
        let offset=ordinal.wrapping_mul(8);
        proof { assert(offset as int == 8*(ordinal as int)); }
        let at=start.wrapping_add(offset).wrapping_add(slot);
        proof { assert(at as int == start + 8*(ordinal as int) + slot); }
        self.words[at]
    }

    /// Inherits the sole schema child of a certified collection or option.
    #[inline(always)]
    pub(crate) fn only_child(self)->(result:Self)
        requires 6<=self.view()[self.root_index()]<=8,
        ensures result==self.retarget(self.view()[self.view()[self.root_index()+5] as int+3] as int),
            result.valid(),
    {
        self.witness();
        // Unary field records immediately precede their parent node.
        let root=self.words[(self.root as usize).wrapping_sub(5)] as usize;
        Self{words:self.words,root:root as u32,budget:self.budget}
    }

    /// Inherits one checked child after checking its field ordinal.
    #[inline(always)]
    pub fn child(self, index: usize, variable: bool) -> (result: Self)
        requires index < self.view()[self.root_index() + if variable {4int} else {3int}],
        ensures result == self.retarget(self.view()[self.view()[self.root_index()+5]
            + 8*(index + if variable {self.view()[self.root_index()+3] as int} else {0int}) + 3] as int),
            result.valid(),
    {
        if variable && index == 0 {
            let kind = self.header(0);
            if matches!(kind, 6..=8) {
                proof { self.metadata(); }
                return self.only_child();
            }
        }
        let root=self.field_word(index,variable,3) as usize;
        proof { use_type_invariant(&self); self.field_metadata(index as int,variable); }
        Self {words:self.words,root:root as u32,budget:self.budget}
    }

    /// Selects a field and inherits its checked child root without certification.
    #[inline(always)]
    pub fn field(self, index: usize, variable: bool) -> (result: (u16, u32, Self))
        requires index < self.view()[self.root_index() + if variable { 4int } else { 3int }],
        ensures
            result.0 == self.view()[self.view()[self.root_index()+5]
                + 8*(index + if variable { self.view()[self.root_index()+3] as int } else { 0int }) + 1] as u16,
            result.1 == self.view()[self.view()[self.root_index()+5]
                + 8*(index + if variable { self.view()[self.root_index()+3] as int } else { 0int }) + 2],
            result.2 == self.retarget(self.view()[self.view()[self.root_index()+5]
                + 8*(index + if variable { self.view()[self.root_index()+3] as int } else { 0int }) + 3] as int),
            result.2.valid(),
    {
        proof { use_type_invariant(&self); self.field_metadata(index as int, variable); }
        assert!(index < self.header(if variable {4} else {3}) as usize, "schema field out of bounds");
        let start=self.header(5) as usize;
        let ordinal=index.wrapping_add(if variable {self.header(3) as usize} else {0});
        proof {
            assert(index + if variable {self.view()[self.root_index()+3] as int} else {0int} <= 65535);
            assert(ordinal as int == index + if variable {self.view()[self.root_index()+3] as int} else {0int});
            assert(start + 8*(ordinal as int) + 8 <= self.words@.len());
        }
        proof { vstd::arithmetic::div_mod::lemma_small_mod(ordinal as nat * 8, usize::MAX as nat + 1); }
        let offset=ordinal.wrapping_mul(8);
        proof { assert(offset as int == 8*(ordinal as int)); }
        let f=start.wrapping_add(offset);
        proof { assert(f as int == start + 8*(ordinal as int)); }
        let field=&self.words[f..f.wrapping_add(8)];
        let result=Self {words:self.words,root:field[3],budget:self.budget};
        (field[1] as u16,field[2],result)
    }

    /// Selects a packed struct field after the declaration map proves its ordinal.
    #[inline(always)]
    pub(crate) fn packed_field(self, index: usize) -> (result: (u32, Self))
        requires self.view()[self.root_index()] == 9,
            index < self.view()[self.root_index()+3] + self.view()[self.root_index()+4],
        ensures result.0 == self.view()[self.view()[self.root_index()+5] + 8*index + 2],
            result.1 == self.retarget(self.view()[self.view()[self.root_index()+5] + 8*index + 3] as int),
            result.1.valid(),
    {
        proof {
            use_type_invariant(&self);
            let nf = self.view()[self.root_index()+3] as int;
            if index < nf { self.field_metadata(index as int, false); }
            else { self.field_metadata(index as int - nf, true); }
            vstd::arithmetic::div_mod::lemma_small_mod(index as nat * 8, usize::MAX as nat + 1);
        }
        let start = self.header(5) as usize;
        let offset = index.wrapping_mul(8);
        proof { assert(offset as int == 8*index); }
        let at = start.wrapping_add(offset);
        proof { assert(at as int == start + 8*index); }
        let child = Self { words: self.words, root: self.words[at.wrapping_add(3)], budget: self.budget };
        (self.words[at.wrapping_add(2)], child)
    }

    /// Exact integer table identified by this witness.
    #[inline(always)]
    pub const fn words(self) -> (result: &'a [u32])
        ensures result@ == self.view(),
    { self.words }

    /// Root's absolute word index.
    #[inline(always)]
    pub const fn root(self) -> (result: usize)
        ensures result == self.root_index(),
    { self.root as usize }

    /// Depth budget checked when this witness was constructed.
    #[inline(always)]
    pub const fn budget(self) -> (result: usize)
        ensures result == self.depth_budget(),
    { self.budget }

    /// Whether this certified fixed type admits every bit pattern.
    #[inline(always)]
    pub const fn is_unrestricted(self) -> (result: bool)
        ensures result == unrestricted(self.view(), self.root_index()),
    {
        self.header(0) == 1 || self.header(0) == 2 || self.header(0) == 4
            || (self.header(0) == 9 && self.header(6) == 1)
    }

    /// Exact immutable words in the certificate.
    pub closed spec fn view(self) -> Seq<u32> { self.words@ }
    /// Exact root word index in the certificate.
    pub closed spec fn root_index(self) -> int { self.root as int }
    /// Exact admitted depth budget in the certificate.
    pub closed spec fn depth_budget(self) -> int { self.budget as int }

    /// Metadata fact carried by a successfully constructed witness.
    pub open spec fn valid(self) -> bool {
        layout_valid(self.view(), self.root_index(), self.depth_budget())
    }
}

const fn check_field(words: &[u32], at: usize) -> (result: bool)
    requires at % 8 == 0, at <= words.len(), words.len() - at >= 8,
    ensures result == field_valid(words@, at as int),
{
    if words[at] != 10 || words[at+1] >= 65535 || words[at+3] as usize >= at
        || !words[at+3].is_multiple_of(8) || words[at+4] >= 65535 || words[at+5] != 0
        || words[at+6] != 0 || words[at+7] != 0 { return false; }
    let child = words[at+3] as usize;
    words[child] >= 1 && words[child] <= 9
}

const fn fixed(words: &[u32], at: usize) -> (result: bool)
    requires at <= words.len(), words.len() - at >= 8,
    ensures result == is_fixed(words@, at as int),
{ (words[at] >= 1 && words[at] <= 4) || (words[at] == 9 && words[at+4] == 0) }

const fn check_node(words: &[u32], at: usize, budget: usize) -> (result: bool)
    requires at % 8 == 0, at <= words.len(), words.len() - at >= 8,
        words.len() <= u32::MAX, budget <= crate::MAX_DEPTH,
    ensures result == node_valid(words@, at as int, budget as int),
{
    reveal(unrestricted_fields);
    let tag = words[at];
    let width = words[at+1];
    let depth = words[at+2];
    if depth == 0 || depth as usize > budget { return false; }
    if tag >= 1 && tag <= 5 {
        if depth != 1 || words[at+3] != 0 || words[at+4] != 0 || words[at+5] != 0
            || words[at+6] != 0 || words[at+7] != 0 { return false; }
        return match tag {
            1 | 2 => width == 1 || width == 2 || width == 4 || width == 8 || width == 16,
            3 => width == 1,
            4 => width > 0,
            _ => width == 0,
        };
    }
    if tag >= 6 && tag <= 8 {
        if at < 8 { return false; }
        let f = at - 8;
        if !check_field(words, f) { return false; }
        let child = words[f+3] as usize;
        if child >= f || words[f+1] != 0 || words[f+2] != 0 || words[f+4] != 0
            || width != 0 || words[at+3] != 0 || words[at+4] != 1
            || words[at+5] as usize != f || words[child+2] as u64 + 1 != depth as u64 {
            return false;
        }
        return if tag == 8 {
            let kw = words[at+6];
            match words[at+7] {
                1 | 2 => kw == 1 || kw == 2 || kw == 4 || kw == 8 || kw == 16,
                3 => kw == 1,
                4 => kw > 0,
                _ => false,
            }
        } else { words[at+6] == 0 && words[at+7] == 0 };
    }
    if tag != 9 { return false; }
    let nf = words[at+3] as usize;
    let nv = words[at+4] as usize;
    let start = words[at+5] as usize;
    if nf > 65535 || nv > 65535 - nf || nf+nv == 0
        || start as u64 + 8*(nf as u64+nv as u64) != at as u64
        || words[at+6] > 1 || words[at+7] != 0 || (nv == 0 && width == 0)
        || (words[at+6] == 1 && nv != 0) {
        return false;
    }
    let mut i = 0usize;
    let mut j = 0usize;
    let mut offset = 0u32;
    let mut maximum = 0u32;
    let ghost mut maximum_slot: int = 0;
    while i < nf || j < nv
        invariant
            at % 8 == 0, at + 8 <= words.len(), words.len() <= u32::MAX,
            nf <= 65535, nv <= 65535 - nf, nf+nv > 0,
            words[at as int] == 9, words[at as int + 1] == width,
            words[at as int + 2] == depth, 0 < depth <= budget,
            words[at as int + 3] == nf, words[at as int + 4] == nv,
            words[at as int + 5] == start, words[at as int + 6] <= 1,
            words[at as int + 6] == 1 ==> nv == 0,
            words[at as int + 6] == 1 ==> (forall|k:int| 0 <= k < i ==>
                #[trigger] unrestricted(words@, words@[start+8*k+3] as int)),
            words[at as int + 7] == 0, nv > 0 || width > 0,
            start + 8*(nf+nv) == at, i <= nf, j <= nv,
            struct_fields(words@, at as int, start as int, nf as int, nv as int,
                0, 0, 0, depth as int)
            == struct_fields(words@, at as int, start as int, nf as int, nv as int,
                i as int, j as int, offset as int, depth as int),
            maximum < depth,
            forall|k:int| (0 <= k < i || nf <= k < nf+j) ==>
                #[trigger] field_depth(words@, start as int, k) <= maximum,
            i+j > 0 ==> (0 <= maximum_slot < i || nf <= maximum_slot < nf+j)
                && field_depth(words@, start as int, maximum_slot) == maximum,
        decreases nf + nv - i - j,
    {
        proof { reveal_with_fuel(struct_fields, 2); }
        let use_fixed = i < nf && (j == nv || words[start+8*i+1] <= words[start+8*(nf+j)+1]);
        let index = if use_fixed { i } else { nf+j };
        let f = start + 8*index;
        if !check_field(words, f) { return false; }
        let child = words[f+3] as usize;
        if child >= start || words[f+1] as usize != i+j || words[child+2] >= depth
            || words[start+8*(i+j)+4] as usize != index {
            return false;
        }
        if i+j == 0 || words[child+2] > maximum {
            maximum = words[child+2];
            proof { maximum_slot = index as int; }
        }
        if words[at+6] == 1
            && !(words[child] == 1 || words[child] == 2 || words[child] == 4
                || (words[child] == 9 && words[child+6] == 1)) {
                assert(index == i && i < nf);
                assert(!unrestricted(words@,words@[start+8*(i as int)+3] as int));
                assert(!unrestricted_fields(words@,start as int,nf as int)) by {reveal(unrestricted_fields);}
                return false;
        }
        if use_fixed {
            if !fixed(words, child) || words[child+1] == 0 || words[f+2] != offset {
                return false;
            }
            offset = match offset.checked_add(words[child+1]) {
                Some(next) => next,
                None => return false,
            };
            i += 1;
        } else {
            if fixed(words, child) || words[f+2] != 0 { return false; }
            j += 1;
        }
    }
    proof {
        assert forall|k:int| 0 <= k < nf+nv implies
            #[trigger] field_depth(words@, start as int, k) <= maximum by { }
        assert((exists|k:int| 0 <= k < nf+nv
            && #[trigger] field_depth(words@, start as int, k) + 1 == depth)
            == (maximum + 1 == depth));
        reveal_with_fuel(struct_fields, 2);
        assert(struct_fields(words@, at as int, start as int, nf as int, nv as int,
            0, 0, 0, depth as int) == (offset == width));
        assert(node_valid(words@, at as int, budget as int)
            == (offset == width && maximum + 1 == depth));
    }
    offset == width && maximum as u64 + 1 == depth as u64
}

/// Certifies all table records and the exact root, or rejects malformed metadata.
/// This same body runs during const evaluation and at arbitrary-table gateways.
/// No witness can be constructed by bypassing this check in ordinary safe Rust.
pub const fn certify(words: &[u32], root: usize, budget: usize)
    -> (result: Option<CheckedLayout<'_>>)
    ensures result.is_some() == layout_valid(words@, root as int, budget as int),
        result.is_some() ==> result.unwrap().view() == words@
            && result.unwrap().root_index() == root && result.unwrap().depth_budget() == budget
            && result.unwrap().valid(),
{
    if budget == 0 || budget > crate::MAX_DEPTH || words.is_empty()
        || !words.len().is_multiple_of(8) || words.len() > u32::MAX as usize
        || root >= words.len() || !root.is_multiple_of(8) { return None; }
    if words[root] < 1 || words[root] > 9 { return None; }
    let mut i = 0usize;
    while i < words.len()/8
        invariant
            words.len() > 0, words.len() % 8 == 0, words.len() <= u32::MAX,
            0 < budget <= crate::MAX_DEPTH, root < words.len(), root % 8 == 0,
            1 <= words[root as int] <= 9, i <= words.len()/8,
            forall|k:int| 0 <= k < i ==>
                (#[trigger] node_valid(words@, 8*k, budget as int) || field_valid(words@, 8*k)),
        decreases words.len()/8-i,
    {
        let at = 8*i;
        if !check_node(words, at, budget) && !check_field(words, at) { return None; }
        i += 1;
    }
    Some(CheckedLayout { words, root: root as u32, budget })
}

}
