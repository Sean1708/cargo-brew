**All new development on cargo-brew is being done by Kornelski at https://github.com/kornelski/cargo-brew.**

# cargo-brew

Easily integrate `cargo install` installed binaries into Homebrew!

## Installation

Unfortunately you'll have to use `cargo install` just this once:

    $ cargo install cargo-brew --root $(brew --cellar)/cargo-brew/0.1.2
    $ brew link cargo-brew

## Usage

cargo-brew currently passes all arguments straight through to `cargo install` and therefore supports
all arguments that `cargo install` does, except for `--root` since cargo-brew uses that to install
things to the right place.

In theory cargo-brew should remove any `--root` options that you pass but in practice this hasn't
been thoroughly tested. At best it'll have no effect but at worst you'll royally screw things up, so
just don't bother.

Installing a program is as simple as:

    $ cargo brew --git https://github.com/rust-lang-nursery/rustfmt

and uninstalling as simple as:

    $ brew uninstall rustfmt

## Upgrading

To upgrade cargo-brew, simply `cargo brew` cargo-brew and cargo-brew will be `cargo brew`ed into the
latest version number:

    $ cargo brew cargo-brew
