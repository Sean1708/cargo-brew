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
            println!("`{}` failed: {}", $command, String::from_utf8_lossy(&o.stderr).trim());
            process::exit(o.status.code().unwrap_or(1));
        },
        Err(_) => panic!(format!("`{}` could not be run", $command)),
    })
}

// TODO: STOP PANICKING! It's going to be Ok!
fn main() {
    let cellar = process::Command::new("brew").arg("--cellar").output();
    let cellar = try_process!(cellar, "brew --cellar");
    let cellar = path::Path::new(cellar.trim());

    // Create a temporary directory to do the initial install into.
    // Randomly generated suffix is easier than the possibility of cleanup failing.
    let temp_cargo_brew_dir = format!("cargo-brew-{}", rand::random::<u32>());
    let temp_dir = env::temp_dir().join(temp_cargo_brew_dir);
    fs::create_dir(&temp_dir).expect("could not create temporary directory");
    let args = set_root(env::args(), temp_dir.to_str().expect("non-unincode temporary directory"));

    // TODO: we can run this and `brew --cellar` in parallel.
    // Install crate into temporary directory so that it can be moved to the Cellar later.
    // Inherits stdout and stderr.
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
            panic!("second `cargo install` succeeded");
        },
        Err(_) => panic!("`cargo install` could not be run"),
    };

    let brew_root = cellar.join(&krate).join(vers).join("bin");
    fs::create_dir_all(&brew_root).expect("could not create directories in Cellar");
    // TODO: maybe use push() instead of join()? might be useful for better error messages.
    // Loop through "temp_dir/bin" and copy the files into "brew_root/bin".
    match temp_dir.join("bin").read_dir() {
        Ok(rd) => for f in rd {
            // TODO: continue on Err but warn.
            let f = f.expect("io error");
            let name = f.file_name();
            fs::rename(f.path(), brew_root.join(name)).expect("unable to move binary into Cellar");
        },
        Err(_) => panic!("unable to open temporary bin directory"),
    };

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
    let re = regex::Regex::new(r"`(\S+) v([0-9.]+)").expect("statically known regex is invalid");
    let krate_vers = if let Some(caps) = re.captures(err) {
        let krate = caps.at(1).expect("could not determine crate name").to_owned();
        let vers = caps.at(2).unwrap_or("HEAD").to_owned();
        (krate, vers)
    } else {
        panic!("could not determine crate name");
    };

    krate_vers
}
