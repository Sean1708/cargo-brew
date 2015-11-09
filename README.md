# cargo-brew

Easily integrate `cargo install` installed binaries into Homebrew!

## Installation

I'll make a brew formula.

## Usage

cargo-brew currently passes all arguments straight through to `cargo install` and therefore supports
all arguments that `cargo install` does, except for `--root` since cargo-brew uses that to install
things to the right place.

Installing a program is as simple as

    cargo brew --git https://github.com/nrc/rustfmt

and uninstalling as simple as

    cargo unbrew rustfmt
