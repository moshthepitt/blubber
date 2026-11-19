//! Backend protocol for the streamed collection byte kernel.

use crate::storage::Backend as Resize;
use crate::{ErrorKind, Result, View};
use blubber_core::raw::{InlinePath, Plan, RewritePlan, RewriteStream};
use blubber_core::Layout;

use crate::edit::{Preparation, PreparationError};

trait MeasureFault {
    fn report_measure(self, lazy: bool, poisoned: &mut bool) -> crate::Error;
}
impl MeasureFault for crate::Error {
    #[inline(always)]
    fn report_measure(self, _lazy: bool, _poisoned: &mut bool) -> crate::Error {
        self
    }
}
impl MeasureFault for PreparationError {
    #[inline(always)]
    fn report_measure(self, lazy: bool, poisoned: &mut bool) -> crate::Error {
        self.report(lazy, poisoned)
    }
}

// Classify from descriptor metadata; source bytes are admitted separately.
#[inline(always)]
pub(crate) fn is_fixed_map(layout: Layout) -> bool {
    match layout {
        Layout::Flat { checked } => {
            checked.header(0) == 8
                && crate::view::width_for_region(&Layout::Flat {
                    checked: checked.child(0, true),
                })
                .is_some()
        }
        _ => matches!(layout.head(), blubber_core::LayoutHead::Map { value, .. }
            if crate::view::width_for_region(&value).is_some()),
    }
}

// A source check precedes user measurement. Validated ingress may skip scanning
// directories/key domains. Lazy fixed maps admit geometry without source keys.
// Match the typed fixed/indexed shape here so the plan needs no repeated
// descriptor dispatch, and a mismatched lazy view cannot reuse key evidence.
#[inline(always)]
pub(crate) fn source(view: View<'_>, admitted: bool, map: bool, fixed_map: bool) -> Result<u32> {
    if admitted {
        if view.layout().tag() != if map { 8 } else { 7 } {
            return Err(ErrorKind::InvalidSchema.into());
        }
        if map && is_fixed_map(view.layout()) {
            if !fixed_map {
                return Err(ErrorKind::InvalidSchema.into());
            }
            return Ok(view.map_header()?.count);
        }
        if fixed_map {
            return Err(ErrorKind::InvalidSchema.into());
        }
        return view.word(0);
    }
    let (count, total, actual_fixed_map) = if map {
        let h = view.map_header()?;
        (
            h.count,
            match h.width {
                Some(width) => crate::schema::mul(h.count, width)?,
                None => view.bytes().len() as u32 - h.payload,
            },
            h.width.is_some(),
        )
    } else {
        let h = view.list_header()?;
        (
            h.count,
            match h.width {
                Some(width) => crate::schema::mul(h.count, width)?,
                None => view.bytes().len() as u32 - h.payload,
            },
            false,
        )
    };
    view.relative(RewritePlan::check(
        view.bytes(),
        &view.layout(),
        count,
        total,
    ))?;
    if actual_fixed_map != fixed_map {
        return Err(ErrorKind::InvalidSchema.into());
    }
    Ok(count)
}

#[inline(always)]
pub(crate) fn plan(view: View<'_>, count: u32, total: u32, fixed_map: bool) -> Result<RewritePlan> {
    if fixed_map {
        return view.relative(RewritePlan::check_fixed_map(
            view.bytes(),
            &view.layout(),
            count,
            total,
        ));
    }
    view.relative(RewritePlan::check_admitted(
        view.bytes(),
        &view.layout(),
        count,
        total,
    ))
}

/// Checks the complete typed fill preflight without encoding or reserving.
/// `admitted` is exclusive validation evidence, or lazy fixed-map admission.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_fill<T: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    map: bool,
    key_width: u32,
    value: &T::Init<'_>,
) -> Result<(RewritePlan, (u32, u32))> {
    prepare_fill_policy::<T>(view, admitted, map, key_width, value)
        .map_err(PreparationError::into_error)
}

#[inline(always)]
pub(crate) fn prepare_fill_policy<T: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    map: bool,
    key_width: u32,
    value: &T::Init<'_>,
) -> Preparation<(RewritePlan, (u32, u32))> {
    let fixed_map = map && T::FIXED_SIZE.is_some();
    if let Some(width) = T::FIXED_SIZE {
        if view.layout().tag() != if map { 8 } else { 7 } {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let count = view.word(0)?;
        let total = crate::view::count_try!(view, crate::schema::mul(count, width));
        let checked = if admitted && map {
            RewritePlan::check_fixed_map(view.bytes(), &view.layout(), count, total)
        } else {
            RewritePlan::check(view.bytes(), &view.layout(), count, total)
        };
        let plan = crate::view::count_try!(view, view.relative(checked)).typed(
            Some(width),
            key_width,
            map,
        )?;
        let len = if count == 0 {
            0
        } else {
            T::measure(value).map_err(PreparationError::input)?
        };
        if count != 0 && len != width {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        return Ok((plan, (count, len)));
    }
    let count = source(view, admitted, map, fixed_map)?;
    let len = if count == 0 {
        0
    } else {
        T::measure(value).map_err(PreparationError::input)?
    };
    let total = crate::schema::mul(count, len)?;
    Ok((
        plan(view, count, total, fixed_map)?.typed(T::FIXED_SIZE, key_width, map)?,
        (count, len),
    ))
}

/// Checks an exact-count list rewrite, measuring each input once.
/// `admitted` is exclusive validation evidence for the exact source view.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_list<T: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    values: &[T::Init<'_>],
) -> Result<(RewritePlan, ())> {
    prepare_list_policy::<T>(view, admitted, values).map_err(PreparationError::into_error)
}

#[inline(always)]
pub(crate) fn prepare_list_policy<T: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    values: &[T::Init<'_>],
) -> Preparation<(RewritePlan, ())> {
    prepare_values::<T>(view, admitted, false, 0, values)
}

/// Checks an exact-count map rewrite without collecting or comparing input keys.
/// `admitted` is exclusive validation evidence, or lazy fixed-map admission.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_map_values<K: crate::MapKey, V: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    values: &[V::Init<'_>],
) -> Result<(RewritePlan, ())> {
    prepare_map_values_policy::<K, V>(view, admitted, values).map_err(PreparationError::into_error)
}

#[inline(always)]
pub(crate) fn prepare_map_values_policy<K: crate::MapKey, V: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    values: &[V::Init<'_>],
) -> Preparation<(RewritePlan, ())> {
    prepare_values::<V>(view, admitted, true, K::SIZE, values)
}

// Lists and keyless maps share count rejection and measurement. The plan carries
// the map stride; the existing stream keeps keys outside every value slot.
#[inline(always)]
fn prepare_values<T: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    map: bool,
    key_width: u32,
    values: &[T::Init<'_>],
) -> Preparation<(RewritePlan, ())> {
    let fixed_map = map && T::FIXED_SIZE.is_some();
    let count = source(view, admitted, map, fixed_map)?;
    if values.len() != count as usize {
        return Err(ErrorKind::InvalidBoundary.into());
    }
    let mut total = 0;
    for value in values {
        total =
            crate::schema::checked_add(total, T::measure(value).map_err(PreparationError::input)?)?;
    }
    Ok((
        plan(view, count, total, fixed_map)?.typed(T::FIXED_SIZE, key_width, map)?,
        (),
    ))
}

/// Checks count, every supplied canonical map key and every value once.
/// `admitted` is exclusive validation evidence, or lazy fixed-map admission.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_map<K: crate::MapKey, V: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    entries: &[(K, V::Init<'_>)],
) -> Result<(RewritePlan, ())> {
    prepare_map_policy::<K, V>(view, admitted, entries).map_err(PreparationError::into_error)
}

#[inline(always)]
pub(crate) fn prepare_map_policy<K: crate::MapKey, V: crate::Schema>(
    view: View<'_>,
    admitted: bool,
    entries: &[(K, V::Init<'_>)],
) -> Preparation<(RewritePlan, ())> {
    let fixed_map = V::FIXED_SIZE.is_some();
    let count = source(view, admitted, true, fixed_map)?;
    if entries.len() != count as usize {
        return Err(ErrorKind::InvalidBoundary.into());
    }
    let header = view.map_header()?;
    let mut total = 0;
    if count == 0 {
        return Ok((
            plan(view, count, 0, fixed_map)?.typed(V::FIXED_SIZE, K::SIZE, true)?,
            (),
        ));
    }
    // Header admission checked the full directory/entry extent. Advancing exact
    // chunks retains that geometry instead of rebuilding each key coordinate.
    let extent = 4 + count as usize * header.stride as usize;
    for ((key, value), entry) in entries
        .iter()
        .zip(view.bytes()[4..extent].chunks_exact(header.stride as usize))
    {
        let stored = &entry[..header.key_width as usize];
        if key.encode_key().as_ref() != stored {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        total =
            crate::schema::checked_add(total, V::measure(value).map_err(PreparationError::input)?)?;
    }
    Ok((
        plan(view, count, total, fixed_map)?.typed(V::FIXED_SIZE, K::SIZE, true)?,
        (),
    ))
}

/// Encodes one measured input into its checked final slot and checks its count.
///
/// The owning rewrite protocol must retain poison until the complete stream and
/// enclosing publication succeed, including when this codec returns or unwinds.
#[doc(hidden)]
#[inline(always)]
pub fn encode_rewrite_slot<T: crate::Schema>(
    value: &T::Init<'_>,
    len: u32,
    stream: &mut RewriteStream<'_>,
) -> Result<()> {
    stream.prepare_slot(len)?;
    let written = T::encode_slot(value, stream.destination())?;
    stream.advance(written)
}

pub(crate) trait FillLayout {
    const KEY: u32;
    const FIXED_MAP: bool;
}
impl<T: crate::Schema> FillLayout for crate::List<T> {
    const KEY: u32 = 0;
    const FIXED_MAP: bool = false;
}
impl<K: crate::MapKey, V: crate::Schema> FillLayout for crate::Map<K, V> {
    const KEY: u32 = K::SIZE;
    const FIXED_MAP: bool = V::FIXED_SIZE.is_some();
}
struct FillEncoder<'a, 'b, T: crate::Schema, C>(&'a T::Init<'b>, core::marker::PhantomData<C>);

impl<T: crate::Schema, C: FillLayout> blubber_core::fill::ValueEncoder
    for FillEncoder<'_, '_, T, C>
{
    #[inline(always)]
    fn geometry(&self) -> (u32, u32) {
        (T::FIXED_SIZE.unwrap_or(0), C::KEY)
    }
    #[inline(always)]
    fn encode(&self, slot: &mut [u8]) -> Result<usize> {
        T::encode_slot(self.0, slot)
    }
}

#[inline(always)]
pub(crate) fn encode_fixed_fill<T: crate::Schema, C: FillLayout>(
    value: &T::Init<'_>,
    bytes: &mut [u8],
) -> Result<()> {
    blubber_core::fill::values(
        bytes,
        &FillEncoder::<T, C>(value, core::marker::PhantomData),
    )
}

#[inline(always)]
pub(crate) fn encode_fill<T: crate::Schema, C: FillLayout>(
    value: &T::Init<'_>,
    count: u32,
    len: u32,
    key: u32,
    stream: &mut RewriteStream<'_>,
) -> Result<()> {
    if T::FIXED_SIZE.is_none() && count > 1 {
        return stream.fill_uniform(
            len,
            key,
            &FillEncoder::<T, C>(value, core::marker::PhantomData),
        );
    }
    if !stream.fill_fixed(
        T::FIXED_SIZE,
        key,
        &FillEncoder::<T, C>(value, core::marker::PhantomData),
    )? {
        for _ in 0..count {
            encode_rewrite_slot::<T>(value, len, stream)?;
        }
    }
    Ok(())
}

struct Detached<I> {
    outer: Plan<'static>,
    collection: RewritePlan,
    input: I,
}

/// Runs a checked, count-preserving collection rewrite under one backend loan.
///
/// This is implementation support for typed wrappers. `measure` checks the
/// selected collection and every input before returning its detached plan.
/// `encode` must encode and advance each exact stream slot once. The operation
/// checks completion itself and retains poison on encoding error or unwind.
/// Input cannot borrow the destination through the supplied view.
///
/// Preflight failures preserve bytes and logical used length. One reserve may
/// precede failed readiness; backend allocation/re-lease failure has the
/// backend's documented preservation or terminal-close behavior.
#[doc(hidden)]
#[inline(always)]
pub(crate) fn rewrite_collection_policy<I>(
    store: &mut dyn Resize,
    lazy: bool,
    layout: &Layout,
    path: &InlinePath,
    account: ([u8; 8], u32),
    measure: impl FnOnce(View<'_>) -> Preparation<(RewritePlan, I)>,
    encode: impl FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
) -> Result<()> {
    rewrite_collection_impl(store, lazy, layout, path, account, measure, encode)
}

#[inline(always)]
fn rewrite_collection_impl<I, F: MeasureFault>(
    store: &mut dyn Resize,
    lazy: bool,
    layout: &Layout,
    path: &InlinePath,
    account: ([u8; 8], u32),
    measure: impl FnOnce(View<'_>) -> core::result::Result<(RewritePlan, I), F>,
    encode: impl FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
) -> Result<()> {
    if store.is_closed() {
        return Err(ErrorKind::Closed.into());
    }
    if store.is_poisoned() {
        return Err(ErrorKind::Poisoned.into());
    }
    let mut encode = Some(encode);
    let mut measure = Some(measure);
    let mut preflight = None;
    if try_tail(
        store.edit_access(),
        lazy,
        layout,
        path,
        account,
        &mut measure,
        &mut preflight,
        &mut encode,
    )? {
        return Ok(());
    }

    if let Some(detached) = stage(
        store.edit_access(),
        lazy,
        layout,
        path,
        account,
        measure,
        preflight,
        &mut encode,
    )? {
        store.resize(detached.outer.required_storage_len() as usize)?;
        complete_detached(store, detached, encode.take().expect("one rewrite encoder"))?;
    }
    Ok(())
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn try_tail<
    I,
    F: MeasureFault,
    M: FnOnce(View<'_>) -> core::result::Result<(RewritePlan, I), F>,
    E: FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
>(
    access: crate::storage::private::EditAccess<'_>,
    lazy: bool,
    layout: &Layout,
    path: &InlinePath,
    (discriminator, minimum): ([u8; 8], u32),
    measure: &mut Option<M>,
    preflight: &mut Option<(RewritePlan, I)>,
    encode: &mut Option<E>,
) -> Result<bool> {
    if let Some(tail) = crate::edit::list_pass::source_try!(
        lazy,
        blubber_core::raw::resolve_rewrite_tail(access.bytes, discriminator, minimum, layout, path),
        access.poisoned,
    ) {
        let (span, selected) = tail.selection();
        let view = View::inherited_for_edit(
            &access.bytes[span.start as usize..span.end as usize],
            selected,
            span.start,
        )?;
        let (collection, input) = measure.take().expect("one rewrite measurement")(view)
            .map_err(|error| error.report_measure(lazy, access.poisoned))?;
        if collection.is_empty() {
            return Ok(true);
        }
        if let Some(mut write) = crate::edit::list_pass::source_try!(
            lazy,
            tail.prepare(access.bytes, collection.encoded_len()),
            access.poisoned,
        ) {
            check_limits(
                write.growth_len().unwrap_or(span.end) as usize,
                access.hard_limit,
                access.limit,
                access.limit_error,
            )?;
            #[cfg(feature = "pinocchio")]
            access
                .preflight
                .check(write.growth_len().unwrap_or(0) as usize)?;
            *access.poisoned = true;
            encode_stream(
                write.destination(),
                collection,
                input,
                encode.take().expect("one rewrite encoder"),
            )?;
            write.finish();
            *access.poisoned = false;
            return Ok(true);
        }
        *preflight = Some((collection, input));
    }
    Ok(false)
}

#[cfg_attr(target_os = "solana", inline(never))]
#[allow(clippy::too_many_arguments)]
fn stage<I, F: MeasureFault, E: FnOnce(I, &mut RewriteStream<'_>) -> Result<()>>(
    access: crate::storage::private::EditAccess<'_>,
    lazy: bool,
    layout: &Layout,
    path: &InlinePath,
    (discriminator, minimum): ([u8; 8], u32),
    mut measure: Option<impl FnOnce(View<'_>) -> core::result::Result<(RewritePlan, I), F>>,
    preflight: Option<(RewritePlan, I)>,
    encode: &mut Option<E>,
) -> Result<Option<Detached<I>>> {
    let resolved = crate::edit::list_pass::source_try!(
        lazy,
        blubber_core::raw::resolve_account_edit(access.bytes, discriminator, minimum, layout, path),
        access.poisoned,
    );
    let (span, selected) = resolved.selection();
    let view = View::inherited_for_edit(
        &resolved.inspect_resolved()[span.start as usize..span.end as usize],
        selected,
        span.start,
    )?;
    let (collection, input) = match preflight {
        Some(pair) => pair,
        None => measure.take().expect("one rewrite measurement")(view)
            .map_err(|error| error.report_measure(lazy, access.poisoned))?,
    };
    if collection.is_empty() {
        return Ok(None);
    }
    let len = collection.encoded_len();
    if resolved.subtree_fits(len) {
        let mut write = crate::edit::list_pass::source_try!(
            lazy,
            resolved.prepare_subtree(len),
            access.poisoned,
        );
        check_limits(
            write.required_len() as usize,
            access.hard_limit,
            access.limit,
            access.limit_error,
        )?;
        #[cfg(feature = "pinocchio")]
        if len > span.end - span.start {
            access.preflight.check(write.required_len() as usize)?;
        }
        *access.poisoned = true;
        encode_stream(
            write.destination(),
            collection,
            input,
            encode.take().expect("one rewrite encoder"),
        )?;
        write.finish();
        *access.poisoned = false;
        Ok(None)
    } else {
        let prepared = crate::edit::list_pass::source_try!(
            lazy,
            resolved.prepare_replace(len, u32::MAX),
            access.poisoned,
        );
        check_limits(
            prepared.prepared_len() as usize,
            access.hard_limit,
            access.limit,
            access.limit_error,
        )?;
        #[cfg(feature = "pinocchio")]
        if prepared.prepared_len() > blubber_core::read_u32(prepared.inspect_prepared(), 8)? {
            access.preflight.check(prepared.prepared_len() as usize)?;
        }
        Ok(Some(Detached {
            outer: prepared.detach(),
            collection,
            input,
        }))
    }
}

fn check_limits(required: usize, hard: usize, limit: usize, limit_error: ErrorKind) -> Result<()> {
    if required > hard {
        Err(ErrorKind::Capacity.into())
    } else if required > limit {
        Err(limit_error.into())
    } else {
        Ok(())
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn complete_detached<I>(
    store: &mut dyn Resize,
    detached: Detached<I>,
    encode: impl FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
) -> Result<()> {
    let access = store.edit_access();
    let mut apply = blubber_core::raw::begin_apply(access.bytes, detached.outer)?;
    *access.poisoned = true;
    apply.move_regions();
    encode_stream(
        apply.destination(),
        detached.collection,
        detached.input,
        encode,
    )?;
    apply.finish();
    *access.poisoned = false;
    Ok(())
}

#[inline(always)]
fn encode_stream<I>(
    bytes: &mut [u8],
    plan: RewritePlan,
    input: I,
    encode: impl FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
) -> Result<()> {
    let mut stream = plan.begin(bytes)?;
    encode(input, &mut stream)?;
    stream.finish()
}

/// Runs a checked raw collection rewrite with the original pre-write error frame.
#[doc(hidden)]
#[inline(always)]
pub fn rewrite_collection<I>(
    store: &mut dyn Resize,
    layout: &Layout,
    path: &InlinePath,
    account: ([u8; 8], u32),
    measure: impl FnOnce(View<'_>) -> Result<(RewritePlan, I)>,
    encode: impl FnOnce(I, &mut RewriteStream<'_>) -> Result<()>,
) -> Result<()> {
    rewrite_collection_impl(store, false, layout, path, account, measure, encode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_map_type_mismatch_rejects_before_measurement() {
        static BYTE: Layout = Layout::Leaf(blubber_core::Leaf::Bool);
        let key = blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Raw,
        };
        for (bytes, value, fixed) in [
            (&[1, 0, 0, 0, 9, 1][..], &BYTE, true),
            (&[1, 0, 0, 0, 9, 1, 0, 0, 0, b'a'][..], &Layout::Str, false),
        ] {
            let view = View::new(bytes, Layout::Map { key, value }, 0).unwrap();
            for admitted in [false, true] {
                assert_eq!(
                    source(view, admitted, true, !fixed)
                        .and_then(|_| { panic!("measurement reached mismatched value geometry") }),
                    Err::<(), _>(ErrorKind::InvalidSchema.into()),
                );
                assert_eq!(source(view, admitted, true, fixed), Ok(1));
            }
        }
    }

    #[test]
    fn capacity_limits_reject_before_any_encoder_or_poison() {
        for (hard, limit, kind) in [
            (25, 32, ErrorKind::Capacity),
            (32, 25, ErrorKind::GrowthLimit),
            (25, 25, ErrorKind::Capacity),
        ] {
            let mut bytes = [0xa5u8; 32];
            bytes[..8].copy_from_slice(b"rewrite8");
            bytes[8..12].copy_from_slice(&22u32.to_le_bytes());
            bytes[12..22].copy_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b']);
            let before = bytes;
            let mut poisoned = false;
            let access = crate::storage::private::EditAccess {
                bytes: &mut bytes,
                poisoned: &mut poisoned,
                hard_limit: hard,
                limit,
                limit_error: ErrorKind::GrowthLimit,
                #[cfg(feature = "pinocchio")]
                preflight: crate::storage::private::Preflight::default(),
            };
            let layout = Layout::List {
                element: &Layout::Str,
            };
            let mut encode = Some(|(), _: &mut RewriteStream<'_>| -> Result<()> {
                panic!("encoder ran before limit checks")
            });
            let result = stage(
                access,
                false,
                &layout,
                &InlinePath::new(),
                (*b"rewrite8", 4),
                Some(|view: View<'_>| {
                    Ok::<_, PreparationError>((
                        RewritePlan::check(view.bytes(), &view.layout(), 2, 6)?,
                        (),
                    ))
                }),
                None,
                &mut encode,
            );
            assert!(matches!(result,Err(error) if error==kind.into()));
            assert_eq!(bytes, before);
            assert!(!poisoned);
        }
    }
    #[test]
    fn direct_tail_limits_reject_before_encoding_or_poison() {
        static LIST: Layout = Layout::List {
            element: &Layout::Str,
        };
        static ROOT: Layout = Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[blubber_core::VariableField {
                declaration: 0,
                layout: &LIST,
            }],
        };
        for (hard, limit, kind) in [
            (25, 32, ErrorKind::Capacity),
            (32, 25, ErrorKind::GrowthLimit),
            (25, 25, ErrorKind::Capacity),
        ] {
            let mut bytes = [0xa5u8; 32];
            bytes[..8].copy_from_slice(b"rewrite8");
            bytes[8..12].copy_from_slice(&22u32.to_le_bytes());
            bytes[12..22].copy_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b']);
            let before = bytes;
            let mut poisoned = false;
            let access = crate::storage::private::EditAccess {
                bytes: &mut bytes,
                poisoned: &mut poisoned,
                hard_limit: hard,
                limit,
                limit_error: ErrorKind::GrowthLimit,
                #[cfg(feature = "pinocchio")]
                preflight: crate::storage::private::Preflight::default(),
            };
            let mut measure = Some(|view: View<'_>| {
                Ok::<_, PreparationError>((
                    RewritePlan::check(view.bytes(), &view.layout(), 2, 6)?,
                    (),
                ))
            });
            let mut encode = Some(|(), _: &mut RewriteStream<'_>| -> Result<()> {
                panic!("encoder ran before limit checks")
            });
            let result = try_tail(
                access,
                false,
                &ROOT,
                &InlinePath::root_field(0),
                (*b"rewrite8", 4),
                &mut measure,
                &mut None,
                &mut encode,
            );
            assert_eq!(result, Err(kind.into()));
            assert_eq!(bytes, before);
            assert!(!poisoned);
        }
    }
}
