//! Point selection from immutable, admitted collection geometry.

use vstd::prelude::*;

verus! {

/// Exact fixed-list header identity retained by an immutable borrow.
pub open spec fn admitted(bytes: Seq<u8>, width: u32) -> bool {
    0 < width && 4 <= bytes.len() <= u32::MAX
        && bytes.len() == 4 + crate::words::le_word(bytes,0) * width
}


/// Borrows one fixed-list scalar after exact count/stride admission.
///
/// # Panics
/// The caller must retain the admitted bytes and select an index below count.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_region(bytes: &[u8], width: u32, index: u32)
    -> (result: (&[u8], u32))
    requires admitted(bytes@, width),
        index < crate::words::le_word(bytes@, 0),
    ensures result.1 == 4 + index * width,
        result.0@ == bytes@.subrange(4 + index * width, 4 + (index + 1) * width),
        result.0.len() == width,
    no_unwind
{
    proof { scalar_containment(bytes@,width,index,0); }
    let start = 4 + index as usize * width as usize;
    let (_, tail) = bytes.split_at(start);
    let (value, _) = tail.split_at(width as usize);
    proof {
        assert(value@ =~= bytes@.subrange(4 + index * width,4 + (index + 1) * width));
    }
    (value, start as u32)
}

/// Fixed scalar bounds follow from the retained count and exact payload extent.
pub proof fn scalar_containment(bytes: Seq<u8>, width: u32, index: u32, base: u32)
    requires admitted(bytes,width), index < crate::words::le_word(bytes,0),
        base + bytes.len() <= u32::MAX,
    ensures 4 <= 4 + index * width,
        4 + index * width + width <= bytes.len(),
        base + 4 + index * width + width <= u32::MAX,
        4 + index * width + width == 4 + (index + 1) * width,
{
    assert(4 + index * width + width <= bytes.len()) by(nonlinear_arith)
        requires index < crate::words::le_word(bytes,0),
            bytes.len() == 4 + crate::words::le_word(bytes,0) * width, width > 0;
    assert(4 + index * width + width == 4 + (index + 1) * width) by(nonlinear_arith);
}

/// The lazy map constructor's complete header geometry, with untouched ends lazy.
pub open spec fn map_header(bytes: Seq<u8>, layout: crate::Layout,
    count: u32, key: u32, width: Option<u32>, stride: u32, payload: u32) -> bool
{
    &&& match layout.head() {
        crate::LayoutHead::Map { key: k, value } =>
            key == k.width && width == crate::layout::width_spec(value),
        _ => false,
    }
    &&& 4 <= bytes.len() <= u32::MAX
    &&& count == crate::words::le_word(bytes,0)
    &&& if count == 0 { bytes.len() == 4 && stride == 0 && payload == 4 }
        else {
            &&& stride == key + if width.is_some() { width.unwrap() as int } else { 4int }
            &&& 4 + count * stride <= bytes.len()
            &&& if width.is_some() { payload == 4 && bytes.len() == 4 + count * stride }
                else { payload == 4 + count * stride }
        }
}

/// Immutable bytes and descriptor preserve the cached header and every probe.
pub proof fn header_view_identity(before: Seq<u8>, after: Seq<u8>, layout: crate::Layout,
    count: u32, key: u32, width: Option<u32>, stride: u32, payload: u32, index: u32)
    requires before == after, map_header(before,layout,count,key,width,stride,payload),
        index <= count,
    ensures map_header(after,layout,count,key,width,stride,payload),
        index < count ==> {
            &&& 4 + index * stride + key <= before.len()
            &&& before.subrange(4 + index * stride,4 + index * stride + key)
                == after.subrange(4 + index * stride,4 + index * stride + key)
        },
{
    if index < count {
        assert(key <= stride);
        assert(4 + index * stride + key <= before.len()) by(nonlinear_arith)
            requires index < count, key <= stride, 4 + count * stride <= before.len();
    }
}

/// The original decoded-probe result, including exact first-error propagation.
pub open spec fn ordinary_probe(order: core::cmp::Ordering,
    query_error: Option<crate::Error>, key_error: Option<crate::Error>) -> crate::Result<core::cmp::Ordering>
{
    if query_error.is_some() { Err(query_error.unwrap()) }
    else if key_error.is_some() { Err(key_error.unwrap()) }
    else { Ok(order) }
}

/// The retained reader's equal-byte shortcut follows both domain decoders.
pub open spec fn retained_probe(candidate: Seq<u8>, query: Seq<u8>, order: core::cmp::Ordering,
    query_error: Option<crate::Error>, key_error: Option<crate::Error>) -> crate::Result<core::cmp::Ordering>
{
    if query_error.is_some() { Err(query_error.unwrap()) }
    else if key_error.is_some() { Err(key_error.unwrap()) }
    else if candidate == query { Ok(core::cmp::Ordering::Equal) }
    else { Ok(order) }
}

/// Byte equality selects exactly the comparator's equal result after decoding.
/// Error results from query and candidate decoding precede this decision.
pub proof fn equality_order_correspondence(candidate: Seq<u8>, query: Seq<u8>,
    order: core::cmp::Ordering, query_error: Option<crate::Error>, key_error: Option<crate::Error>)
    requires crate::search::lex_total(order,candidate,query),
    ensures (candidate == query) == (order == core::cmp::Ordering::Equal),
        retained_probe(candidate,query,order,query_error,key_error)
            == ordinary_probe(order,query_error,key_error),
        candidate != query ==> order != core::cmp::Ordering::Equal,
{
}

/// Each unequal probe strictly shrinks its bounded interval, for either half.
pub proof fn binary_search_progress(count: u32, lo: u32, hi: u32)
    requires lo < hi <= count,
    ensures lo <= lo + (hi-lo)/2 < hi,
        lo + (hi-lo)/2 + 1 <= hi,
        hi - (lo + (hi-lo)/2 + 1) < hi-lo,
        (lo + (hi-lo)/2) - lo < hi-lo,
        lo + (hi-lo)/2 < u32::MAX,
{
}

#[cfg(verus_only)]
fn point_witness()
    ensures true,
{
    let bytes = [2u8,0,0,0,7,9];
    let _payload = crate::raw::fixed_read_payload(&bytes,1);
    let (value, offset) = fixed_region(&bytes,1,1);
    proof {
        scalar_containment(bytes@,1,0,17);
        scalar_containment(bytes@,1,1,17);
        assert(value@ == seq![9u8] && offset == 5);
    }
}

#[cfg(verus_only)]
fn header_witness()
    ensures true,
{
    reveal(crate::Layout::head_spec);
    reveal(crate::layout_head::head_spec);
    let ghost fixed = seq![2u8,0,0,0,1,7,3,9];
    let ghost indexed = seq![1u8,0,0,0,1,255,255,255,255];
    let ghost fixed_layout = crate::Layout::Map {
        key: crate::KeyLayout { width:1,domain:crate::KeyDomain::Raw },
        value: &crate::Layout::Leaf(crate::Leaf::Unsigned(1)),
    };
    let ghost indexed_layout = crate::Layout::Map {
        key: crate::KeyLayout { width:1,domain:crate::KeyDomain::Raw },
        value: &crate::Layout::Str,
    };
    proof {
        header_view_identity(fixed,fixed,fixed_layout,2,1,Some(1u32),2,4,0);
        header_view_identity(fixed,fixed,fixed_layout,2,1,Some(1u32),2,4,1);
        header_view_identity(indexed,indexed,indexed_layout,1,1,None,5,9,0);
        assert(crate::words::le_word(indexed,5) > indexed.len()-9);
        let empty = seq![0u8,0,0,0];
        header_view_identity(empty,empty,fixed_layout,0,1,Some(1u32),0,4,0);
        header_view_identity(empty,empty,indexed_layout,0,1,None,0,4,0);
    }
}

#[cfg(verus_only)]
fn equality_witness()
    ensures true,
{
    let a = [1u8,7];
    let b = [1u8,9];
    let order = crate::compare_keys(&a,&b);
    proof {
        equality_order_correspondence(a@,b@,order,None,None);
        equality_order_correspondence(a@,a@,core::cmp::Ordering::Equal,None,None);
        equality_order_correspondence(a@,a@,core::cmp::Ordering::Equal,None,
            Some(crate::Error::Fault {kind:crate::ErrorKind::InvalidBool,offset:23}));
        assert(order == core::cmp::Ordering::Less);
    }
}

#[cfg(verus_only)]
fn progress_witness()
    ensures true,
{
    proof {
        binary_search_progress(1,0,1);
        binary_search_progress(u32::MAX,0,u32::MAX);
        binary_search_progress(u32::MAX,(u32::MAX-1) as u32,u32::MAX);
    }
}

}
