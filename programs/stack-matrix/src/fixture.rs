//! Each opcode retains an actual call graph through the generated nested editor.

use super::*;
use blubber::{
    pinocchio::{Funding, Headroom, Store},
    List, Map, Result, Str,
};

#[cfg(feature = "wide")]
type Wide = [u8; 1024];
#[cfg(all(not(feature = "wide"), feature = "medium"))]
type Wide = [u8; 256];
#[cfg(not(any(feature = "wide", feature = "medium")))]
type Wide = [u8; 8];
const VALUE: Wide = [7; core::mem::size_of::<Wide>()];
const RUN: &[Wide] = &[VALUE, VALUE];

#[blubber::schema]
struct Body {
    scalar: u64,
    text: Str,
    optional: Option<Wide>,
    fixed: List<Wide>,
    indexed: List<Str>,
    fixed_map: Map<u16, Wide>,
    indexed_map: Map<u16, Str>,
}

macro_rules! node {
    ($name:ident, $child:ty) => {
        #[blubber::schema]
        struct $name {
            child: $child,
        }
    };
}
node!(N1, Body);
node!(N2, N1);
node!(N3, N2);
node!(N4, N3);
#[cfg(feature = "depth16")]
node!(N5, N4);
#[cfg(feature = "depth16")]
node!(N6, N5);
#[cfg(feature = "depth16")]
node!(N7, N6);
#[cfg(feature = "depth16")]
node!(N8, N7);
#[cfg(feature = "depth16")]
node!(N9, N8);
#[cfg(feature = "depth16")]
node!(N10, N9);
#[cfg(feature = "depth16")]
node!(N11, N10);
#[cfg(feature = "depth16")]
node!(N12, N11);

#[cfg(feature = "depth16")]
type Nested = N12;
#[cfg(not(feature = "depth16"))]
type Nested = N4;

#[blubber::schema(account)]
struct Root {
    child: Nested,
}

#[inline(never)]
fn initialize(state: &mut AccountView, id: &Address) -> Result<()> {
    let body = BodyInit {
        scalar: 1,
        text: "seed",
        optional: None,
        fixed: RUN,
        indexed: &["one", "two"],
        fixed_map: &[(1, VALUE), (2, VALUE)],
        indexed_map: &[(1, "one"), (2, "two")],
    };
    let nested = N4Init {
        child: N3Init {
            child: N2Init {
                child: N1Init { child: body },
            },
        },
    };
    #[cfg(feature = "depth16")]
    let nested = N12Init {
        child: N11Init {
            child: N10Init {
                child: N9Init {
                    child: N8Init {
                        child: N7Init {
                            child: N6Init {
                                child: N5Init { child: nested },
                            },
                        },
                    },
                },
            },
        },
    };
    let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::UpTo(0))?;
    Root::initialize(&mut store, RootInit { child: nested })?.commit()?;
    Ok(())
}

#[inline(never)]
fn with_body(
    root: &mut RootMut<'_>,
    edit: impl FnOnce(&mut BodyMut<'_>) -> Result<()>,
) -> Result<()> {
    let mut nested = root.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    #[cfg(feature = "depth16")]
    let mut nested = nested.child_mut();
    let mut nested = nested.child_mut();
    let mut nested = nested.child_mut();
    let mut nested = nested.child_mut();
    let mut nested = nested.child_mut();
    edit(&mut nested)
}

#[inline(never)]
fn leaves(body: &mut BodyMut<'_>, op: u8) -> Result<()> {
    match op {
        0 => body.scalar_mut().set(42),
        1 => body.text_mut().set("replacement"),
        2 => body.text_mut().push_str("suffix"),
        3 => body.text_mut().truncate(1),
        4 => body.text_mut().clear(),
        5 => body.optional_mut().set(Some(VALUE)),
        6 => body.optional_mut().set(None),
        7 => body.fixed_mut().at_mut(0)?.set(VALUE),
        _ => Err(blubber::ErrorKind::InvalidSchema.into()),
    }
}

// Consume wide by-value results in their own frame, as an SBF caller must
// when its input plus returned value and Result temporaries approach 4 KiB.
#[inline(never)]
fn observe<T>(result: Result<T>) -> Result<()> {
    let value = result?;
    core::hint::black_box(&value);
    Ok(())
}

// Isolate operations so the fixture measures one public operation's frame,
// including its wide input/output, without combining unrelated caller locals.
macro_rules! isolated {
    ($operation:ident, $target:ident, $ty:ty, $body:block) => {{
        #[inline(never)]
        fn $operation($target: &mut $ty) -> Result<()> $body
        $operation(&mut $target)
    }};
}

#[inline(never)]
fn fixed_list(body: &mut BodyMut<'_>, op: u8) -> Result<()> {
    let mut list = body.fixed_mut();
    match op {
        0 => isolated!(fixed_list_0, list, blubber::ListMut<'_, Wide>, {
            list.push(VALUE)
        }),
        1 => isolated!(fixed_list_1, list, blubber::ListMut<'_, Wide>, {
            list.insert(0, VALUE)
        }),
        2 => isolated!(fixed_list_2, list, blubber::ListMut<'_, Wide>, {
            list.replace(0, VALUE)
        }),
        3 => isolated!(fixed_list_3, list, blubber::ListMut<'_, Wide>, {
            list.extend_from_slice(RUN)
        }),
        4 => isolated!(fixed_list_4, list, blubber::ListMut<'_, Wide>, {
            list.insert_from_slice(0, RUN)
        }),
        5 => isolated!(fixed_list_5, list, blubber::ListMut<'_, Wide>, {
            {
                let removed = list.remove(0)?;
                core::hint::black_box(&removed);
                Ok(())
            }
        }),
        6 => isolated!(fixed_list_6, list, blubber::ListMut<'_, Wide>, {
            {
                let removed = list.pop()?;
                core::hint::black_box(&removed);
                Ok(())
            }
        }),
        7 => isolated!(fixed_list_7, list, blubber::ListMut<'_, Wide>, {
            {
                let mut out = [0; core::mem::size_of::<Wide>()];
                list.remove_into(0, &mut out)?;
                core::hint::black_box(&out);
                Ok(())
            }
        }),
        8 => isolated!(fixed_list_8, list, blubber::ListMut<'_, Wide>, {
            list.truncate(1)
        }),
        9 => isolated!(fixed_list_9, list, blubber::ListMut<'_, Wide>, {
            list.clear()
        }),
        10 => isolated!(fixed_list_10, list, blubber::ListMut<'_, Wide>, {
            list.try_for_each_mut(|value| value.set(VALUE))
        }),
        _ => Err(blubber::ErrorKind::InvalidSchema.into()),
    }
}

#[inline(never)]
fn indexed_list(body: &mut BodyMut<'_>, op: u8) -> Result<()> {
    let mut list = body.indexed_mut();
    match op {
        0 => list.push("three"),
        1 => list.insert(0, "zero"),
        2 => list.replace(0, "first"),
        3 => list.extend_from_slice(&["three", "four"]),
        4 => list.insert_from_slice(0, &["zero", "half"]),
        5 => list.remove(0),
        6 => {
            list.pop()?;
            Ok(())
        }
        7 => {
            let mut out = [0; 16];
            list.remove_into(0, &mut out)?;
            core::hint::black_box(&out);
            Ok(())
        }
        8 => list.truncate(1),
        9 => list.clear(),
        10 => list.try_for_each_mut(|value| value.push_str("!")),
        _ => Err(blubber::ErrorKind::InvalidSchema.into()),
    }
}

#[inline(never)]
fn fixed_map(body: &mut BodyMut<'_>, op: u8) -> Result<()> {
    let mut map = body.fixed_map_mut();
    match op {
        0 => isolated!(fixed_map_0, map, blubber::MapMut<'_, u16, Wide>, {
            {
                observe(map.insert(3, VALUE))?;
            };
            Ok(())
        }),
        1 => isolated!(fixed_map_1, map, blubber::MapMut<'_, u16, Wide>, {
            {
                observe(map.insert(1, VALUE))?;
            };
            Ok(())
        }),
        2 => isolated!(fixed_map_2, map, blubber::MapMut<'_, u16, Wide>, {
            {
                let removed = map.remove(&1)?;
                core::hint::black_box(&removed);
            };
            Ok(())
        }),
        3 => isolated!(fixed_map_3, map, blubber::MapMut<'_, u16, Wide>, {
            {
                let mut out = [0; core::mem::size_of::<Wide>()];
                map.remove_into(&1, &mut out)?;
                core::hint::black_box(&out);
            };
            Ok(())
        }),
        4 => isolated!(fixed_map_4, map, blubber::MapMut<'_, u16, Wide>, {
            map.clear()?;
            Ok(())
        }),
        5 => isolated!(fixed_map_5, map, blubber::MapMut<'_, u16, Wide>, {
            map.entry(3)?.or_insert(VALUE)?;
            Ok(())
        }),
        6 => isolated!(fixed_map_6, map, blubber::MapMut<'_, u16, Wide>, {
            match map.entry(1)? {
                blubber::Entry::Occupied(entry) => {
                    observe(entry.insert(VALUE))?;
                }
                _ => unreachable!(),
            };
            Ok(())
        }),
        7 => isolated!(fixed_map_7, map, blubber::MapMut<'_, u16, Wide>, {
            match map.entry(1)? {
                blubber::Entry::Occupied(entry) => {
                    let removed = entry.remove()?;
                    core::hint::black_box(&removed);
                }
                _ => unreachable!(),
            };
            Ok(())
        }),
        8 => isolated!(fixed_map_8, map, blubber::MapMut<'_, u16, Wide>, {
            map.try_for_each_mut(|_, value| value.set(VALUE))?;
            Ok(())
        }),
        _ => Err(blubber::ErrorKind::InvalidSchema.into()),
    }
}

#[inline(never)]
fn indexed_map(body: &mut BodyMut<'_>, op: u8) -> Result<()> {
    let mut map = body.indexed_map_mut();
    match op {
        0 | 1 => {
            map.insert(if op == 0 { 3 } else { 1 }, "value")?;
        }
        2 => {
            map.remove(&1)?;
        }
        3 => {
            let mut out = [0; 16];
            map.remove_into(&1, &mut out)?;
            core::hint::black_box(&out);
        }
        4 => map.clear()?,
        5 => map.entry(3)?.or_insert("value")?.push_str("!")?,
        6 => match map.entry(1)? {
            blubber::Entry::Occupied(entry) => entry.insert("first")?,
            _ => unreachable!(),
        },
        7 => match map.entry(1)? {
            blubber::Entry::Occupied(entry) => entry.remove()?,
            _ => unreachable!(),
        },
        8 => map.try_for_each_mut(|_, value| value.push_str("!"))?,
        _ => return Err(blubber::ErrorKind::InvalidSchema.into()),
    }
    Ok(())
}

pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if ix == [0] {
        return initialize(state, id).map_err(Into::into);
    }
    let [family, op] = *ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    // Initialization commits exact used storage; growth families need an explicit
    // lease for nested strings and maximum-width fixed records.
    Root::with_edit_options(
        state,
        id,
        blubber::pinocchio::EditOptions {
            funding: Funding::Prepaid,
            headroom: Headroom::Max,
            refund: None,
        },
        |root| {
            match family {
                1 => with_body(root, |body| leaves(body, op))?,
                2 => with_body(root, |body| fixed_list(body, op))?,
                3 => with_body(root, |body| indexed_list(body, op))?,
                4 => with_body(root, |body| fixed_map(body, op))?,
                5 => with_body(root, |body| indexed_map(body, op))?,
                6 => {
                    root.reserve(32)?;
                    root.shrink_to_fit()?;
                }
                _ => return Err(blubber::ErrorKind::InvalidSchema.into()),
            }
            root.validate()
        },
    )?;
    Ok(())
}
