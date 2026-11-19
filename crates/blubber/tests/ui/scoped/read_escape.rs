#![forbid(unsafe_code)]
use blubber::{ListMut, Str};
fn bad(list: &mut ListMut<'_, Str>) {
    let _ = list.with_mut(0, |word| Ok(word.get()));
}
fn main() {}
