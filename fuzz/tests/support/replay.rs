#[test]
fn validation_seed_mutations() {
    for (_, seed) in blubber_fuzz::validation_seeds() {
        for len in 0..=seed.len() {
            blubber_fuzz::validate(&seed[..len]);
        }
        for index in 2..seed.len() {
            for replacement in [0, 1, 2, 127, 255] {
                let mut mutated = seed.clone();
                mutated[index] = replacement;
                blubber_fuzz::validate(&mutated);
            }
        }
    }
}

#[test]
fn edit_seed_histories() {
    for (_, seed) in blubber_fuzz::edit_seeds() {
        blubber_fuzz::edit(&seed);
    }
}

#[test]
fn committed_corpus() {
    let root = corpus_root();
    for target in ["validate", "edit"] {
        for entry in std::fs::read_dir(root.join(target)).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            match target {
                "validate" => blubber_fuzz::validate(&bytes),
                _ => blubber_fuzz::edit(&bytes),
            }
        }
    }
}
