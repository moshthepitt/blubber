# Comparison operation code

The snippets show the operation bodies after the load. The shared `grow` function builds the same 17-byte replacement in each fixture. Snippet counts cover the displayed operations. The linked adapters show owner checks, load, error mapping, serialization, commit and borrow and resize order. Zeropod and Wincode share the owned-operation helper. The Zeropod snippet shows that helper inside the root replacement.

Snippet counts include every nonblank Rust line, including braces. Adapter counts cover all workloads, shared helper declarations and host setup in each file. The [CU results](../COMPARE.md) measure instruction cost.

| Library | Grow snippet lines | Insert snippet lines | Complete adapter lines |
|---|---|---|---|
| borsh | 4 | 1 | 129 |
| blubber_trusted | 10 | 2 | 204 |
| blubber_checked | 10 | 2 | 204 |
| star_frame | 6 | 7 | 192 |
| bytemuck | n/a | 5 | 36 |
| sokoban | n/a | 2 | 45 |
| zeropod | 6 | 3 | 90 |
| wincode | 4 | 1 | 99 |
| pinapod | 6 | 2 | 160 |
| light | n/a | 1 | 48 |
| anchor_slab | n/a | 2 | 40 |
| critbit | n/a | 2 | 50 |
| manifest | n/a | 19 | 203 |
| manifest_checked | n/a | 19 | 203 |

## borsh

[program/src/owned.rs](program/src/owned.rs). W4 owned BTreeMap; decoder/encoder lifecycle in linked handler.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>for s in v.values_mut() {
    let next = grow(s);
    *s = core::str::from_utf8(&amp;next).unwrap().into();
}</pre></td><td><pre>v.insert(insert_key(n), EQUAL.into());</pre></td></tr></table>

## blubber_trusted

[program/src/blubber_adapter.rs](program/src/blubber_adapter.rs). Loads with `load_trusted` or `load_trusted_mut`. Mutable instructions commit after the operation.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>let mut staged = Vec::&lt;[u8; 17]&gt;::with_capacity(n);
for item in root.value().try_iter() {
    let (_, old) = item?;
    staged.push(grow(old));
}
let values: Vec&lt;&amp;str&gt; = staged
    .iter()
    .map(|s| core::str::from_utf8(s).unwrap())
    .collect();
root.value_mut().rewrite_values(&amp;values)?;</pre></td><td><pre>root.value_mut()
    .insert(Pubkey::from_bytes(insert_key(n)), EQUAL)?;</pre></td></tr></table>

## blubber_checked

[program/src/blubber_checked_adapter.rs](program/src/blubber_checked_adapter.rs). Uses the same operation bodies and `Headroom::Max` as the lazy fixture. Loads with `load` or `load_checked_mut` and commits mutable instructions.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>let mut staged = Vec::&lt;[u8; 17]&gt;::with_capacity(n);
for item in root.value().try_iter() {
    let (_, old) = item?;
    staged.push(grow(old));
}
let values: Vec&lt;&amp;str&gt; = staged
    .iter()
    .map(|s| core::str::from_utf8(s).unwrap())
    .collect();
root.value_mut().rewrite_values(&amp;values)?;</pre></td><td><pre>root.value_mut()
    .insert(Pubkey::from_bytes(insert_key(n)), EQUAL)?;</pre></td></tr></table>

## star_frame

[star/src/lib.rs](star/src/lib.rs). W4 native exclusive wrapper; bulk owned replacement, including all staging and resize costs.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>let mut values = TextMap::owned_from_ptr(&amp;v).map_err(error)?;
for s in values.values_mut() {
    let next = grow(s);
    *s = core::str::from_utf8(&amp;next).unwrap().into();
}
v.set_from_owned(values).map_err(error)?;</pre></td><td><pre>v.insert(insert_key(n), star_frame::unsize::init::DefaultInit)
    .map_err(error)?;
v.get_exclusive(&amp;insert_key(n))
    .map_err(error)?
    .unwrap()
    .set(EQUAL)
    .map_err(error)?;</pre></td></tr></table>

## bytemuck

[program/src/bytemuck_adapter.rs](program/src/bytemuck_adapter.rs). W1 numeric array; insert is push, capacity 1000.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>if n == CAPACITY {
    return Err(ProgramError::AccountDataTooSmall);
}
v[n] = SET;
*len += 1;</pre></td></tr></table>

## sokoban

[program/src/sokoban_adapter.rs](program/src/sokoban_adapter.rs). W2 numeric tree, capacity 1000; no variable String payload.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>tree.insert(insert_key(n), SET)
    .ok_or(ProgramError::AccountDataTooSmall)?;</pre></td></tr></table>

## zeropod

[program/src/zeropod_adapter.rs](program/src/zeropod_adapter.rs). W4 root replacement; handler also measures, resizes, revalidates, sets and trims.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>let mut value = BTreeMap::&lt;Key, String&gt;::own(view);
for s in value.values_mut() {
    let next = grow(s);
    *s = core::str::from_utf8(&amp;next).unwrap().into();
}
edit.set(value.input()).map_err(error)?;</pre></td><td><pre>let mut value = BTreeMap::&lt;Key, String&gt;::own(view);
value.insert(insert_key(n), EQUAL.into());
edit.set(value.input()).map_err(error)?;</pre></td></tr></table>

## wincode

[program/src/wincode_adapter.rs](program/src/wincode_adapter.rs). W4 owned BTreeMap mutation; serialize/resize in linked handler.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>for s in v.values_mut() {
    let next = grow(s);
    *s = core::str::from_utf8(&amp;next).unwrap().into();
}</pre></td><td><pre>v.insert(insert_key(n), EQUAL.into());</pre></td></tr></table>

## pinapod

[program/src/pinapod_adapter.rs](program/src/pinapod_adapter.rs). W3 old-dependent vector growth with fixed 17-byte string slots; W1 numeric push. Handler preflights, resizes when the element count changes, updates and trims.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>for s in &amp;mut values {
    let next = grow(s.as_str());
    s.try_set(core::str::from_utf8(&amp;next).unwrap())
        .map_err(error)?;
}
let patch = PinaWordsPatch::new().replace_values(&amp;values);</pre></td><td><pre>values.push(SET.into());
let patch = PinaNumbersPatch::new().replace_values(&amp;values);</pre></td></tr></table>

## light

[program/src/light_adapter.rs](program/src/light_adapter.rs). W1 numeric vector; insert is push, runtime capacity 1000.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>v.push(SET).map_err(|_| ProgramError::AccountDataTooSmall)?;</pre></td></tr></table>

## anchor_slab

[program/src/anchor_slab_adapter.rs](program/src/anchor_slab_adapter.rs). W1 numeric slab; exact growth, rent prepaid, default guardrails; map/string workloads unsupported.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>view.resize_to_capacity((view.len() + 1) as u32)?;
view.try_push(SET)?;</pre></td></tr></table>

## critbit

[program/src/critbit_adapter.rs](program/src/critbit_adapter.rs). W2 projected 128-bit key, fixed capacity 1000; no variable strings.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>tree.insert(project(insert_key(n)), SET)
    .ok_or(ProgramError::AccountDataTooSmall)?;</pre></td></tr></table>

## manifest

[program/src/manifest_adapter.rs](program/src/manifest_adapter.rs). W2 full 256-bit key; header snapshot h already loaded and its borrow released. The snippet includes resize, free-list allocation, insertion and header persistence; native graph trust.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>let old_slots_bytes = state.data_len() - HEADER;
let expanded = h.free == NIL;
if expanded {
    state.resize(state.data_len() + SLOT)?;
}
let mut bytes = state.try_borrow_mut()?;
let (prefix, arena) = bytes.split_at_mut(HEADER);
let mut free = FreeList::&lt;[u8; 52]&gt;::new(arena, h.free);
if expanded {
    free.add(old_slots_bytes as u32);
}
let index = free.remove();
h.free = free.get_head();
let mut tree = RedBlackTree::&lt;Entry&gt;::new(arena, h.root, h.max);
tree.insert(index, Entry { key: insert_key(n), value: SET });
h.root = tree.get_root_index();
h.max = tree.get_max_index();
h.len += 1;
save(prefix, h);</pre></td></tr></table>

## manifest_checked

[program/src/manifest_adapter.rs](program/src/manifest_adapter.rs). Same complete W2 insertion body; initial header load additionally calls upstream validate_red_black_tree. Initial load is excluded from both snippet counts; no variable strings.

<table><tr><th>Grow all from old values</th><th>Insert one</th></tr><tr><td><pre>No variable-string container measured.</pre></td><td><pre>let old_slots_bytes = state.data_len() - HEADER;
let expanded = h.free == NIL;
if expanded {
    state.resize(state.data_len() + SLOT)?;
}
let mut bytes = state.try_borrow_mut()?;
let (prefix, arena) = bytes.split_at_mut(HEADER);
let mut free = FreeList::&lt;[u8; 52]&gt;::new(arena, h.free);
if expanded {
    free.add(old_slots_bytes as u32);
}
let index = free.remove();
h.free = free.get_head();
let mut tree = RedBlackTree::&lt;Entry&gt;::new(arena, h.root, h.max);
tree.insert(index, Entry { key: insert_key(n), value: SET });
h.root = tree.get_root_index();
h.max = tree.get_max_index();
h.len += 1;
save(prefix, h);</pre></td></tr></table>
