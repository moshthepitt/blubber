//! Implementation support for downstream macro expansions.

/// Encoder protocol selected entirely from monomorphized graph markers.
///
/// The contract route uses proved library kernels with trusted projection and
/// implementer premises. A true handwritten marker is not a proof of its code.
/// Both retained runtime checks remain active until final S0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncoderRoute {
    /// A default-false node keeps its complete parent graph checked.
    Checked,
    /// Every child/key supplies the documented encoder and replay contract.
    Contract,
}

/// Selects a graph route without invoking an encoder or custom conversion hook.
///
/// Generic wrappers and aliases read the same associated constants after normal
/// Rust monomorphization. Field projection permission is independent of these
/// complete encoder/measurement contracts.
#[inline(always)]
pub const fn encoder_route<S: crate::Schema>() -> EncoderRoute {
    if blubber_core::encoding_product::contract(blubber_core::encoding_product::Markers {
        encoder: S::__ENCODER_AUDITED,
        replay: S::__MEASURE_REUSABLE,
    }) {
        EncoderRoute::Contract
    } else {
        EncoderRoute::Checked
    }
}

pub use crate::account::{PrefixChoice, SelectPrefix};
pub use crate::edit::MeasuredInit;
pub use crate::projection::{ProjectField, Projection};
pub use crate::storage::Writer;
/// Shared type-directed derive support.
pub mod product {
    pub use crate::product::*;
}
pub use blubber_core::encoding_product::Pair;
/// Checked envelope-plus-reservation arithmetic used by generated helpers.
pub const fn account_space(minimum: u32, reserve: usize) -> crate::Result<usize> {
    crate::account::account_space(minimum, reserve)
}

pub use crate::schema::check_depth as check_schema_depth;
pub use crate::schema::checked_add;
pub use crate::schema_table::{flatten, schema_layout, Cons, Nil, SchemaList};
pub use blubber_core::{certify, CheckedLayout};
pub use blubber_core::{FixedField, Layout, VariableField, MAX_DEPTH};

/// Resolves a derived field in the consumer's scope before binding masks exist.
pub trait SchemaField<const INDEX: usize> {
    /// The original field schema, without generated names in its scope.
    type Type: crate::Schema;
}

// Select using the dependency's feature, even when the consumer has no local
// `alloc` feature. Proc-macro feature unification must not select this branch.
#[doc(hidden)]
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! __with_alloc {
    ($($item:tt)*) => { $($item)* };
}

#[doc(hidden)]
#[cfg(not(feature = "alloc"))]
#[macro_export]
macro_rules! __with_alloc {
    ($($item:tt)*) => {};
}

pub use crate::__with_alloc as with_alloc;

// Select on the library's feature, independent of downstream feature names.
#[doc(hidden)]
#[cfg(feature = "pinocchio")]
#[macro_export]
macro_rules! __with_pinocchio {
    ($($item:tt)*) => { $($item)* };
}

#[doc(hidden)]
#[cfg(not(feature = "pinocchio"))]
#[macro_export]
macro_rules! __with_pinocchio {
    ($($item:tt)*) => {};
}

pub use crate::__with_pinocchio as with_pinocchio;

/// Checks a schema's kind, widths and complete descriptor during const evaluation.
pub const fn assert_schema<S: crate::Schema>(budget: usize) {
    let (width, minimum) = layout_sizes(S::LAYOUT, budget);
    assert!(
        S::MIN_SIZE == minimum,
        "Schema minimum differs from descriptor"
    );
    match (S::KIND, S::FIXED_SIZE, width) {
        (crate::Kind::Fixed, Some(declared), Some(actual)) => {
            assert!(
                declared == actual,
                "Schema fixed width differs from descriptor"
            );
        }
        (crate::Kind::Variable, None, None) => {}
        _ => panic!("Schema kind differs from descriptor"),
    }
}

const fn sum(left: u32, right: u32) -> u32 {
    match left.checked_add(right) {
        Some(value) => value,
        None => panic!("schema wire width exceeds u32"),
    }
}

// Const counterpart of core metadata validation, with minimum-size calculation.
const fn layout_sizes(layout: &Layout, budget: usize) -> (Option<u32>, u32) {
    assert!(
        budget > 0,
        "schema depth exceeds MAX_DEPTH; consider max-depth-16"
    );
    match layout {
        Layout::Flat { .. } => panic!("recursive CTFE descriptor expected"),
        Layout::Leaf(leaf) => {
            let width = match *leaf {
                blubber_core::Leaf::Unsigned(width) | blubber_core::Leaf::Signed(width) => {
                    assert!(matches!(width, 1 | 2 | 4 | 8 | 16), "invalid integer width");
                    width as u32
                }
                blubber_core::Leaf::Bool => 1,
                blubber_core::Leaf::Raw(width) => width,
            };
            assert!(width > 0, "fixed fields must have positive width");
            (Some(width), width)
        }
        Layout::Str => (None, 0),
        Layout::Option { value } => {
            let _ = layout_sizes(value, budget - 1);
            (None, 1)
        }
        Layout::List { element } => {
            let _ = layout_sizes(element, budget - 1);
            (None, 4)
        }
        Layout::Map { key, value } => {
            let valid = match key.domain {
                blubber_core::KeyDomain::Unsigned | blubber_core::KeyDomain::Signed => {
                    matches!(key.width, 1 | 2 | 4 | 8 | 16)
                }
                blubber_core::KeyDomain::Bool => key.width == 1,
                blubber_core::KeyDomain::Raw => key.width > 0,
            };
            assert!(valid, "invalid map key descriptor");
            let _ = layout_sizes(value, budget - 1);
            (None, 4)
        }
        Layout::Struct {
            fixed_bytes,
            fixed,
            variable,
        } => {
            assert!(
                fixed.len() <= u16::MAX as usize,
                "invalid schema field count"
            );
            assert!(
                variable.len() <= u16::MAX as usize - fixed.len(),
                "invalid schema field count"
            );
            assert!(
                !fixed.is_empty() || !variable.is_empty(),
                "schema requires named fields"
            );
            let mut i = 0;
            let mut j = 0;
            let mut offset = 0;
            let mut minimum = 0;
            while i < fixed.len() || j < variable.len() {
                if i < fixed.len()
                    && (j == variable.len() || fixed[i].declaration <= variable[j].declaration)
                {
                    let field = &fixed[i];
                    assert!(
                        field.declaration as usize == i + j && field.offset == offset,
                        "invalid fixed field ordinal or offset"
                    );
                    let width = match layout_sizes(field.layout, budget - 1).0 {
                        Some(width) => width,
                        None => panic!("fixed field has variable descriptor"),
                    };
                    offset = sum(offset, width);
                    minimum = sum(minimum, width);
                    i += 1;
                } else {
                    let field = &variable[j];
                    assert!(
                        field.declaration as usize == i + j,
                        "invalid variable field ordinal"
                    );
                    let (width, child_minimum) = layout_sizes(field.layout, budget - 1);
                    assert!(width.is_none(), "variable field has fixed descriptor");
                    minimum = sum(minimum, child_minimum);
                    j += 1;
                }
            }
            assert!(offset == *fixed_bytes, "invalid fixed prefix width");
            if variable.is_empty() {
                assert!(offset > 0, "fixed fields must have positive width");
                (Some(offset), minimum)
            } else {
                (None, sum(minimum, (variable.len() as u32 - 1) * 4))
            }
        }
    }
}

/// Checks fixed descriptor width and depth during derived const evaluation.
///
/// The budget bounds recursion even for a manually supplied descriptor cycle.
/// This is metadata checking only; typed byte reads remain checked at runtime.
pub const fn assert_fixed_layout(layout: &Layout, width: u32, budget: usize) {
    assert!(
        budget > 0,
        "Fixed schema depth exceeds MAX_DEPTH; consider max-depth-16"
    );
    assert!(width > 0, "Fixed fields must have positive width");
    match layout {
        Layout::Flat { .. } => panic!("recursive CTFE descriptor expected"),
        Layout::Leaf(leaf) => {
            let actual = match *leaf {
                blubber_core::Leaf::Unsigned(size) | blubber_core::Leaf::Signed(size) => {
                    assert!(
                        matches!(size, 1 | 2 | 4 | 8 | 16),
                        "invalid fixed integer width"
                    );
                    size as u32
                }
                blubber_core::Leaf::Bool => 1,
                blubber_core::Leaf::Raw(size) => size,
            };
            assert!(
                actual == width,
                "Fixed descriptor width differs from codec width"
            );
        }
        Layout::Struct {
            fixed_bytes,
            fixed,
            variable,
        } => {
            assert!(
                variable.is_empty(),
                "Fixed fields require fixed-kind descriptors"
            );
            assert!(
                !fixed.is_empty() && fixed.len() <= u16::MAX as usize,
                "invalid fixed field count"
            );
            assert!(
                *fixed_bytes == width,
                "Fixed descriptor width differs from codec width"
            );
            let mut index = 0;
            let mut offset = 0u32;
            while index < fixed.len() {
                let field = &fixed[index];
                assert!(
                    field.declaration as usize == index && field.offset == offset,
                    "Fixed descriptor fields must follow packed declaration order"
                );
                let child_width = match field.layout {
                    Layout::Leaf(blubber_core::Leaf::Unsigned(size))
                    | Layout::Leaf(blubber_core::Leaf::Signed(size)) => *size as u32,
                    Layout::Leaf(blubber_core::Leaf::Bool) => 1,
                    Layout::Leaf(blubber_core::Leaf::Raw(size)) => *size,
                    Layout::Struct { fixed_bytes, .. } => *fixed_bytes,
                    _ => panic!("Fixed fields require fixed-kind descriptors"),
                };
                assert_fixed_layout(field.layout, child_width, budget - 1);
                offset = match offset.checked_add(child_width) {
                    Some(end) => end,
                    None => panic!("Fixed wire width exceeds u32"),
                };
                index += 1;
            }
            assert!(
                offset == width,
                "Fixed descriptor fields do not cover the codec width"
            );
        }
        _ => panic!("Fixed fields require fixed-kind descriptors"),
    }
}

#[cfg(kani)]
#[doc(hidden)]
pub use crate::readers::kani_batched_text;

#[cfg(kani)]
#[doc(hidden)]
pub use crate::readers::{kani_batched_list_state, kani_batched_map_state};

/// Creates a system account for a generated initializer.
#[cfg(feature = "pinocchio")]
pub use crate::pinocchio::creation::create;
