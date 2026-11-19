#[test]
fn authenticated_seed_histories() {
    let seeds = blubber_fuzz::corrupt::seeds();
    let mut coverage = blubber_fuzz::corrupt::Coverage::default();
    for (_, seed) in &seeds {
        coverage += blubber_fuzz::corrupt::check(seed, false);
        coverage += blubber_fuzz::corrupt::check(seed, true);
    }
    coverage.assert_nonvacuous();
    println!(
        "d21-corrupt seeds: {} directed tapes, read and edit; {coverage}",
        seeds.len()
    );
}

#[test]
fn authenticated_committed_corpus() {
    for (target, edit) in [("policy_read", false), ("policy_edit", true)] {
        let directory = corpus_root().join(target);
        // Every named family is committed with the exact tape it generates.
        let families = blubber_fuzz::corrupt::corpus(edit);
        for (name, tape) in &families {
            let committed = std::fs::read(directory.join(name))
                .unwrap_or_else(|_| panic!("missing corpus family {target}/{name}"));
            assert_eq!(&committed, tape, "stale corpus family {target}/{name}");
        }
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let data = std::fs::read(entry.unwrap().path()).unwrap();
            blubber_fuzz::corrupt::check(&data, edit);
            count += 1;
        }
        println!(
            "d21-corrupt corpus {target}: {count} tapes, {} named families",
            families.len()
        );
    }
}
