use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Unwrap with [Display](std::fmt::Display).
#[macro_export]
macro_rules! unwrap {
    ($res:expr) => {{
        match $res {
            Ok(res) => res,
            Err(err) => ::std::panic!("{}", err),
        }
    }};
}

/// For debugging purposes.
/// Run command line interface in pipeline/command mode.
///
/// - Pipeline mode: Read source file content from [std::io::Stdin].
/// - Command mode: Read source file path from [std::env::Args].
pub fn cli(mut f: impl FnMut(&String, &Path) -> String) {
    let args = std::env::args();
    let args = args.skip(1);
    if args.len() == 0 {
        // Read from stdin.
        // Terminate with EOF.
        let mut buf = String::new();
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();

        let mut h_stdin = stdin.lock();
        unwrap!(h_stdin.read_to_string(&mut buf));
        let res = f(&buf, &PathBuf::new());
        let mut h_stdout = stdout.lock();
        unwrap!(writeln!(h_stdout, "{}", res));
    } else {
        for arg in args {
            let stdout = std::io::stdout();
            let mut h_stdout = stdout.lock();
            traverse_rpl(arg.into(), |path| {
                let buf = unwrap!(read_file_from_path_buf(&path));
                let res = f(&buf, &path);
                unwrap!(writeln!(h_stdout, "{res}"));
            });
        }
    }
}

fn is_rpl(path: &OsStr) -> bool {
    // debug_eprintln!("Checking if is .rpl: {:?}", path);
    PathBuf::from(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "rpl")
        .unwrap_or(false)
}

fn is_hidden(path: &OsStr) -> bool {
    // debug_eprintln!("Checking if is hidden: {:?}", path);
    path.to_str().map(|name| name.starts_with('.')).unwrap_or(true)
}

/// Convert [`String`] to [`PathBuf`].
pub fn into_mounted(path: String) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

/// Read file from path buffer to string.
pub fn read_file_from_path_buf(file: impl AsRef<Path> + Debug) -> io::Result<String> {
    eprintln!("Reading {:?}", file);
    let content = { std::fs::read_to_string(file)? };
    Ok(content)
}

/// Read file from string to string.
pub fn read_file_from_string(file: String) -> io::Result<String> {
    read_file_from_path_buf(into_mounted(file))
}

fn read_dir(dir: &PathBuf) -> Option<impl Iterator<Item = io::Result<(PathBuf, OsString)>>> {
    std::fs::read_dir(dir)
        .map(|dir| dir.map(|dir| dir.map(|dir| (dir.path(), dir.file_name()))))
        .ok()
}

fn traverse_dir(
    stack: &mut Vec<(PathBuf, OsString)>,
    dir: impl Iterator<Item = io::Result<(PathBuf, OsString)>>,
    path: &PathBuf,
) {
    eprintln!("Walking into {:?}", path);
    for entry in dir {
        match entry {
            Ok(entry) => stack.push(entry),
            Err(err) => eprintln!("Can't read entry under {:?} because of:\n{}", path, err),
        }
    }
}

/// Traverse all `.rpl` files under a repository.
pub fn traverse_rpl(root: PathBuf, mut f: impl FnMut(PathBuf)) {
    let mut stack: Vec<(PathBuf, OsString)> = vec![];

    if let Some(dir) = read_dir(&root) {
        traverse_dir(&mut stack, dir, &root);
    } else {
        // debug_eprintln!("Running {:?} because it's not a directory.", root);
        f(root);
    }

    while let Some(next) = stack.pop() {
        let (full, file) = next;
        if !is_hidden(&file) {
            if let Some(dir) = read_dir(&full) {
                traverse_dir(&mut stack, dir, &full);
            } else if is_rpl(&file) {
                eprintln!("{:?} is a normal file which ends with `.rpl`.", full,);
                f(full);
            } else {
                eprintln!("Skipped {:?}.", full);
            }
        }
    }
}

/// Collect all `.rpl` files under a repository.
pub fn collect_rpl(root: PathBuf) -> Vec<PathBuf> {
    let mut res = Vec::new();
    traverse_rpl(root, |p| res.push(p));
    res
}
