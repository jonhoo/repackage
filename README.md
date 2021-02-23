[![Build Status](https://dev.azure.com/jonhoo/jonhoo/_apis/build/status/repackage?branchName=master)](https://dev.azure.com/jonhoo/jonhoo/_build/latest?definitionId=32&branchName=master)
[![Codecov](https://codecov.io/github/jonhoo/repackage/coverage.svg?branch=master)](https://codecov.io/gh/jonhoo/repackage)
[![Crates.io](https://img.shields.io/crates/v/repackage.svg)](https://crates.io/crates/repackage)
[![Documentation](https://docs.rs/repackage/badge.svg)](https://docs.rs/repackage/)

Repackage Rust `.crate` files under a different crate name.

This crate provides `repackage::dot_crate`, which repackages a `.crate`
file so that it exports the same crate under a different name. It
replaces the `name` attribute in `Cargo.toml`, and also rewrites
references to the old name in the various `.rs` files that live outside
of `src/` (those in `src/` use `crate::`).

See the library documentation for details.

Use with extreme caution.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
