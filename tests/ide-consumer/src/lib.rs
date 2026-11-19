//! Consumer of local generated methods without extension-trait imports.

#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]

pub mod model;

/// Schema aliases deliberately defined outside the schema declaration module.
pub mod aliases {
    /// UTF-8 schema alias.
    pub type Text = wire::Str;
    /// Collection alias containing an aliased marker from another module.
    pub type Rows = wire::List<crate::model::Record>;
    /// Map alias with variable values.
    pub type Labels = wire::Map<u16, Text>;
}

use model::{Account, AccountMut, RecordInit as RowInit, RecordMut as RowMut};

/// Returns a selected editor through temporary projections and a marker alias.
pub fn select<'a>(account: &'a mut AccountMut<'_>) -> wire::Result<RowMut<'a>> {
    account.rows_mut().at_mut(0)
}

/// Reads a helper-returned reference after temporary collection views end.
pub fn first_name<'a>(account: &'a AccountMut<'_>) -> Option<&'a str> {
    Some(account.rows().get(0)?.display_name())
}

/// Exercises retained records, bound lists, maps, and inherent application helpers.
pub fn edit(account: &mut AccountMut<'_>) -> wire::Result<()> {
    account.append_named("first")?;
    let mut row = select(account)?;
    row.annotate("renamed")?;
    row.name_mut().push_str(" record")?;
    let mut tags = row.tags_mut();
    tags.extend_from_slice(&[8])?;
    tags.insert_from_slice(0, &[])?;
    tags.reborrow().at_mut(0)?.set(9)?;
    tags.replace(1, 10)?;
    account
        .labels_mut()
        .entry(1)?
        .or_insert("label")?
        .push_str(" value")?;
    let mut rows = account.rows_mut();
    rows.push(RowInit {
        name: "second",
        tags: &[],
    })?;
    for record in &rows {
        let _: &str = record.name();
    }
    for record in account.rows() {
        let _: &str = record.name();
    }
    for (key, value) in account.labels() {
        let _: (u16, &str) = (key, value);
    }
    let mut rows = account.rows_mut();
    rows.try_for_each_mut(|record| record.name_mut().push_str("!"))?;
    account.title_mut().set("complete")?;
    account.validate()
}

/// Loads an aliased account and uses its generated getters and application helper.
pub fn inspect(bytes: &[u8]) -> wire::Result<&str> {
    let account = Account::load(bytes)?;
    Ok(account.rows().get(0).unwrap().display_name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renamed_dependency_aliases_and_inherent_helpers_execute() -> wire::Result<()> {
        let mut room = [0; 512];
        let mut store = wire::SliceStore::new(&mut room);
        let mut account = Account::initialize(
            &mut store,
            model::AccountInit {
                rows: &[],
                labels: &[],
                title: "ready",
            },
        )?;
        edit(&mut account)?;
        assert_eq!(first_name(&account), Some("renamed record!"));
        assert_eq!(account.rows().get(0).unwrap().tags().get(0), Some(9));
        assert_eq!(account.rows().get(0).unwrap().tags().get(1), Some(10));
        assert_eq!(account.rows().get(1).unwrap().name(), "second!");
        assert_eq!(account.labels().get(&1), Some("label value"));
        assert_eq!(account.title(), "complete");
        account.commit()?;
        assert_eq!(inspect(&room)?, "renamed record!");
        Ok(())
    }
}
