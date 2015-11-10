# cargo-brew

Easily integrate `cargo install` installed binaries into Homebrew!

## Installation

Unfortunately you'll have to use `cargo install` just this once:

    $ cargo install --git https://github.com/Sean1708/cargo-brew --root $(brew --cellar)/cargo-brew/0.1.1
    $ brew link cargo-brew

Alternatively, in a one-liner:

    $ cargo install --git https://github.com/Sean1708/cargo-brew --root $(brew --cellar)/cargo-brew/0.1.1 && brew link cargo-brew

## Usage

cargo-brew currently passes all arguments straight through to `cargo install` and therefore supports
all arguments that `cargo install` does, except for `--root` since cargo-brew uses that to install
things to the right place.

In theory cargo-brew should remove any `--root` options that you pass but in practice this hasn't
been thoroughly tested. At best it'll have no effect but at worst you'll royally screw things up, so
just don't bother.

Installing a program is as simple as

    cargo brew --git https://github.com/nrc/rustfmt

and uninstalling as simple as

    brew uninstall rustfmt
