#[macro_use] extern crate userror;
extern crate rand;
extern crate regex;

use std::env;
use std::fs;
use std::path;
use std::process;

macro_rules! try_process {
    ($output:expr, $command:expr) => (match $output {
        Ok(ref o) => if o.status.success() {
            String::from_utf8_lossy(&o.stdout)
        } else {
            userror::error(&format!("`{}` failed: {}", $command, String::from_utf8_lossy(&o.stderr).trim()))
                .expect("failed to write error message");
            process::exit(o.status.code().unwrap_or(1));
        },
        Err(error) => userror::fatal(&format!("`{}` could not be run: {}", $command, error)),
    });
}

fn main() {
    // This is hardly going to be a bottleneck but it's not needed until later so we can happily
    // let it chug along in the background.
    let cellar = process::Command::new("brew").arg("--cellar")
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn();

    let cellar = match cellar {
        Ok(thread) => thread,
        Err(error) => userror::fatal(&format!("could not run `brew --cellar`: {}", error)),
    };

    // Create a temporary directory to do the initial install into.
    // Randomly generated suffix is easier than the possibility of cleanup failing.
    let temp_dir = format!("cargo-brew-{}", rand::random::<u32>());
    let temp_dir = env::temp_dir().join(temp_dir);
    expect!(fs::create_dir(&temp_dir), "could not create temporary directory");
    let args = set_root(
        env::args(),
        expect!(temp_dir.to_str(), "non-unincode temporary directory")
    );

    // Install crate into temporary directory so that it can be moved to the Cellar later.
    // Inherits stdout so user doesn't have to wait staring at a blank screen.
    let install = process::Command::new("cargo").arg("install").args(&args)
        .stdout(process::Stdio::inherit())
        .output();
    try_process!(install, "cargo install");

    // This is at best a massive hack and at worst something that will go horrendously wrong one day.
    // Essentially when trying to install something twice with `cargo install` it errors with:
    //
    //     binary `$PROG` already exists in destination as part of `$KRATE v$VERS`
    //
    // So this tries installing twice then parses the stderr for `$KRATE v$VERS`.
    //
    // TODO: This is the simplest way that I could think of without completely reinventing the
    // `cargo install` command but it is horrendously slow. Maybe completely reinventing the
    // `cargo install` command is the best way forward, or possibly `cargo install` could provide
    // us a way to get more information (like a `--dry-run` flag or something).
    let fail_on_purpose = process::Command::new("cargo").arg("install").args(&args).output();
    let (krate, vers) = match fail_on_purpose {
        Ok(ref o) => if !o.status.success() {
            parse_krate_vers_from_error(&String::from_utf8_lossy(&o.stderr))
        } else {
            userror::fatal("second `cargo install` succeeded");
        },
        Err(e) => userror::fatal(&format!("`cargo install` could not be run: {}", e)),
    };

    // Find user's Homebrew Cellar and create the directory structure.
    let cellar = cellar.wait_with_output();
    let cellar = try_process!(cellar, "brew --cellar");
    let cellar = path::Path::new(cellar.trim());
    let brew_root = cellar.join(&krate).join(vers).join("bin");
    expect!(fs::create_dir_all(&brew_root), "could not create directories in Cellar");

    // Loop through "temp_dir/bin" and copy the files into "brew_root/bin".
    let temp_dir = temp_dir.join("bin");
    match temp_dir.read_dir() {
        Ok(dir) => for file in dir {
            let file = match file {
                Ok(file) => file,
                Err(error) => {
                    userror::warn(&format!("could not read directory entry: {}", error))
                        .expect("failed to write error message");
                    continue
                },
            };
            let name = file.file_name();
            let old_path = file.path();
            let new_path = brew_root.join(name);
            if let Err(error) = fs::rename(&old_path, &new_path) {
                userror::warn(&format!("could not move binary '{:?}' to '{:?}': {}", old_path, new_path, error))
                    .expect("failed to write error message");
            };
        },
        Err(error) => userror::fatal(&format!("could not open '{:?}': {}", temp_dir, error)),
    };

    // kegs need to be unlinked before they can be linked again
    let brew_unlink = process::Command::new("brew").arg("unlink").arg(&krate).output();
    // unfortunately unlink will fail if the keg doesn't exist which it will be first time it is run
    match brew_unlink {
        Ok(output) => if !output.status.success() {
            userror::warn(&format!("keg {} could not be unlinked", krate))
                .expect("failed to write error message");
            userror::info("this should only happen the first time you install a crate")
                .expect("failed to write error message");
        },
        Err(error) => userror::fatal(&format!("`brew unlink {}` could not be run: {}", krate, error)),
    }

    let brew_link = process::Command::new("brew").arg("link").arg(&krate).output();
    try_process!(brew_link, format!("brew link {}", krate));
}

fn set_root(old_args: env::Args, temp_dir: &str) -> Vec<String> {
    let mut new_args = vec![];

    let mut skip = false;
    // Skip executable name and command name with skip(2).
    for arg in old_args.skip(2) {
        // Previous arg was `--root` so this arg is `/path/to/root` and shouldn't be pushed.
        if skip {
            skip = false;
        // Root specified as `--root /path/to/root` so skip next iteration as well.
        } else if &arg == "--root" {
            skip = true;
        // Root not specified as `--root=/path/to/root` so just push arg.
        } else if !arg.starts_with("--root") {
            new_args.push(arg);
        }
    }

    new_args.push(format!("--root={}", temp_dir));
    new_args
}

fn parse_krate_vers_from_error(err: &str) -> (String, String) {
    // Find the `$KRATE v$VERS part of the error message.
    let re = expect!(regex::Regex::new(r"`(\S+) v([0-9.]+)"), "statically known regex is invalid");
    let krate_vers = if let Some(caps) = re.captures(err) {
        let krate = expect!(caps.at(1), "could not determine crate name").to_owned();
        let vers = caps.at(2).unwrap_or("HEAD").to_owned();
        (krate, vers)
    } else {
        userror::fatal("could not determine crate name");
    };

    krate_vers
}
