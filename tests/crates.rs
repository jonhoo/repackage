use cargo_toml::Manifest;
use std::path::Path;
use std::process::Command;

macro_rules! repackage {
    ($krate:literal, $into:literal) => {{
        let output = Command::new("cargo")
            .arg("package")
            .arg("--quiet")
            .arg("--allow-dirty")
            .arg("--no-verify")
            .arg("--no-metadata")
            .current_dir(concat!("tests/test-crates/", $krate))
            .env_remove("CARGO_TARGET_DIR")
            .output()
            .expect("failed to run cargo package");
        assert!(
            output.status.success(),
            "cargo package failed: {:?}",
            output
        );

        let dot_crate = Path::new(concat!(
            "tests/test-crates/",
            $krate,
            "/target/package/",
            $krate,
            "-0.1.0.crate"
        ));
        assert!(
            dot_crate.exists(),
            "{} does not exist after cargo package",
            dot_crate.display()
        );

        repackage::dot_crate(dot_crate, Some($krate), $into).expect("repackaging to rptest failed");

        let new_dot_crate = Path::new(concat!(
            "tests/test-crates/",
            $krate,
            "/target/package/",
            $into,
            "-0.1.0.crate"
        ));
        assert!(
            new_dot_crate.exists(),
            "{} does not exist after repackage",
            new_dot_crate.display()
        );

        let dot_crate =
            std::fs::File::open(new_dot_crate).expect("could not open repackaged .crate");
        let dot_crate = flate2::read::GzDecoder::new(dot_crate);
        let mut dot_crate = tar::Archive::new(dot_crate);

        let unpkg = Path::new(concat!("tests/test-crates/", $krate, "/target/unpackage"));
        if unpkg.exists() {
            std::fs::remove_dir_all(&unpkg).expect("failed to remove old unpackage dir");
        }
        std::fs::create_dir_all(&unpkg).expect("failed to create unpackage dir");
        dot_crate
            .unpack(unpkg)
            .expect("failed to unpackage repackaged .crate");

        let unpkg = unpkg.join(concat!($into, "-0.1.0"));
        let cargo_toml = unpkg.join("Cargo.toml");

        // To avoid reading into memory we need:
        // https://github.com/alexcrichton/toml-rs/issues/215
        let cargo_toml = std::fs::read(&cargo_toml).expect("failed to read repackaged Cargo.toml");
        let manifest = Manifest::from_slice(&cargo_toml)
            .expect("parse Cargo.toml from repackaged .crate file");

        assert_eq!(
            manifest
                .package
                .as_ref()
                .expect("repackaged manifest has no package")
                .name,
            $into
        );

        // Check that the repackaged package actually builds
        let output = Command::new("cargo")
            .arg("check")
            .arg("--all-targets")
            .current_dir(&unpkg)
            .env_remove("CARGO_TARGET_DIR")
            .output()
            .expect("failed to run cargo package");
        assert!(
            output.status.success(),
            "cargo package failed: {:?}",
            output
        );

        (manifest, unpkg)
    }};
}

#[test]
fn trivial() {
    let (_manifest, _unpkg) = repackage!("trivial", "rptest");
}

#[test]
fn with_tests() {
    let (_manifest, _unpkg) = repackage!("with-tests", "wt");
}
