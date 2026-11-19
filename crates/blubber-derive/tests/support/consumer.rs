//! Cargo consumer checks shared by the UI and derive suites.
#![forbid(unsafe_code)]
// Each suite uses the checks relevant to its consumers.
#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub struct Consumer {
    pub fixture: PathBuf,
    target: PathBuf,
    manifest: String,
    package: Option<&'static str>,
}

pub fn repo_root(manifest: &str) -> &Path {
    Path::new(manifest).parent().unwrap().parent().unwrap()
}

impl Consumer {
    pub fn new(root: &Path, name: &str, manifest: String, package: Option<&'static str>) -> Self {
        let consumer = Self {
            fixture: root.join(".tmp").join(format!("r2-tests-{name}")),
            target: root.join("target/r2-tests-consumers"),
            manifest,
            package,
        };
        consumer.write("Cargo.toml", &consumer.manifest);
        consumer
    }

    pub fn write(&self, path: &str, contents: &str) {
        let path = self.fixture.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn check_source(
        &self,
        name: &str,
        path: &str,
        source: &str,
        expected: Option<&str>,
        features: &[&str],
    ) -> String {
        self.write(path, source);
        self.check(
            name,
            expected.into_iter().collect::<Vec<_>>().as_slice(),
            features,
        )
    }

    fn command(&self, subcommand: &str) -> Command {
        let mut command = Command::new(env!("CARGO"));
        command
            .args([
                subcommand,
                "--offline",
                "--color",
                "never",
                "--manifest-path",
            ])
            .arg(self.fixture.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &self.target);
        if let Some(package) = self.package {
            command.args(["-p", package]);
        }
        command
    }

    fn check(&self, name: &str, expected: &[&str], features: &[&str]) -> String {
        let mut command = self.command("check");
        if !features.is_empty() {
            command.args(["--features", &features.join(",")]);
        }
        let output = command.output().unwrap();
        let raw = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        self.write(
            &format!("{name}.log"),
            &raw.lines().take(40).collect::<Vec<_>>().join("\n"),
        );
        assert_eq!(
            output.status.success(),
            expected.is_empty(),
            "{name}: {raw}"
        );
        assert!(!raw.contains("custom attribute panicked"), "{name}: {raw}");
        if expected.is_empty() {
            assert!(!raw.contains("warning:"), "{name}: {raw}");
        } else {
            // Rust 1.89 qualifies Copy; newer rustc omits the qualification.
            let normalized = raw.replace("core::marker::Copy", "Copy");
            for message in expected {
                if let Some(primary) = message.strip_prefix("@primary ") {
                    let (code, position) = primary.split_once(' ').unwrap();
                    let diagnostic = raw.split(&format!("error[{code}]")).nth(1).unwrap();
                    let location = diagnostic
                        .lines()
                        .find(|line| line.contains("-->"))
                        .unwrap();
                    assert!(location.trim().ends_with(position), "{name}: {diagnostic}");
                    continue;
                }
                assert!(
                    normalized.contains(message),
                    "{name}: missing {message:?}: {raw}"
                );
            }
            assert!(
                raw.lines().any(|line| line.trim_start().starts_with("-->")),
                "{name}: no source location: {raw}"
            );
        }
        raw
    }

    pub fn check_ui(&self, directory: &Path, inventory: &str, group: &str) -> usize {
        let mut listed = Vec::new();
        for line in inventory.lines() {
            let mut fields = line.split('\t');
            let file = fields.next().unwrap();
            if !file.starts_with(&format!("{group}/")) {
                continue;
            }
            let expected = fields.filter(|message| *message != "-").collect::<Vec<_>>();
            let path = directory.join(file);
            assert!(path.is_file(), "missing UI case: {file}");
            self.write(
                "Cargo.toml",
                &format!("{}\n[lib]\npath = {:?}\n", self.manifest, path),
            );
            self.check(file.trim_end_matches(".rs"), &expected, &[]);
            listed.push(path);
        }
        assert!(!listed.is_empty(), "empty UI group: {group}");
        listed.sort();
        listed.dedup();
        let mut files = fs::read_dir(directory.join(group))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(listed, files, "UI inventory must cover every case");
        listed.len()
    }

    pub fn dependency_tree(&self) -> String {
        let output = self
            .command("tree")
            .args(["--edges", "normal", "--prefix", "none"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tree = String::from_utf8(output.stdout).unwrap();
        self.write("dependency-tree.log", &tree);
        tree
    }
}
