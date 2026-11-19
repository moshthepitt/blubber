#![forbid(unsafe_code)]
use blubber::{ListMut, Str};
fn bad(list: &mut ListMut<'_, Str>) {
    let _ = list.with_mut(0, |word| {
        let child = word.reborrow();
        word.push_str("growth")?;
        Ok(child.get().len())
    });
}
fn main() {}
