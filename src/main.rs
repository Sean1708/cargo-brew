extern crate rand;
extern crate regex;

use std::env;
use std::fs;
use std::path;
use std::process;

use std::error::Error;

macro_rules! try_process {
    ($output:expr, $command:expr) => (match $output {
        Ok(ref o) => if o.status.success() {
            String::from_utf8_lossy(&o.stdout)
        } else {
            println!("`{}` failed: {}", $command, String::from_utf8_lossy(&o.stderr).trim());
            process::exit(o.status.code().unwrap_or(1));
        },
        Err(e) => panic!("`{}` could not be run: {}", $command, e.description()),
    });
}

macro_rules! msg_file_line {
    ($msg:expr) => (concat!($msg, ":", file!(), ":", line!()));
}

fn main() {
    // This is hardly going to be a bottleneck but it's not needed until later so we can happily
    // let it chug along in the background.
    let cellar = process::Command::new("brew").arg("--cellar")
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()
        .expect(msg_file_line!("could not run `brew install`"));

    // Create a temporary directory to do the initial install into.
    // Randomly generated suffix is easier than the possibility of cleanup failing.
    let temp_dir = format!("cargo-brew-{}", rand::random::<u32>());
    let temp_dir = env::temp_dir().join(temp_dir);
    fs::create_dir(&temp_dir).expect(msg_file_line!("could not create temporary directory"));
    let args = set_root(env::args(), temp_dir.to_str().expect("non-unincode temporary directory"));

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
            panic!("second `cargo install` succeeded");
        },
        Err(e) => panic!("`cargo install` could not be run: {}", e.description()),
    };

    // Find user's Homebrew Cellar and create the directory structure.
    let cellar = cellar.wait_with_output();
    let cellar = try_process!(cellar, "brew --cellar");
    let cellar = path::Path::new(cellar.trim());
    let brew_root = cellar.join(&krate).join(vers).join("bin");
    fs::create_dir_all(&brew_root).expect(msg_file_line!("could not create directories in Cellar"));

    // Loop through "temp_dir/bin" and copy the files into "brew_root/bin".
    let temp_dir = temp_dir.join("bin");
    match temp_dir.read_dir() {
        Ok(dir) => for file in dir {
            let file = match file {
                Ok(file) => file,
                Err(e) => {
                    println!("Warning: could not read directory entry: {}", e.description());
                    continue
                },
            };
            let name = file.file_name();
            let old_path = file.path();
            let new_path = brew_root.join(name);
            if let Err(e) = fs::rename(&old_path, &new_path) {
                println!("Warning: could not move binary '{:?}' to '{:?}': {}", old_path, new_path, e.description());
            };
        },
        Err(_) => panic!("unable to open '{:?}'", temp_dir),
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
        let krate = caps.at(1).expect(msg_file_line!("could not determine crate name")).to_owned();
        let vers = caps.at(2).unwrap_or("HEAD").to_owned();
        (krate, vers)
    } else {
        panic!("could not determine crate name");
    };

    krate_vers
}
