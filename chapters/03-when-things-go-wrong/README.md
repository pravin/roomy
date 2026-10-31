# Chapter 3 - When things go wrong

[< Chapter 2: Walking a folder](../02-walking-a-folder/README.md)

At the end of the last chapter we pointed Roomy at a home folder and it gave up. Somewhere in there was one folder it wasn't allowed to read, and one `?` passed the error all the way up and out of `main`. One locked door, and we left the building.

On a real disk there are always locked doors. Other people's folders, system folders, things macOS keeps private unless you say otherwise. A disk tool that stops at the first one is useless. So this chapter is about errors: which ones should stop us, which ones we should note and walk past, and how Rust makes you decide.

In this chapter you will

- tell the difference between errors that end a scan and errors that don't,
- make your own error type with an `enum`,
- teach it to print itself by implementing the `Display` trait,
- bundle the results in a `struct`,
- pass a `&mut` borrow down a recursive function,
- use `match`, `let ... else` and `ExitCode`,
- test the scanner on folders you make in the test itself.

## Before we begin

Still one file, `src/main.rs`, but most of it is new. `Cargo.toml` has gained a *dev dependency*, a crate we only use in tests.

```toml
[dev-dependencies]
tempfile = "3"
```

Cargo downloads it from [crates.io](https://crates.io), Rust's library of crates, the first time you run the tests. `"3"` means "any version 3.x", the latest that's compatible.

Build and run it on the folder that broke last time.

```text
cd chapters/03-when-things-go-wrong
cargo run --release -- ~
```

`--release` turns on the optimiser. The program takes longer to compile and runs much faster, which is starting to matter now that we're reading a whole home folder. You should see something like

```text
/Users/me: 214 GB in 2187402 files
(7 things couldn't be read)
```

It finishes, it counts what it couldn't read, and it tells you. Now try something that isn't a folder, and something that isn't there.

```text
cargo run -- Cargo.toml
roomy: Cargo.toml is not a folder
cargo run -- nowhere
roomy: can't read nowhere: No such file or directory (os error 2)
```

Those are proper error messages now, not a panic and a backtrace.

## Breaking it up

### Two kinds of error

The trick is to notice there are two different sorts of failure.

1. **The scan can't happen at all.** The folder you named doesn't exist, or it's a file. There's nothing sensible to report, so we stop and say why.
2. **Something inside the scan can't be read.** We'll count it, skip it, and carry on. You'd still like to know about the other 213 GB.

Rust doesn't let you ignore either sort. Every function that can fail returns a `Result`, so we have to decide, in code, what each failure means. That's what this chapter does.

### Our own error type

Here's the first sort, as a type.

```text
35  #[derive(Debug)]
36  enum ScanError {
37      Io {
38          path: PathBuf,
39          source: std::io::Error,
40      },
41      NotAFolder(PathBuf),
42  }
```

An `enum` is a type that is exactly one of a list of *variants*. Rust's enums are more powerful than those in C or Java, because each variant can carry its own data. `Io` carries the path we couldn't read and the error the operating system gave us. `NotAFolder` carries the path that turned out to be a file. A `ScanError` is always one or the other, never both, never neither.

You've already used two enums without knowing it. `Option` is an enum with variants `Some(value)` and `None`, and `Result` is one with `Ok(value)` and `Err(error)`. There's no magic in them. They're ordinary enums in the standard library.

`#[derive(Debug)]` asks the compiler to write the code that prints a `ScanError` for programmers, the way `{:?}` prints things. `derive` writes boring code for you, and we'll use it a lot.

`PathBuf` is to `Path` what `String` is to `str`: the owned version. The error keeps its path after the function that made it has gone, so it has to own it.

### Printing it nicely

`Debug` output is for us. People want a sentence. For that, a type implements `Display`.

```text
45  impl fmt::Display for ScanError {
46      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
47          match self {
48              ScanError::Io { path, source } => write!(f, "can't read {}: {source}", path.display()),
49              ScanError::NotAFolder(path) => write!(f, "{} is not a folder", path.display()),
50          }
51      }
52  }
53
54  impl std::error::Error for ScanError {}
```

`Display` is a *trait*. A trait is a set of methods a type promises to have. If you've used interfaces in Java, C# or Go, it's the same idea. `impl fmt::Display for ScanError` keeps that promise for our error by writing the one method `Display` asks for, `fmt`. From then on, `println!("{error}")` works, because `{}` means "use `Display`".

`&self` is the error being printed, borrowed. `match` [line 47] looks at which variant it is and runs the matching arm. In each arm we pull the variant's data out into variables (`path`, `source`) and write a sentence with `write!`, which is `format!` aimed at the formatter instead of a new string.

`match` must cover every variant. Add a third kind of `ScanError` and forget to say how to print it, and the program won't compile. That's the sort of thing that's annoying for ten seconds and saves you an embarrassing bug.

`path.display()` is there because a `Path` doesn't implement `Display` itself. Remember that a path might not be valid text. `display()` gives you something printable, swapping any bytes it can't show for a placeholder. It's honest about the problem rather than hiding it.

Line 54 says `ScanError` is a proper error type. The `Error` trait's methods all have defaults, so there's nothing to write in the braces. It means other code can treat our error like any other.

### The totals

Chapter 2 returned a tuple of two numbers. We need three now, and that's when a tuple stops being clear. Was it `(bytes, files, errors)` or `(files, bytes, errors)`?

```text
57  #[derive(Debug, Default, PartialEq)]
58  struct Totals {
59      bytes: u64,
60      files: u64,
61      /// Things we weren't allowed to read.
62      errors: u64,
63  }
```

A `struct` groups named values. Three things derived: `Debug` for printing, `Default` so `Totals::default()` gives us one with every field zero, and `PartialEq` so tests can compare two `Totals` with `==`. Comments starting `///` are *doc comments*. They show up when you hover over the field in your editor, and in generated documentation.

### Scanning

```text
66  fn scan(root: &Path) -> Result<Totals, ScanError> {
67      let meta = fs::metadata(root).map_err(|source| ScanError::Io {
68          path: root.to_path_buf(),
69          source,
70      })?;
71      if !meta.is_dir() {
72          return Err(ScanError::NotAFolder(root.to_path_buf()));
73      }
74      let mut totals = Totals::default();
75      walk(root, &mut totals);
76      Ok(totals)
77  }
```

`scan` returns our own error type, so the `io::Error` from `fs::metadata` won't do as it is. `map_err` [line 67] turns it into one of ours. It runs the closure only if the result is an `Err`, and the closure wraps the I/O error in a `ScanError::Io` along with the path. Line 69 says just `source` rather than `source: source`; when a variable has the same name as the field, Rust lets you write it once.

Then `?` [line 70] does its usual job: return the error if there is one.

These are the only two ways a scan can fail as a whole: the root isn't there (or we can't look at it), or it isn't a folder. Everything after that, `walk` handles itself.

### Lending something you can change

```text
74      let mut totals = Totals::default();
75      walk(root, &mut totals);
```

In chapter 2 each call of `walk` made its own totals and returned them, and the caller added them up. That works, but now there are three numbers to add at every level. Instead we make one `Totals` and lend it to `walk` to fill in.

`&mut totals` is a *mutable borrow*. `&` lends something to read; `&mut` lends it to change. Rust has one rule about this that it never bends: while something is lent as `&mut`, nothing else can look at it at all. One writer, or any number of readers, never both. It seems strict for a single-threaded program like this one. In chapter 6, when we split the scan across every core, it's the rule that will make the compiler stop us before we write a bug, instead of us finding it at three in the morning.

### Walking past locked doors

```text
79  fn walk(path: &Path, totals: &mut Totals) {
80      let items = match fs::read_dir(path) {
81          Ok(items) => items,
82          Err(_) => {
83              totals.errors += 1;
84              return;
85          }
86      };
```

`walk` no longer returns a `Result`. It can't fail, because it doesn't let failures out. If we can't read a folder [line 82], we count it and return. `_` means "there's a value here and I don't care what it is".

`match` is an expression. It has a value, so we can put it on the right of `let`. If `read_dir` worked, the match is worth `items`; if not, we leave the function, so there's no value needed.

```text
87      for item in items {
88          let Ok(item) = item else {
89              totals.errors += 1;
90              continue;
91          };
92          // This doesn't follow symbolic links, which is what we want.
93          let Ok(meta) = item.metadata() else {
94              totals.errors += 1;
95              continue;
96          };
```

`let ... else` [lines 88 and 93] is a shorter way to say the same thing. `let Ok(item) = item` tries to match the pattern `Ok(item)`. If it matches, `item` is the value inside. If it doesn't, the `else` block runs, and the `else` block isn't allowed to carry on as if nothing happened: it has to leave, with `return`, `continue` or `break`. The compiler checks. So after line 91 you know for certain you have a good `item`.

The rest is chapter 2 again, except that the recursive call [line 98] passes the same `totals` down. It's already a `&mut Totals`, so we hand it on as it is.

### main

```text
 7  fn main() -> ExitCode {
 8      let path = env::args_os()
 9          .nth(1)
10          .map(PathBuf::from)
11          .unwrap_or_else(|| PathBuf::from("."));
```

Two small changes here. `env::args_os` [line 8] gives us the arguments as `OsString`s, raw operating system strings, instead of `String`s. `env::args` panics if an argument isn't valid text, and a file name is allowed not to be. `.map(PathBuf::from)` turns the `Option<OsString>` into an `Option<PathBuf>`. `map` on an `Option` runs a function on the value inside, if there is one. Here the function is `PathBuf::from` itself. You can pass a function anywhere a closure would go.

```text
13      match scan(&path) {
14          Ok(totals) => {
...
24              ExitCode::SUCCESS
25          }
26          Err(error) => {
27              eprintln!("roomy: {error}");
28              ExitCode::FAILURE
29          }
30      }
```

`main` returns an `ExitCode` now rather than a `Result`, so we choose exactly what's printed. Returning a `Result` from `main`, as in chapter 2, prints the error with `Debug`, which is for programmers. `eprintln!` [line 27] prints to *standard error* rather than standard output. Errors belong there, so that someone piping Roomy's output into another program doesn't get error messages mixed in with the results.

### Tests that make their own folders

```text
150      #[test]
151      fn adds_up_every_file() {
152          let dir = tempfile::tempdir().unwrap();
153          fs::create_dir(dir.path().join("a")).unwrap();
154          fs::write(dir.path().join("one"), vec![0u8; 1000]).unwrap();
155          fs::write(dir.path().join("a/two"), vec![0u8; 2000]).unwrap();
156
157          let totals = scan(dir.path()).unwrap();
```

`tempfile::tempdir()` makes an empty folder somewhere temporary. When `dir` goes out of scope at the end of the test, the folder and everything in it are deleted. Rust runs clean-up code when a value goes away, and `tempdir` uses that to tidy up after itself even if the test fails. In the test we build a small folder tree with known sizes, scan it, and check the answer is exactly right. `vec![0u8; 1000]` is a list of a thousand zero bytes. `unwrap()` is `expect` without a message. In tests that's fine: if setting up fails, the test should fail.

```text
173          assert!(matches!(scan(&path), Err(ScanError::NotAFolder(_))));
```

`matches!` checks that a value fits a pattern. We don't care which path is in the error, only that it's the right kind.

The last test [lines 176 to 187] makes a folder nobody can read (`0o000` is "no permissions at all", in the octal notation Unix uses), scans around it, and checks it was counted rather than fatal. `#[cfg(unix)]` means it's only compiled on macOS and Linux; Windows permissions work differently. Note that it puts the permissions back *before* checking the result. If the assert failed first, the temporary folder couldn't be cleaned up.

Run them all with `cargo test`.

## Adding functionality

Let's count the folders we skipped separately from the files we couldn't read.

1. Add a `locked: u64` field to `Totals`.
2. In `walk`, where `read_dir` fails, add one to `locked` instead of `errors`.
3. In `main`, print the folders and the other errors on separate lines.
4. Fix the test that compares whole `Totals`. The compiler will find it for you.

## Exercise

Make Roomy print the path of each folder it couldn't read, as it goes, on standard error.

*Hint: In the `Err(_)` arm of the `match` in `walk`, give the error a name instead of `_`, and use `eprintln!` with `path.display()` and the error.*

## That's it

Roomy can scan a real disk now and live with what it finds there. It's still a command line program, though, and it only gives you one number. In the next chapter we split it into a library and two programs, and give it a window.

[Chapter 4: A window of your own](../04-a-window-of-your-own/README.md)
