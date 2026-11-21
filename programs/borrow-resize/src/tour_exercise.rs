use crate::state::{Limits, MyAccountMut, RecordInit};
use blubber::{Address, ErrorKind, Result};

/// Runs the complete read/edit tour after checking the stored authority.
///
/// Expects an account initialized from [`crate::state::seed`].
pub fn exercise(acct: &mut MyAccountMut<'_>, actor: Address) -> Result<()> {
    if acct.authority() != actor {
        return Err(ErrorKind::Unauthorized.into());
    }

    let note: &str = acct.note();
    let first = acct.records().get(0).unwrap();
    let name: &str = first.name();
    assert_eq!(note, "ready");
    assert_eq!(name, "one");
    for record in acct.records() {
        for tag in record.tags() {
            assert!(tag > 0);
        }
    }
    for (key, amount) in acct.balances() {
        let _: (Address, u64) = (key, amount);
    }
    // Last uses of the borrowed reads precede the first edit.

    acct.counter_mut().set(1)?;
    acct.limits_mut().set(Limits {
        enabled: true,
        limit: 200,
    })?;
    // docs:start nested-string-edit
    acct.records_mut().at_mut(1)?.name_mut().set("longer")?;
    // docs:end nested-string-edit
    acct.records_mut()
        .at_mut(1)?
        .tags_mut()
        .extend_from_slice(&[4, 5])?;
    acct.records_mut().extend_from_slice(&[RecordInit {
        name: "third",
        tags: &[5],
    }])?;
    acct.records_mut().insert(
        0,
        RecordInit {
            name: "front",
            tags: &[],
        },
    )?;
    acct.records_mut().replace(
        0,
        RecordInit {
            name: "replaced",
            tags: &[6],
        },
    )?;
    acct.records_mut().remove(0)?;
    let _: Option<()> = acct.records_mut().pop()?;

    let mut record = acct.records_mut().at_mut(1)?;
    record.name_mut().push_str(" still here")?;
    let mut tags = record.tags_mut();
    tags.insert_from_slice(0, &[99])?;
    tags.reborrow().at_mut(0)?.set(98)?;
    assert_eq!(tags.remove(0)?, 98);
    tags.replace(0, 7)?;
    tags.truncate(1)?;
    tags.clear()?;

    let old: Option<u64> = acct.balances_mut().insert(actor, 10)?;
    assert_eq!(old, None);
    assert_eq!(acct.balances().get(&actor), Some(10));
    acct.balances_mut()
        .into_entry(actor)?
        .or_insert(0)?
        .set(12)?;
    acct.balances_mut().at_mut(&actor)?.set(13)?;
    acct.balances_mut().try_for_each_mut(|_key, value| {
        let next = value
            .get()
            .checked_add(1)
            .ok_or(ErrorKind::ArithmeticOverflow)?;
        value.set(next)
    })?;
    assert_eq!(acct.balances_mut().remove(&actor)?, Some(14));
    acct.balances_mut().clear()?;

    assert_eq!(acct.labels_mut().insert(2, "two")?, None);
    assert_eq!(acct.labels_mut().insert(2, "second")?, Some(()));
    acct.labels_mut()
        .into_entry(3)?
        .or_insert("three")?
        .set("THREE")?;
    acct.labels_mut().at_mut(&2)?.push_str(" label")?;
    assert_eq!(acct.labels_mut().remove(&2)?, Some(()));
    acct.labels_mut().clear()?;

    let mut cursor = acct.records_mut().cursor_mut()?;
    while let Some(mut record) = cursor.next()? {
        record.name_mut().set("grown during iteration")?;
        record.tags_mut().push(8)?;
    }
    acct.records_mut()
        .try_for_each_mut(|record| record.name_mut().set("visited"))?;

    acct.note_mut().set("finished")?;
    acct.note_mut().truncate(3)?; // Byte count; must be a UTF-8 boundary.
    acct.note_mut().clear()?;
    acct.note_mut().push_str("done")?;
    acct.attachment_mut().extend_from_slice(&[0xaa])?;
    acct.attachment_mut().insert(0, 0xbb)?;
    acct.attachment_mut().replace(0, 0xcc)?;
    assert_eq!(acct.attachment_mut().remove(0)?, 0xcc);
    assert_eq!(acct.attachment_mut().pop()?, Some(0xaa));
    acct.attachment_mut().truncate(0)?;
    acct.attachment_mut().clear()?;
    acct.memo_mut().set(Some("remember"))?;
    acct.memo_mut().some_mut()?.set("remember this")?;
    acct.memo_mut().set(None)?;
    assert_eq!(acct.memo(), None);

    acct.reserve(64)?; // Request at least used_len + 64 retained physical bytes.
    let _trimmed: bool = acct.shrink_to_fit()?; // Pinocchio trims at commit.
    acct.note_mut().set("grew after physical shrink")?;
    acct.records_mut().truncate(1)?;
    acct.records_mut().clear()?;
    acct.validate()?;
    Ok(())
}
