//! Borrow checking for both account lease modes under no_std.

#![forbid(unsafe_code)]
#![cfg(feature = "pinocchio")]

use std::{fs, path::Path, process::Command};

#[test]
fn account_store_lifetimes_and_mode_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = root.join(".tmp/pinocchio-ui");
    fs::create_dir_all(fixture.join("src")).unwrap();
    for alloc in [false, true] {
        fs::write(fixture.join("Cargo.toml"), format!(
            "[workspace]\n[package]\nname = \"pinocchio-ui\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
             [dependencies]\nwire = {{ package = \"blubber\", path = {:?}, default-features = false, features = [\"derive\", \"pinocchio\"{}{}] }}\n\
             pinocchio = {{ version = \"=0.12.0\", default-features = false, features = [\"account-resize\"] }}\n",
            root.join("crates/blubber"), if alloc { ", \"alloc\"" } else { "" }, if cfg!(feature = "verify-on-load") { ", \"verify-on-load\"" } else { "" }
        )).unwrap();
        let mode = if alloc {
            "Headroom::OnDemand"
        } else {
            "Headroom::UpTo(0)"
        };
        let header = "#![no_std]\n#![forbid(unsafe_code)]\n#![deny(warnings)]\nuse pinocchio::{AccountView, Address};\n#[allow(unused_imports)]\nuse wire::{pinocchio::{Store, Headroom, Funding}, ReadStorage, Resize};\n#[allow(unused_imports)]\nuse wire::raw::Resize as _;\n";
        for (name, body, expected) in [
            ("valid", format!("pub fn valid(a: &mut AccountView, owner: &Address) -> wire::Result<()> {{ let mut s = Store::new_with_headroom(a, owner, Funding::Prepaid, {mode})?; let _ = s.bytes().len(); s.resize(16)?; s.commit(16)?; Ok(()) }}"), None),
            ("funding_valid", format!("pub fn valid(a: &mut AccountView, p: &AccountView, sys: &AccountView, r: &mut AccountView, owner: &Address) -> wire::Result<()> {{ let mut s = Store::new_with_headroom(a, owner, Funding::TopUp {{ payer: p, system_program: sys }}, {mode})?.with_refund(r)?; let _ = s.bytes().len(); s.resize(16)?; s.commit(16)?; Ok(()) }}"), None),
            ("scoped_valid", "#[wire::schema(account)] pub struct State { pub authority: Address, pub text: wire::Str, pub map: wire::Map<Address, u64> } pub fn valid(a: &mut AccountView, owner: &Address) -> wire::Result<()> { State::with_read(a, owner, |acct| Ok(acct.text().len()))?; State::with_edit(a, owner, Funding::Prepaid, |acct| acct.text_mut().set(\"text\"))?; Ok(()) }".into(), None),
            ("options_valid", "#[wire::schema(account)] pub struct State { pub values: wire::List<u64> } pub fn valid(a: &mut AccountView, owner: &Address) -> wire::Result<()> { State::with_edit_options(a, owner, wire::pinocchio::EditOptions { funding: Funding::Prepaid, headroom: Headroom::UpTo(16), refund: None }, |root| { root.values_mut().push(1)?; root.values_mut().push(2) })?; Ok(()) }".into(), None),
            ("options_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, owner: &Address) -> wire::Result<&'a str> { Ok(State::with_edit_options(a, owner, Funding::Prepaid.into(), |root| Ok(root.text()))?.0) }".into(), Some("lifetime may not live long enough")),
            ("checked_valid", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn valid(a: &mut AccountView, owner: &Address) -> wire::Result<()> { State::with_read_checked(a, owner, |root| root.verify())?; State::with_edit_checked(a, owner, Funding::Prepaid, |root| root.verify())?; let mut s = Store::new_with_headroom(a, owner, Funding::Prepaid, Headroom::UpTo(0))?; s.load_checked::<State>()?.verify()?; Ok(()) }".into(), None),
            ("checked_options_valid", "#[wire::schema(account)] pub struct State { pub values: wire::List<u64> } pub fn valid(a: &mut AccountView, r: &mut AccountView, owner: &Address) -> wire::Result<()> { State::with_edit_checked_options(a, owner, wire::pinocchio::EditOptions { funding: Funding::Prepaid, headroom: Headroom::UpTo(8), refund: Some(r) }, |root| root.values_mut().push(1))?; r.set_lamports(1); Ok(()) }".into(), None),
            ("checked_options_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, owner: &Address) -> wire::Result<&'a str> { Ok(State::with_edit_checked_options(a, owner, Funding::Prepaid.into(), |root| Ok(root.text()))?.0) }".into(), Some("lifetime may not live long enough")),
            ("checked_options_refund_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, r: &'a mut AccountView, owner: &Address) -> wire::Result<&'a mut AccountView> { Ok(State::with_edit_checked_options(a, owner, wire::pinocchio::EditOptions { funding: Funding::Prepaid, headroom: Headroom::UpTo(0), refund: Some(r) }, |_| Ok(r))?.0) }".into(), Some("error[E0505]")),
            ("typed_valid", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn valid(a: &mut AccountView, owner: &Address) -> wire::Result<()> { { let s = wire::pinocchio::ReadStore::new(a, owner)?; s.load::<State>()?.verify()?; } let mut s = Store::new_with_headroom(a, owner, Funding::Prepaid, Headroom::UpTo(0))?; s.load::<State>()?.commit()?; Ok(()) }".into(), None),
            ("typed_read_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a AccountView, owner: &Address) -> wire::Result<&'a str> { let s = wire::pinocchio::ReadStore::new(a, owner)?; Ok(s.load::<State>()?.text()) }".into(), Some("error[E0515]")),
            ("typed_edit_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, owner: &Address) -> wire::Result<StateMut<'a>> { let mut s = Store::new_with_headroom(a, owner, Funding::Prepaid, Headroom::UpTo(0))?; s.load::<State>() }".into(), Some("error[E0515]")),
            ("checked_read_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a AccountView, owner: &Address) -> wire::Result<&'a str> { State::with_read_checked(a, owner, |root| Ok(root.text())) }".into(), Some("lifetime may not live long enough")),
            ("checked_edit_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, owner: &Address) -> wire::Result<(&'a str, wire::CommitInfo)> { State::with_edit_checked(a, owner, Funding::Prepaid, |root| Ok(root.text())) }".into(), Some("lifetime may not live long enough")),
            ("verify_capability", "pub fn bad(_: wire::__private::ValidatedIngress) {}".into(), Some("cannot find type `ValidatedIngress`")),
            ("child_verify", "#[wire::schema] pub struct Child { pub flag: bool } #[wire::schema(account)] pub struct State { pub child: Child } pub fn bad(root: &mut StateMut<'_>) -> wire::Result<()> { root.child_mut().verify() }".into(), Some("no method named `verify`")),
            ("scoped_read_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a AccountView, owner: &Address) -> wire::Result<&'a str> { State::with_read(a, owner, |acct| Ok(acct.text())) }".into(), Some("lifetime may not live long enough")),
            ("scoped_edit_escape", "#[wire::schema(account)] pub struct State { pub text: wire::Str } pub fn bad<'a>(a: &'a mut AccountView, owner: &Address) -> wire::Result<(&'a str, wire::CommitInfo)> { State::with_edit(a, owner, Funding::Prepaid, |acct| Ok(acct.text())) }".into(), Some("lifetime may not live long enough")),
            ("payer_reuse", format!("pub fn bad(a: &mut AccountView, p: &mut AccountView, sys: &AccountView, owner: &Address) -> wire::Result<()> {{ let s = Store::new_with_headroom(a, owner, Funding::TopUp {{ payer: p, system_program: sys }}, {mode})?; p.set_lamports(1); let _ = s.bytes(); Ok(()) }}"), Some("error[E0502]")),
            ("refund_reuse", format!("pub fn bad(a: &mut AccountView, r: &mut AccountView, owner: &Address) -> wire::Result<()> {{ let s = Store::new_with_headroom(a, owner, Funding::Prepaid, {mode})?.with_refund(r)?; r.set_lamports(1); let _ = s.bytes(); Ok(()) }}"), Some("error[E0499]")),
            ("escape", format!("pub fn escape(a: &mut AccountView, owner: &Address) -> wire::Result<Store<'static>> {{ let s = Store::new_with_headroom(a, owner, Funding::Prepaid, {mode})?; let _ = s.bytes(); let _ = s.storage_len(); Ok(s) }}"), Some("lifetime may not live long enough")),
            ("caller", format!("pub fn bad(a: &mut AccountView, owner: &Address) -> wire::Result<()> {{ let s = Store::new_with_headroom(a, owner, Funding::Prepaid, {mode})?; a.set_lamports(1); let _ = s.bytes(); let _ = s.storage_len(); Ok(()) }}"), Some("error[E0499]")),
            ("read_across_edit", format!("#[wire::schema(account)] pub struct State {{ pub text: wire::Str }} pub fn bad(a: &mut AccountView, owner: &Address) -> wire::Result<()> {{ let mut s = Store::new_with_headroom(a, owner, Funding::Prepaid, {mode})?; let _ = s.bytes(); let _ = s.storage_len(); let mut root = State::load_mut(&mut s)?; let old = root.text(); root.text_mut().push_str(\"growth\")?; let _ = old.len(); Ok(()) }}"), Some("error[E0502]")),
        ] {
            fs::write(fixture.join("src/lib.rs"), format!("{header}{body}\n")).unwrap();
            let output = Command::new(env!("CARGO"))
                .args(["check", "--offline", "--color", "never", "--manifest-path"])
                .arg(fixture.join("Cargo.toml"))
                .env("CARGO_TARGET_DIR", root.join("target"))
                .output().unwrap();
            let log = String::from_utf8_lossy(&output.stderr);
            fs::write(fixture.join(format!("{alloc}-{name}.log")), log.as_bytes()).unwrap();
            assert_eq!(output.status.success(), expected.is_none(), "{name}, alloc={alloc}: {log}");
            if let Some(message) = expected { assert!(log.contains(message), "{name}: {log}"); }
        }
    }
}
