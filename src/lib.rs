//! Repackage `.crate` files under a different crate name.
//!
//! This crate provides [`repackage::dot_crate`](dot_crate), which repackages a `.crate` file so
//! that it exports the same crate under a different name. It replaces the `name` attribute in
//! `Cargo.toml`, and also rewrites references to the old name in the various `.rs` files that live
//! outside of `src/` (those in `src/` use `crate::`).
//!
//! # Rewriting .rs files
//!
//! Normally, rewriting the `name` in `Cargo.toml` should be sufficient for _most_ use-cases.
//! Consumers of a `.crate` file likely only care about the exported library, which only ever
//! refers to itself using paths starting with `crate::` or `::`, not including the name. Tests and
//! binaries do have to name the library crate, but are usually not used by downstream consumers of
//! the `.crate`. But, _just_ in case, this crate tries to modify those files as well using some
//! simple string replacement. It's brittle though, so you might only get so far with that approach
//! if you make heavy use of non-library artifacts in the produced `.crate` files.
#![warn(missing_docs, broken_intra_doc_links)]

use anyhow::Context;
use cargo_toml::Manifest;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

/// Repackage the crate contained in the `.crate` tarball at `dot_crate` as `new_name`.
///
/// Pass in the old crate name to verify that the crate you are repackaging is in fact the one you
/// think it is. If you do not, it will be inferred from the name of the `.crate` file. The old
/// name is needed one way or the other in order to also replace references to the crate inside
/// non-`src/` .rs files.
///
/// The repackaged file will end up next to the current `.crate` file with the crate name replaced
/// appropriately. In other words, if you are replacing `foo` with `bar`, and give the input file
/// `baz/foo-0.1.0.crate`, the repackaged crate file will be `baz/bar-0.1.0.crate`.
pub fn dot_crate(
    dot_crate: impl AsRef<Path>,
    old_name: Option<&str>,
    new_name: &str,
) -> anyhow::Result<()> {
    let dot_crate = dot_crate.as_ref();

    // We want to use the same file path, but with the crate name replaced.
    // To do that we first need to extract the file name portion of the .crate path:
    let old_fn = dot_crate
        .file_name()
        .ok_or_else(|| anyhow::anyhow!(".crate file path '{}' is not a file", dot_crate.display()))?
        .to_str()
        .ok_or_else(|| {
            anyhow::anyhow!(
                ".crate file path '{}' is not valid utf-8",
                dot_crate.display()
            )
        })?;

    // Next, we verify that the .crate file is actually for the crate the user wanted to replace.
    // Otherwise, we might be repackaging some entirely different crate. Now, that will also be
    // caught once we get to the Cargo.toml file and verify its name, but if we can catch a mistake
    // sooner, that's better.
    //
    // Let's also handle the somewhat unlikely (but possible) prefix problem: Imagine someone wants
    // to rewrite the net crate to net2, but then pass us the crate file:
    //
    //     netscape-0.1.0.crate
    //
    // Sure, it _starts_ with net, but it's probably not the .crate file they intended to pass us.
    //
    // The trick is to look for the first . (which cannot appear in crate names), and walk
    // _backwards_ from there.
    let mut prefix = None;
    if let Some(dot) = old_fn.find('.') {
        if let Some(dash) = old_fn[..dot].rfind('-') {
            let name = &old_fn[..dash];
            let major = &old_fn[(dash + 1)..dot];
            if !name.is_empty() && !major.is_empty() && major.chars().all(|c| c.is_ascii_digit()) {
                prefix = Some(name);
            }
        }
    }
    if old_name.is_some() && prefix != old_name {
        anyhow::bail!(
            ".crate file '{}' does not match given old name '{}'",
            dot_crate.display(),
            old_name
                .as_ref()
                .expect("check for is_some in if conditional"),
        );
    }
    let old_name = old_name
        .or(prefix)
        .ok_or_else(|| anyhow::anyhow!("failed to infer current crate name"))?;

    let repackaged_fn = old_fn.replace(old_name, new_name);
    let repackaged_path = dot_crate.with_file_name(repackaged_fn);
    let repackaged = std::fs::File::create(&repackaged_path)?;
    let dot_crate_path = dot_crate;
    let dot_crate = std::fs::File::open(&dot_crate_path)?;

    // https://github.com/rust-lang/cargo/blob/8e075c9cab41eb1ed6222f819924999476477f2e/src/cargo/ops/cargo_package.rs#L481
    let dot_crate = flate2::read::GzDecoder::new(dot_crate);
    let mut dot_crate = tar::Archive::new(dot_crate);
    let repackaged = flate2::GzBuilder::new().write(repackaged, flate2::Compression::best());
    let mut repackaged = tar::Builder::new(repackaged);

    // We've got to be a little careful with replacements in .rs files.
    //
    // Imagine that a crate is called toml, and there's a struct field in the program called toml.
    // We obviously don't want to replace that, as it may be referenced elsewhere (might even be a
    // public field!). The same concern applies to both prefixes and suffixes.
    //
    // Luckily, crate names should only really show up in paths. That is, as crate_name::.
    // Teeechnically it can also show up as "use crate_name;" or "extern crate crate_name;", or
    // _even_ "extern crate crate_name as foobar;", but we're going to ignore those here since they
    // first two are trivial to fix in the code, and the last will break our renaming anyway.
    //
    // And for good measure, we also need to make sure the path is preceeded by a space, otherwise
    // our `toml` example would also rewrite
    //
    //     use foo_toml::bar;
    //
    // and
    //
    //     use foo::toml::bar;
    //
    // which we don't want. This will also still work in cases like toml::some_func(a).
    //
    // Unfortunately, it will _not_ work for anyone who tries to be fancy, such as by using tabs
    // over spaces, invoking macros by their full path at the top level of the file, or providing
    // full paths to functions and types as the first argument to a function
    // (`.any(toml::is_foo)`). Those _should_ be rare though, and keep in mind this does not apply
    // for files in `src/`, so let's consider it good enough until someone complains.
    let from = format!(" {}::", old_name.replace('-', "_"));
    let to = format!(" {}::", new_name.replace('-', "_"));

    // We also need to modify all paths inside the archive to start at new-name-0.1.0/ rather than
    // old-name-0.1.0/. This is simple enough as we're replacing the path wholesale.
    let old_base_dir = {
        let mut d = PathBuf::from(old_fn);
        d.set_extension("");
        d
    };
    let new_base_dir = {
        let mut d = PathBuf::from(old_fn.replace(old_name, new_name));
        d.set_extension("");
        d
    };

    let mut got_cargo_toml = false;
    let mut file_bytes = String::new();
    for file in dot_crate
        .entries()
        .context("walk entries from .crate file")?
    {
        let mut file = file.context("walk entry from .crate file")?;
        let mut header = file.header().clone();
        let path = file.path().context("get .crate file entry path")?;
        let sub_path = path.strip_prefix(&old_base_dir).map_err(|_| {
            anyhow::anyhow!(
                ".crate contained entry not under old crate subdir: {}",
                path.display()
            )
        })?;
        let path = new_base_dir.join(sub_path);

        if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
            // To avoid reading into memory we need:
            // https://github.com/alexcrichton/toml-rs/issues/215
            let mut toml_bytes = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut toml_bytes)
                .context("read Cargo.toml from .crate file")?;
            let mut manifest =
                Manifest::from_slice(&toml_bytes).context("parse Cargo.toml from .crate file")?;
            if manifest.workspace.is_some() {
                anyhow::bail!(".crate file is a workspace, so is not packaged");
            }
            let p = manifest.package.as_mut().ok_or_else(|| {
                anyhow::anyhow!("Cargo.toml in .crate file does not contain a package")
            })?;
            if &p.name != old_name {
                anyhow::bail!(
                    "crate name in .crate ('{}') file did not match given name ('{}')",
                    p.name,
                    old_name
                );
            }
            p.name = new_name.to_string();

            // Work around https://gitlab.com/crates.rs/cargo_toml/-/issues/3
            // See https://github.com/alexcrichton/toml-rs/issues/142#issuecomment-278970591
            let manifest =
                toml::Value::try_from(&manifest).context("serialize modified Cargo.toml")?;
            let bytes = toml::to_vec(&manifest).context("serialize modified Cargo.toml")?;
            let mut bytes = &bytes[..]; // to give us io::Read
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            repackaged
                .append_data(&mut header, path, &mut bytes)
                .context("append modified Cargo.toml to new .crate file")?;

            got_cargo_toml = true;
        } else if !path.starts_with("src") && path.extension().map(|e| e == "rs").unwrap_or(false) {
            // Replace previous_crate_name with new_crate_name.
            //
            // Binaries, tests, etc. will contain previous_crate_name:: paths, which we need to
            // re-write so that they still work after we change the top-level crate name. We
            // _could_ try to inject `extern crate previous_crate_name as new_crate_name`, but it
            // gets tricky as those can only be injected at the top-level crate entry point (and
            // only after //!, #!, /*!, etc.), so just straight up replacing is easier.
            //
            // Now, _technically_ this replacement shouldn't matter, since we're modifying a
            // package in a `.crate`, so any consumers should only be using the `lib` of the
            // current package anyway. And `lib` lives in `src/` (let's go ahead and assume they
            // haven't changed that) and refers to the current crate using `::` or `crate::`,
            // neither of which contain the current crate's name.
            //
            // But, we go for best effort anyway.

            // It would be nice if we could do the rewrite in a streaming fashion.
            // Unfortunately, doing so is tricky for two main reasons:
            //
            //  1) replacing the crate name changes the file size. We have to declare the size in
            //     the header, but we don't know how many replacements we're going to do until
            //     we've passed over the data!
            //  2) the crate name may appear at a chunk boundary.
            //
            // So, we just read the file into memory and then do the replacement(s) there.
            file_bytes.clear();
            file.read_to_string(&mut file_bytes)
                .context("read .rs file for in-place modification")?;

            let file_bytes = if file_bytes.contains(&from) {
                std::borrow::Cow::Owned(file_bytes.replace(&from, &to))
            } else {
                std::borrow::Cow::Borrowed(&file_bytes)
            };

            header.set_size(file_bytes.bytes().len() as u64);
            repackaged.append_data(&mut header, path, &mut file_bytes.as_bytes())?;
        } else {
            repackaged
                .append_data(&mut header, path, file)
                .context("append unmodified file to new .crate file")?;
        }
    }

    if !got_cargo_toml {
        let _ = std::fs::remove_file(repackaged_path);
        anyhow::bail!(
            ".crate file {} did not contain a Cargo.toml file",
            dot_crate_path.display()
        );
    }

    Ok(())
}
