use super::*;

#[test]
fn rent_skip_requires_all_three_conditions_after_the_actual_trim() {
    for opening in [0, 128, MAX_STORAGE] {
        for balance in [0, 1, u64::MAX] {
            for physical in [0, 64, 128, 200, MAX_STORAGE] {
                for changed in [false, true] {
                    for refund in [false, true] {
                        let final_balance = if changed { balance ^ 1 } else { balance };
                        assert_eq!(
                            settlement_unchanged(opening, balance, physical, final_balance, refund),
                            physical == opening && !changed && !refund,
                        );
                    }
                }
            }
        }
    }
    for retained in [0, 160] {
        for fail in [false, true] {
            let (physical, error) = trim(200, 100usize.max(retained), |target| {
                if fail {
                    (200, Err(ProgramError::InvalidRealloc))
                } else {
                    (target, Ok(()))
                }
            });
            assert_eq!(error.is_some(), fail);
            // Opening length is 128, before speculative growth to 200. Neither
            // the requested 100-byte trim nor used_len can justify skipping.
            assert!(!settlement_unchanged(128, 99, physical, 99, false));
        }
    }
}

#[test]
fn final_physical_storage_controls_rent_even_after_failed_trim() {
    let mut rent = [0; 17];
    rent[..8].copy_from_slice(&19u64.to_le_bytes());
    rent[8..16].copy_from_slice(&1f64.to_le_bytes());
    for retained in [0, 160] {
        for fail in [false, true] {
            let (physical, error) = trim(200, 100usize.max(retained), |target| {
                if fail {
                    (200, Err(ProgramError::InvalidRealloc))
                } else {
                    (target, Ok(()))
                }
            });
            let expected = if fail { 200 } else { 100usize.max(retained) };
            assert_eq!(physical, expected);
            assert_eq!(error, fail.then(|| ProgramError::InvalidRealloc.into()));
            assert_eq!(
                rent::from_bytes(rent, physical),
                Ok((128 + expected as u64) * 19)
            );
        }
    }
    assert_eq!(
        trim(200, 200, |_| panic!("equal-length resize")),
        (200, None)
    );
}

#[test]
fn refund_checks_both_balances_before_writing() {
    assert_eq!(refund_balances(100, 17, 40), Ok((60, 57)));
    assert_eq!(refund_balances(100, u64::MAX, 0), Ok((100, u64::MAX)));
    assert_eq!(
        refund_balances(100, u64::MAX, 1),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
    assert_eq!(
        refund_balances(0, 17, 1),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
}

#[test]
fn every_remaining_window_is_found_with_bounded_monotone_probes() {
    // Every possible remaining byte count after earlier growth, including zero.
    for remaining in 0..=MAX_INCREASE {
        for headroom in [
            Headroom::Max,
            Headroom::UpTo(0),
            Headroom::UpTo(37),
            Headroom::UpTo(8_191),
        ] {
            let current = 30_000;
            let (target, _) = probe_target(current, headroom).unwrap();
            let ceiling = current + remaining;
            let mut calls = 0;
            let mut physical = current;
            let mut zeroed = 0;
            let found = discover(current, target, |len| {
                calls += 1;
                assert!(len <= target);
                if len > ceiling {
                    return Err(ProgramError::InvalidRealloc);
                }
                assert!(len >= physical, "successful probe shrank storage");
                zeroed += len - physical;
                physical = len;
                Ok(())
            })
            .unwrap();
            assert_eq!(found, target.min(ceiling));
            assert_eq!(physical, found);
            assert_eq!(zeroed, found - current);
            assert!(calls <= 15, "remaining={remaining}, calls={calls}");
        }
    }
}

#[test]
fn every_probe_range_width_and_hard_cap_are_checked() {
    for width in 0..=MAX_INCREASE {
        let current = MAX_STORAGE - width;
        let (target, cap) = probe_target(current, Headroom::Max).unwrap();
        assert_eq!(target, MAX_STORAGE);
        assert_eq!(cap, MAX_STORAGE);
        for remaining in [0, width / 2, width] {
            let mut calls = 0;
            let found = discover(current, target, |len| {
                calls += 1;
                if len <= current + remaining {
                    Ok(())
                } else {
                    Err(ProgramError::InvalidRealloc)
                }
            })
            .unwrap();
            assert_eq!(found, current + remaining);
            assert!(calls <= 15);
        }
    }
    assert_eq!(
        probe_target(MAX_STORAGE + 1, Headroom::Max),
        Err(ErrorKind::Capacity.into())
    );
    assert_eq!(
        probe_target(12, Headroom::UpTo(usize::MAX)),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
}

#[test]
fn other_sdk_errors_propagate_at_first_and_later_probe() {
    let error = ProgramError::Custom(77);
    assert_eq!(
        discover(12, 100, |_| Err(error.clone())),
        Err(error.clone().into())
    );
    let mut calls = 0;
    let result = discover(12, 100, |_| {
        calls += 1;
        Err(if calls == 1 {
            ProgramError::InvalidRealloc
        } else {
            error.clone()
        })
    });
    assert_eq!(calls, 2);
    assert_eq!(result, Err(error.into()));
}

#[test]
fn no_op_target_makes_no_sdk_calls() {
    assert_eq!(discover(12, 12, |_| panic!("no-op resize")), Ok(12));
    let (target, _) = probe_target(12, Headroom::UpTo(0)).unwrap();
    assert_eq!(
        session_limit(12, target, |_| panic!("UpTo(0) resize")),
        Ok(12)
    );
}

#[test]
fn constructor_restores_partial_growth_and_preserves_original_error() {
    use core::cell::Cell;
    for fail_restore in [false, true] {
        let physical = Cell::new(100);
        let calls = Cell::new(0);
        let result = session_limit(100, 200, |len| {
            calls.set(calls.get() + 1);
            match calls.get() {
                1 => Err(ProgramError::InvalidRealloc),
                2 => {
                    physical.set(len);
                    Ok(())
                }
                3 => Err(ProgramError::Custom(77)),
                4 => {
                    assert_eq!(len, 100);
                    if fail_restore {
                        Err(ProgramError::Custom(88))
                    } else {
                        physical.set(len);
                        Ok(())
                    }
                }
                _ => panic!("unexpected resize"),
            }
        });
        assert_eq!(result, Err(ProgramError::Custom(77).into()));
        assert_eq!(calls.get(), 4);
        assert_eq!(physical.get(), if fail_restore { 150 } else { 100 });
    }
    assert_eq!(
        session_limit(12, 100, |_| Err(ProgramError::Custom(77))),
        Err(ProgramError::Custom(77).into())
    );
}

#[cfg(feature = "alloc")]
#[test]
fn on_demand_releases_once_per_growth_and_always_reacquires() {
    use core::cell::Cell;
    struct Guard<'a>(&'a Cell<usize>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let releases = Cell::new(0);
    let acquisitions = Cell::new(0);
    let mut guard = Some(Guard(&releases));
    let mut poison = false;
    let mut physical = 64;
    let mut calls = 0;
    // Same-size readiness requires no re-lease. Fifty 8-byte pushes share
    // production slack/retry and guard transition code, with counted fake SDK.
    for needed in (64..=464).step_by(8) {
        if needed <= physical {
            continue;
        }
        let target = growth_target(physical, needed);
        re_lease(
            &mut guard,
            &mut poison,
            || {
                resize_with_retry(needed, target, |len| {
                    calls += 1;
                    physical = len;
                    Ok(())
                })
            },
            || {
                acquisitions.set(acquisitions.get() + 1);
                Ok(Guard(&releases))
            },
        )
        .unwrap_or_else(|_| panic!("growth failed"));
    }
    assert_eq!(
        (physical, calls, releases.get(), acquisitions.get()),
        (478, 8, 8, 8)
    );
    for error in [ProgramError::InvalidRealloc, ProgramError::Custom(77)] {
        let result = re_lease(
            &mut guard,
            &mut poison,
            || Err(error.clone()),
            || {
                acquisitions.set(acquisitions.get() + 1);
                Ok(Guard(&releases))
            },
        );
        assert!(matches!(result, Err(ReLeaseError::Resize(e)) if e == error));
        assert!(guard.is_some());
        assert!(!poison);
    }
    let result = re_lease(
        &mut guard,
        &mut poison,
        || Ok(()),
        || Err(ProgramError::AccountBorrowFailed),
    );
    assert!(matches!(
        result,
        Err(ReLeaseError::Reacquire(ProgramError::AccountBorrowFailed))
    ));
    assert!(guard.is_none());
    assert!(poison);
}

#[cfg(feature = "alloc")]
#[test]
fn on_demand_retries_exact_only_for_invalid_realloc() {
    for (needed, expected) in [
        (64 + MAX_INCREASE, Ok(())),
        (65 + MAX_INCREASE, Err(ProgramError::InvalidRealloc)),
    ] {
        let mut calls = 0;
        let result = resize_with_retry(needed, growth_target(64, needed), |len| {
            calls += 1;
            if len > 64 + MAX_INCREASE {
                Err(ProgramError::InvalidRealloc)
            } else {
                Ok(())
            }
        });
        assert_eq!(calls, 2);
        assert_eq!(result, expected);
    }
    let mut calls = 0;
    assert_eq!(
        resize_with_retry(72, 88, |_| {
            calls += 1;
            Err(ProgramError::Custom(77))
        }),
        Err(ProgramError::Custom(77))
    );
    assert_eq!(calls, 1);
    assert_eq!(growth_target(MAX_STORAGE - 1, MAX_STORAGE), MAX_STORAGE);
}

#[test]
fn exact_budget_selection_uses_zero_or_one_resize() {
    assert_eq!(Headroom::default(), Headroom::UpTo(0));
    assert_eq!(
        EditOptions::from(Funding::Prepaid).headroom,
        Headroom::UpTo(0)
    );
    for current in [12, 128, MAX_STORAGE - 8] {
        for room in [0, 8] {
            let (target, _) = probe_target(current, Headroom::UpTo(room)).unwrap();
            let mut calls = 0;
            let leased = session_limit(current, target, |len| {
                calls += 1;
                assert_eq!(len, current + room);
                Ok(())
            })
            .unwrap();
            assert_eq!(leased, current + room);
            assert_eq!(calls, usize::from(room != 0));
        }
    }
    assert_eq!(
        probe_target(12, Headroom::UpTo(usize::MAX)),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
}
