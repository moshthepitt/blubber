//! Per-operation SBF stack frame screen for public Blubber APIs.

use super::*;
use blubber::{
    pinocchio::{Funding, Headroom, ReadStore, Store},
    Entry, List, ListMut, ListRef, Map, MapMut, MapRef, OptionMut, Result, ScalarMut, Str, StrMut,
};
use core::hint::black_box;

#[blubber::schema(account)]
pub struct ScreenAccount {
    pub authority: Address,
    pub counter: u64,
    pub text: Str,
    pub opt: Option<u64>,
    pub fixed_list: List<u64>,
    pub indexed_list: List<Str>,
    pub fixed_map: Map<Address, u64>,
    pub indexed_map: Map<Address, Str>,
}

const KEY: Address = Address::new_from_array([7; 32]);

macro_rules! op_fn {
    ($name:ident ( $($arg:ident : $ty:ty),* ) -> $ret:ty $body:block) => {
        #[inline(never)]
        fn $name($($arg : $ty),*) -> $ret $body
    };
}

// --- Harness baselines (expected ~0 B caller overhead) ---

op_fn!(op_00_noop_store(store: &mut Store<'_>) -> Result<()> {
    black_box(store);
    Ok(())
});
op_fn!(op_00_noop_root_mut(root: &mut ScreenAccountMut<'_>) -> Result<()> {
    black_box(root);
    Ok(())
});
op_fn!(op_00_noop_root_ref(root: &ScreenAccountRef<'_>) -> Result<()> {
    black_box(root);
    Ok(())
});
op_fn!(op_00_noop_list_fixed(list: &mut ListMut<'_, u64>) -> Result<()> {
    black_box(list);
    Ok(())
});
op_fn!(op_00_noop_list_indexed(list: &mut ListMut<'_, Str>) -> Result<()> {
    black_box(list);
    Ok(())
});
op_fn!(op_00_noop_map_fixed(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map);
    Ok(())
});
op_fn!(op_00_noop_map_indexed(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map);
    Ok(())
});

// --- Store & Account lifecycle ---

op_fn!(op_store_new(account: &mut AccountView, id: &Address) -> Result<()> {
    let store = Store::new(account, id, Funding::Prepaid)?;
    black_box(&store);
    Ok(())
});
op_fn!(op_store_new_max(account: &mut AccountView, id: &Address) -> Result<()> {
    let store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
    black_box(&store);
    Ok(())
});
op_fn!(op_read_store_new(account: &AccountView, id: &Address) -> Result<()> {
    let store = ReadStore::new(account, id)?;
    black_box(&store);
    Ok(())
});
op_fn!(op_account_load_trusted(bytes: &[u8]) -> Result<()> {
    let root = ScreenAccount::load_trusted(bytes)?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_load(bytes: &[u8]) -> Result<()> {
    let root = ScreenAccount::load(bytes)?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_load_trusted_mut(store: &mut Store<'_>) -> Result<()> {
    let root = ScreenAccount::load_trusted_mut(store)?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_load_checked_mut(store: &mut Store<'_>) -> Result<()> {
    let root = ScreenAccount::load_checked_mut(store)?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_load_mut(store: &mut Store<'_>) -> Result<()> {
    let root = ScreenAccount::load_mut(store)?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_initialize(store: &mut Store<'_>, id: &Address) -> Result<()> {
    let root = ScreenAccount::initialize(
        store,
        ScreenAccountInit {
            authority: *id,
            counter: 1,
            text: "seed",
            opt: None,
            fixed_list: &[1, 2],
            indexed_list: &["a", "b"],
            fixed_map: &[(KEY, 1)],
            indexed_map: &[(KEY, "a")],
        },
    )?;
    black_box(&root);
    Ok(())
});
op_fn!(op_account_commit(root: ScreenAccountMut<'_>) -> Result<()> {
    let info = root.commit()?;
    black_box(info);
    Ok(())
});
op_fn!(op_account_validate(root: &ScreenAccountMut<'_>) -> Result<()> {
    root.validate()
});
op_fn!(op_account_with_read(account: &AccountView, id: &Address) -> Result<()> {
    ScreenAccount::with_read(account, id, |root| {
        black_box(&root);
        Ok(())
    })
});
op_fn!(op_account_with_edit(account: &mut AccountView, id: &Address) -> Result<()> {
    let res = ScreenAccount::with_edit(account, id, Funding::Prepaid, |root| {
        black_box(root);
        Ok(())
    })?;
    black_box(res);
    Ok(())
});

// --- Scalar / Str / Option ---

op_fn!(op_scalar_try_get(s: &mut ScalarMut<'_, u64>) -> Result<()> {
    black_box(s.try_get()?);
    Ok(())
});
op_fn!(op_scalar_set(s: &mut ScalarMut<'_, u64>) -> Result<()> {
    s.set(42)
});
op_fn!(op_str_try_get(s: &mut StrMut<'_>) -> Result<()> {
    black_box(s.try_get()?);
    Ok(())
});
op_fn!(op_str_set(s: &mut StrMut<'_>) -> Result<()> {
    s.set("replacement")
});
op_fn!(op_str_push_str(s: &mut StrMut<'_>) -> Result<()> {
    s.push_str("suffix")
});
op_fn!(op_str_truncate(s: &mut StrMut<'_>) -> Result<()> {
    s.truncate(1)
});
op_fn!(op_str_clear(s: &mut StrMut<'_>) -> Result<()> {
    s.clear()
});
op_fn!(op_option_try_get(o: &mut OptionMut<'_, u64>) -> Result<()> {
    black_box(o.try_get()?);
    Ok(())
});
op_fn!(op_option_set_some(o: &mut OptionMut<'_, u64>) -> Result<()> {
    o.set(Some(7))
});
op_fn!(op_option_set_none(o: &mut OptionMut<'_, u64>) -> Result<()> {
    o.set(None)
});
op_fn!(op_option_get_mut(o: OptionMut<'_, u64>) -> Result<()> {
    if let Some(mut v) = o.get_mut()? {
        v.set(9)?;
    }
    Ok(())
});
op_fn!(op_option_some_mut(o: OptionMut<'_, u64>) -> Result<()> {
    o.some_mut()?.set(9)
});

// --- ListRef read APIs ---

op_fn!(op_list_ref_fixed_try_len(list: ListRef<'_, u64>) -> Result<()> {
    black_box(list.try_len()?);
    Ok(())
});
op_fn!(op_list_ref_fixed_try_get(list: ListRef<'_, u64>) -> Result<()> {
    black_box(list.try_get(0)?);
    Ok(())
});
op_fn!(op_list_ref_fixed_try_iter(list: ListRef<'_, u64>) -> Result<()> {
    for item in list.try_iter() {
        black_box(item?);
    }
    Ok(())
});
op_fn!(op_list_ref_fixed_validate(list: ListRef<'_, u64>) -> Result<()> {
    list.validate()
});
op_fn!(op_list_ref_indexed_try_len(list: ListRef<'_, Str>) -> Result<()> {
    black_box(list.try_len()?);
    Ok(())
});
op_fn!(op_list_ref_indexed_try_get(list: ListRef<'_, Str>) -> Result<()> {
    black_box(list.try_get(0)?);
    Ok(())
});
op_fn!(op_list_ref_indexed_try_iter(list: ListRef<'_, Str>) -> Result<()> {
    for item in list.try_iter() {
        black_box(item?);
    }
    Ok(())
});
op_fn!(op_list_ref_indexed_try_concat_str(list: ListRef<'_, Str>) -> Result<()> {
    black_box(list.try_concat_str()?);
    Ok(())
});
op_fn!(op_list_ref_indexed_try_iter_batched(list: ListRef<'_, Str>) -> Result<()> {
    for item in list.try_iter_batched() {
        black_box(item?);
    }
    Ok(())
});
op_fn!(op_list_ref_indexed_validate(list: ListRef<'_, Str>) -> Result<()> {
    list.validate()
});

// --- ListMut<u64> mutate / bulk / traversal ---

op_fn!(op_list_fixed_push(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.push(99)
});
op_fn!(op_list_fixed_insert(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.insert(0, 99)
});
op_fn!(op_list_fixed_replace(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.replace(0, 99)
});
op_fn!(op_list_fixed_remove(list: &mut ListMut<'_, u64>) -> Result<()> {
    black_box(list.remove(0)?);
    Ok(())
});
op_fn!(op_list_fixed_pop(list: &mut ListMut<'_, u64>) -> Result<()> {
    black_box(list.pop()?);
    Ok(())
});
op_fn!(op_list_fixed_remove_into(list: &mut ListMut<'_, u64>) -> Result<()> {
    let mut out = [0u8; 8];
    black_box(list.remove_into(0, &mut out)?);
    Ok(())
});
op_fn!(op_list_fixed_truncate(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.truncate(1)
});
op_fn!(op_list_fixed_clear(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.clear()
});
op_fn!(op_list_fixed_get_mut(list: &mut ListMut<'_, u64>) -> Result<()> {
    if let Some(mut v) = list.reborrow().get_mut(0)? {
        v.set(7)?;
    }
    Ok(())
});
op_fn!(op_list_fixed_at_mut(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.reborrow().at_mut(0)?.set(7)
});
op_fn!(op_list_fixed_with_mut(list: &mut ListMut<'_, u64>) -> Result<()> {
    black_box(list.with_mut(0, |v| v.set(7))?);
    Ok(())
});
op_fn!(op_list_fixed_cursor_mut(list: &mut ListMut<'_, u64>) -> Result<()> {
    let mut c = list.reborrow().cursor_mut()?;
    while let Some(mut v) = c.next()? {
        v.set(7)?;
    }
    Ok(())
});
op_fn!(op_list_fixed_try_for_each_mut(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.try_for_each_mut(|v| v.set(7))
});
op_fn!(op_list_fixed_extend_from_slice(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.extend_from_slice(&[1, 2])
});
op_fn!(op_list_fixed_insert_from_slice(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.insert_from_slice(0, &[1, 2])
});
op_fn!(op_list_fixed_fill(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.fill(7)
});
op_fn!(op_list_fixed_rewrite_from(list: &mut ListMut<'_, u64>) -> Result<()> {
    list.rewrite_from(&[1, 2])
});

// --- ListMut<Str> mutate / bulk / traversal ---

op_fn!(op_list_indexed_push(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.push("three")
});
op_fn!(op_list_indexed_insert(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.insert(0, "zero")
});
op_fn!(op_list_indexed_replace(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.replace(0, "first")
});
op_fn!(op_list_indexed_remove(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.remove(0)
});
op_fn!(op_list_indexed_pop(list: &mut ListMut<'_, Str>) -> Result<()> {
    black_box(list.pop()?);
    Ok(())
});
op_fn!(op_list_indexed_remove_into(list: &mut ListMut<'_, Str>) -> Result<()> {
    let mut out = [0u8; 16];
    black_box(list.remove_into(0, &mut out)?);
    Ok(())
});
op_fn!(op_list_indexed_truncate(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.truncate(1)
});
op_fn!(op_list_indexed_clear(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.clear()
});
op_fn!(op_list_indexed_get_mut(list: &mut ListMut<'_, Str>) -> Result<()> {
    if let Some(mut v) = list.reborrow().get_mut(0)? {
        v.set("hi")?;
    }
    Ok(())
});
op_fn!(op_list_indexed_at_mut(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.reborrow().at_mut(0)?.set("hi")
});
op_fn!(op_list_indexed_with_mut(list: &mut ListMut<'_, Str>) -> Result<()> {
    black_box(list.with_mut(0, |v| v.set("hi"))?);
    Ok(())
});
op_fn!(op_list_indexed_cursor_mut(list: &mut ListMut<'_, Str>) -> Result<()> {
    let mut c = list.reborrow().cursor_mut()?;
    while let Some(mut v) = c.next()? {
        v.push_str("!")?;
    }
    Ok(())
});
op_fn!(op_list_indexed_try_for_each_mut(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.try_for_each_mut(|v| v.push_str("!"))
});
op_fn!(op_list_indexed_extend_from_slice(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.extend_from_slice(&["a", "b"])
});
op_fn!(op_list_indexed_insert_from_slice(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.insert_from_slice(0, &["a", "b"])
});
op_fn!(op_list_indexed_fill(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.fill("same")
});
op_fn!(op_list_indexed_rewrite_from(list: &mut ListMut<'_, Str>) -> Result<()> {
    list.rewrite_from(&["a", "b"])
});

// --- MapRef read APIs ---

op_fn!(op_map_ref_fixed_try_len(map: MapRef<'_, Address, u64>) -> Result<()> {
    black_box(map.try_len()?);
    Ok(())
});
op_fn!(op_map_ref_fixed_try_get(map: MapRef<'_, Address, u64>) -> Result<()> {
    black_box(map.try_get(&KEY)?);
    Ok(())
});
op_fn!(op_map_ref_fixed_try_contains_key(map: MapRef<'_, Address, u64>) -> Result<()> {
    black_box(map.try_contains_key(&KEY)?);
    Ok(())
});
op_fn!(op_map_ref_fixed_try_iter(map: MapRef<'_, Address, u64>) -> Result<()> {
    for item in map.try_iter() {
        black_box(item?);
    }
    Ok(())
});
op_fn!(op_map_ref_fixed_validate(map: MapRef<'_, Address, u64>) -> Result<()> {
    map.validate()
});
op_fn!(op_map_ref_indexed_try_len(map: MapRef<'_, Address, Str>) -> Result<()> {
    black_box(map.try_len()?);
    Ok(())
});
op_fn!(op_map_ref_indexed_try_get(map: MapRef<'_, Address, Str>) -> Result<()> {
    black_box(map.try_get(&KEY)?);
    Ok(())
});
op_fn!(op_map_ref_indexed_try_contains_key(map: MapRef<'_, Address, Str>) -> Result<()> {
    black_box(map.try_contains_key(&KEY)?);
    Ok(())
});
op_fn!(op_map_ref_indexed_try_iter(map: MapRef<'_, Address, Str>) -> Result<()> {
    for item in map.try_iter() {
        black_box(item?);
    }
    Ok(())
});
op_fn!(op_map_ref_indexed_validate(map: MapRef<'_, Address, Str>) -> Result<()> {
    map.validate()
});

// --- MapMut<Address, u64> mutate / entry / traversal / bulk ---

op_fn!(op_map_fixed_insert(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.insert(KEY, 42)?);
    Ok(())
});
op_fn!(op_map_fixed_remove(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.remove(&KEY)?);
    Ok(())
});
op_fn!(op_map_fixed_remove_into(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    let mut out = [0u8; 8];
    black_box(map.remove_into(&KEY, &mut out)?);
    Ok(())
});
op_fn!(op_map_fixed_clear(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.clear()
});
op_fn!(op_map_fixed_get_mut(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    if let Some(mut v) = map.get_mut(&KEY)? {
        v.set(7)?;
    }
    Ok(())
});
op_fn!(op_map_fixed_into_get_mut(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    if let Some(mut v) = map.reborrow().into_get_mut(&KEY)? {
        v.set(7)?;
    }
    Ok(())
});
op_fn!(op_map_fixed_at_mut(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.reborrow().at_mut(&KEY)?.set(7)
});
op_fn!(op_map_fixed_cursor_mut(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    let mut c = map.reborrow().cursor_mut()?;
    while let Some((k, mut v)) = c.next()? {
        black_box(k);
        v.set(7)?;
    }
    Ok(())
});
op_fn!(op_map_fixed_try_for_each_mut(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.try_for_each_mut(|k, v| {
        black_box(k);
        v.set(7)
    })
});
op_fn!(op_map_fixed_entry_or_insert(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.entry(KEY)?.or_insert(42)?);
    Ok(())
});
op_fn!(op_map_fixed_entry_or_insert_with(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.entry(KEY)?.or_insert_with(|| 42)?);
    Ok(())
});
op_fn!(op_map_fixed_entry_or_default(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.entry(KEY)?.or_default()?);
    Ok(())
});
op_fn!(op_map_fixed_entry_and_modify_or_insert(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    black_box(map.entry(KEY)?.and_modify(|v| v.set(1))?.or_insert_with(|| 2)?);
    Ok(())
});
op_fn!(op_map_fixed_occupied_insert(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    if let Entry::Occupied(e) = map.entry(KEY)? {
        black_box(e.insert(99)?);
    }
    Ok(())
});
op_fn!(op_map_fixed_occupied_remove(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    if let Entry::Occupied(e) = map.entry(KEY)? {
        black_box(e.remove()?);
    }
    Ok(())
});
op_fn!(op_map_fixed_vacant_insert(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    if let Entry::Vacant(e) = map.entry(KEY)? {
        black_box(e.insert(99)?);
    }
    Ok(())
});
op_fn!(op_map_fixed_fill_values(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.fill_values(7)
});
op_fn!(op_map_fixed_rewrite_values(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.rewrite_values(&[1, 2])
});
op_fn!(op_map_fixed_rewrite_values_from(map: &mut MapMut<'_, Address, u64>) -> Result<()> {
    map.rewrite_values_from(&[(KEY, 1)])
});

// --- MapMut<Address, Str> mutate / entry / traversal / bulk ---

op_fn!(op_map_indexed_insert(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.insert(KEY, "val")?);
    Ok(())
});
op_fn!(op_map_indexed_remove(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.remove(&KEY)?);
    Ok(())
});
op_fn!(op_map_indexed_remove_into(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    let mut out = [0u8; 16];
    black_box(map.remove_into(&KEY, &mut out)?);
    Ok(())
});
op_fn!(op_map_indexed_clear(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.clear()
});
op_fn!(op_map_indexed_get_mut(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    if let Some(mut v) = map.get_mut(&KEY)? {
        v.set("hi")?;
    }
    Ok(())
});
op_fn!(op_map_indexed_into_get_mut(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    if let Some(mut v) = map.reborrow().into_get_mut(&KEY)? {
        v.set("hi")?;
    }
    Ok(())
});
op_fn!(op_map_indexed_at_mut(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.reborrow().at_mut(&KEY)?.set("hi")
});
op_fn!(op_map_indexed_cursor_mut(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    let mut c = map.reborrow().cursor_mut()?;
    while let Some((k, mut v)) = c.next()? {
        black_box(k);
        v.push_str("!")?;
    }
    Ok(())
});
op_fn!(op_map_indexed_try_for_each_mut(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.try_for_each_mut(|k, v| {
        black_box(k);
        v.push_str("!")
    })
});
op_fn!(op_map_indexed_entry_or_insert(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.entry(KEY)?.or_insert("val")?);
    Ok(())
});
op_fn!(op_map_indexed_entry_or_insert_with(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.entry(KEY)?.or_insert_with(|| "val")?);
    Ok(())
});
op_fn!(op_map_indexed_entry_or_default(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.entry(KEY)?.or_default()?);
    Ok(())
});
op_fn!(op_map_indexed_entry_and_modify_or_insert(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    black_box(map.entry(KEY)?.and_modify(|v| v.set("x"))?.or_insert_with(|| "y")?);
    Ok(())
});
op_fn!(op_map_indexed_occupied_insert(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    if let Entry::Occupied(e) = map.entry(KEY)? {
        e.insert("val")?;
    }
    Ok(())
});
op_fn!(op_map_indexed_occupied_remove(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    if let Entry::Occupied(e) = map.entry(KEY)? {
        e.remove()?;
    }
    Ok(())
});
op_fn!(op_map_indexed_vacant_insert(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    if let Entry::Vacant(e) = map.entry(KEY)? {
        black_box(e.insert("val")?);
    }
    Ok(())
});
op_fn!(op_map_indexed_fill_values(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.fill_values("same")
});
op_fn!(op_map_indexed_rewrite_values(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.rewrite_values(&["a", "b"])
});
op_fn!(op_map_indexed_rewrite_values_from(map: &mut MapMut<'_, Address, Str>) -> Result<()> {
    map.rewrite_values_from(&[(KEY, "a")])
});

// --- Realistic chained handlers ---

op_fn!(op_chain_entry_fixed(store: &mut Store<'_>) -> Result<()> {
    let mut root = ScreenAccount::load_trusted_mut(store)?;
    root.fixed_map_mut().entry(KEY)?.or_insert(1)?.set(2)?;
    root.commit()?;
    Ok(())
});
op_fn!(op_chain_entry_indexed(store: &mut Store<'_>) -> Result<()> {
    let mut root = ScreenAccount::load_trusted_mut(store)?;
    root.indexed_map_mut()
        .entry(KEY)?
        .and_modify(|v| v.set("x"))?
        .or_insert_with(|| "y")?;
    root.commit()?;
    Ok(())
});
op_fn!(op_chain_list_and_map(store: &mut Store<'_>) -> Result<()> {
    let mut root = ScreenAccount::load_trusted_mut(store)?;
    root.fixed_list_mut().push(1)?;
    root.indexed_map_mut().insert(KEY, "z")?;
    root.commit()?;
    Ok(())
});

#[inline(never)]
pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let [op] = *ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    match op {
        0 => op_store_new(state, id)?,
        1 => op_store_new_max(state, id)?,
        2 => op_read_store_new(state, id)?,
        3 => {
            let store = ReadStore::new(state, id)?;
            op_account_load_trusted(store.bytes())?;
        }
        4 => {
            let store = ReadStore::new(state, id)?;
            op_account_load(store.bytes())?;
        }
        5 => {
            let store = ReadStore::new(state, id)?;
            let root = ScreenAccount::load_trusted(store.bytes())?;
            op_00_noop_root_ref(&root)?;
        }
        6 => op_account_with_read(state, id)?,
        7 => op_account_with_edit(state, id)?,
        8 => {
            let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::Max)?;
            match op {
                _ => op_00_noop_store(&mut store)?,
            }
        }
        9..=12 => {
            let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::Max)?;
            match op {
                9 => op_account_load_trusted_mut(&mut store)?,
                10 => op_account_load_checked_mut(&mut store)?,
                11 => op_account_load_mut(&mut store)?,
                _ => op_account_initialize(&mut store, id)?,
            }
        }
        13..=19 => {
            let store = ReadStore::new(state, id)?;
            let root = ScreenAccount::load_trusted(store.bytes())?;
            match op {
                13 => op_list_ref_fixed_try_len(root.fixed_list())?,
                14 => op_list_ref_fixed_try_get(root.fixed_list())?,
                15 => op_list_ref_fixed_try_iter(root.fixed_list())?,
                16 => op_list_ref_fixed_validate(root.fixed_list())?,
                17 => op_list_ref_indexed_try_len(root.indexed_list())?,
                18 => op_list_ref_indexed_try_get(root.indexed_list())?,
                _ => op_list_ref_indexed_try_iter(root.indexed_list())?,
            }
        }
        20..=27 => {
            let store = ReadStore::new(state, id)?;
            let root = ScreenAccount::load_trusted(store.bytes())?;
            match op {
                20 => op_list_ref_indexed_try_concat_str(root.indexed_list())?,
                21 => op_list_ref_indexed_try_iter_batched(root.indexed_list())?,
                22 => op_list_ref_indexed_validate(root.indexed_list())?,
                23 => op_map_ref_fixed_try_len(root.fixed_map())?,
                24 => op_map_ref_fixed_try_get(root.fixed_map())?,
                25 => op_map_ref_fixed_try_contains_key(root.fixed_map())?,
                26 => op_map_ref_fixed_try_iter(root.fixed_map())?,
                _ => op_map_ref_fixed_validate(root.fixed_map())?,
            }
        }
        28..=32 => {
            let store = ReadStore::new(state, id)?;
            let root = ScreenAccount::load_trusted(store.bytes())?;
            match op {
                28 => op_map_ref_indexed_try_len(root.indexed_map())?,
                29 => op_map_ref_indexed_try_get(root.indexed_map())?,
                30 => op_map_ref_indexed_try_contains_key(root.indexed_map())?,
                31 => op_map_ref_indexed_try_iter(root.indexed_map())?,
                _ => op_map_ref_indexed_validate(root.indexed_map())?,
            }
        }
        33..=160 => {
            let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::Max)?;
            if op == 150 {
                return op_chain_entry_fixed(&mut store).map_err(Into::into);
            }
            if op == 151 {
                return op_chain_entry_indexed(&mut store).map_err(Into::into);
            }
            if op == 152 {
                return op_chain_list_and_map(&mut store).map_err(Into::into);
            }
            let mut root = ScreenAccount::load_trusted_mut(&mut store)?;
            match op {
                33 => op_00_noop_root_mut(&mut root)?,
                34 => op_account_validate(&root)?,
                35 => return op_account_commit(root).map_err(Into::into),
                36 => op_scalar_try_get(&mut root.counter_mut())?,
                37 => op_scalar_set(&mut root.counter_mut())?,
                38 => op_str_try_get(&mut root.text_mut())?,
                39 => op_str_set(&mut root.text_mut())?,
                40 => op_str_push_str(&mut root.text_mut())?,
                41 => op_str_truncate(&mut root.text_mut())?,
                42 => op_str_clear(&mut root.text_mut())?,
                43 => op_option_try_get(&mut root.opt_mut())?,
                44 => op_option_set_some(&mut root.opt_mut())?,
                45 => op_option_set_none(&mut root.opt_mut())?,
                46 => op_option_get_mut(root.opt_mut())?,
                47 => op_option_some_mut(root.opt_mut())?,
                48 => op_00_noop_list_fixed(&mut root.fixed_list_mut())?,
                49 => op_list_fixed_push(&mut root.fixed_list_mut())?,
                50 => op_list_fixed_insert(&mut root.fixed_list_mut())?,
                51 => op_list_fixed_replace(&mut root.fixed_list_mut())?,
                52 => op_list_fixed_remove(&mut root.fixed_list_mut())?,
                53 => op_list_fixed_pop(&mut root.fixed_list_mut())?,
                54 => op_list_fixed_remove_into(&mut root.fixed_list_mut())?,
                55 => op_list_fixed_truncate(&mut root.fixed_list_mut())?,
                56 => op_list_fixed_clear(&mut root.fixed_list_mut())?,
                57 => op_list_fixed_get_mut(&mut root.fixed_list_mut())?,
                58 => op_list_fixed_at_mut(&mut root.fixed_list_mut())?,
                59 => op_list_fixed_with_mut(&mut root.fixed_list_mut())?,
                60 => op_list_fixed_cursor_mut(&mut root.fixed_list_mut())?,
                61 => op_list_fixed_try_for_each_mut(&mut root.fixed_list_mut())?,
                62 => op_list_fixed_extend_from_slice(&mut root.fixed_list_mut())?,
                63 => op_list_fixed_insert_from_slice(&mut root.fixed_list_mut())?,
                64 => op_list_fixed_fill(&mut root.fixed_list_mut())?,
                65 => op_list_fixed_rewrite_from(&mut root.fixed_list_mut())?,
                66 => op_00_noop_list_indexed(&mut root.indexed_list_mut())?,
                67 => op_list_indexed_push(&mut root.indexed_list_mut())?,
                68 => op_list_indexed_insert(&mut root.indexed_list_mut())?,
                69 => op_list_indexed_replace(&mut root.indexed_list_mut())?,
                70 => op_list_indexed_remove(&mut root.indexed_list_mut())?,
                71 => op_list_indexed_pop(&mut root.indexed_list_mut())?,
                72 => op_list_indexed_remove_into(&mut root.indexed_list_mut())?,
                73 => op_list_indexed_truncate(&mut root.indexed_list_mut())?,
                74 => op_list_indexed_clear(&mut root.indexed_list_mut())?,
                75 => op_list_indexed_get_mut(&mut root.indexed_list_mut())?,
                76 => op_list_indexed_at_mut(&mut root.indexed_list_mut())?,
                77 => op_list_indexed_with_mut(&mut root.indexed_list_mut())?,
                78 => op_list_indexed_cursor_mut(&mut root.indexed_list_mut())?,
                79 => op_list_indexed_try_for_each_mut(&mut root.indexed_list_mut())?,
                80 => op_list_indexed_extend_from_slice(&mut root.indexed_list_mut())?,
                81 => op_list_indexed_insert_from_slice(&mut root.indexed_list_mut())?,
                82 => op_list_indexed_fill(&mut root.indexed_list_mut())?,
                83 => op_list_indexed_rewrite_from(&mut root.indexed_list_mut())?,
                84 => op_00_noop_map_fixed(&mut root.fixed_map_mut())?,
                85 => op_map_fixed_insert(&mut root.fixed_map_mut())?,
                86 => op_map_fixed_remove(&mut root.fixed_map_mut())?,
                87 => op_map_fixed_remove_into(&mut root.fixed_map_mut())?,
                88 => op_map_fixed_clear(&mut root.fixed_map_mut())?,
                89 => op_map_fixed_get_mut(&mut root.fixed_map_mut())?,
                90 => op_map_fixed_into_get_mut(&mut root.fixed_map_mut())?,
                91 => op_map_fixed_at_mut(&mut root.fixed_map_mut())?,
                92 => op_map_fixed_cursor_mut(&mut root.fixed_map_mut())?,
                93 => op_map_fixed_try_for_each_mut(&mut root.fixed_map_mut())?,
                94 => op_map_fixed_entry_or_insert(&mut root.fixed_map_mut())?,
                95 => op_map_fixed_entry_or_insert_with(&mut root.fixed_map_mut())?,
                96 => op_map_fixed_entry_or_default(&mut root.fixed_map_mut())?,
                97 => op_map_fixed_entry_and_modify_or_insert(&mut root.fixed_map_mut())?,
                98 => op_map_fixed_occupied_insert(&mut root.fixed_map_mut())?,
                99 => op_map_fixed_occupied_remove(&mut root.fixed_map_mut())?,
                100 => op_map_fixed_vacant_insert(&mut root.fixed_map_mut())?,
                101 => op_map_fixed_fill_values(&mut root.fixed_map_mut())?,
                102 => op_map_fixed_rewrite_values(&mut root.fixed_map_mut())?,
                103 => op_map_fixed_rewrite_values_from(&mut root.fixed_map_mut())?,
                104 => op_00_noop_map_indexed(&mut root.indexed_map_mut())?,
                105 => op_map_indexed_insert(&mut root.indexed_map_mut())?,
                106 => op_map_indexed_remove(&mut root.indexed_map_mut())?,
                107 => op_map_indexed_remove_into(&mut root.indexed_map_mut())?,
                108 => op_map_indexed_clear(&mut root.indexed_map_mut())?,
                109 => op_map_indexed_get_mut(&mut root.indexed_map_mut())?,
                110 => op_map_indexed_into_get_mut(&mut root.indexed_map_mut())?,
                111 => op_map_indexed_at_mut(&mut root.indexed_map_mut())?,
                112 => op_map_indexed_cursor_mut(&mut root.indexed_map_mut())?,
                113 => op_map_indexed_try_for_each_mut(&mut root.indexed_map_mut())?,
                114 => op_map_indexed_entry_or_insert(&mut root.indexed_map_mut())?,
                115 => op_map_indexed_entry_or_insert_with(&mut root.indexed_map_mut())?,
                116 => op_map_indexed_entry_or_default(&mut root.indexed_map_mut())?,
                117 => op_map_indexed_entry_and_modify_or_insert(&mut root.indexed_map_mut())?,
                118 => op_map_indexed_occupied_insert(&mut root.indexed_map_mut())?,
                119 => op_map_indexed_occupied_remove(&mut root.indexed_map_mut())?,
                120 => op_map_indexed_vacant_insert(&mut root.indexed_map_mut())?,
                121 => op_map_indexed_fill_values(&mut root.indexed_map_mut())?,
                122 => op_map_indexed_rewrite_values(&mut root.indexed_map_mut())?,
                _ => op_map_indexed_rewrite_values_from(&mut root.indexed_map_mut())?,
            }
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}
