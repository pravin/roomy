# Chapter 9 - Race day

[< Chapter 8: Sizes that don't lie](../08-sizes-that-dont-lie/README.md)

Every Rust project gets to this point eventually. It works, the numbers are right, and you'd quite like to know if it's fast. Is it? Fast compared with what?

This chapter has two halves. First we give the command line tool proper options, with `--help` and `--version`, using `clap`, the crate nearly every Rust command line tool uses. That takes about fifteen lines. Then we race Roomy against `du`, the disk usage tool that's been on every Unix machine since the 1970s, and learn how to time things without fooling ourselves.

In this chapter you will

- describe command line options as a struct, and let `clap` do the rest,
- meet *derive macros* that do a lot more than `Debug`,
- build for release, and see what the release profile in `Cargo.toml` does,
- learn why the second run is always faster, and what to do about it,
- benchmark with `hyperfine`,
- find out how Roomy does against `du`, honestly.

## Before we begin

Only the command line tool changes.

- `Cargo.toml` (the workspace one) and `cli/Cargo.toml` add `clap`.
- `cli/src/main.rs` has a `Cli` struct describing its options, and passes `--cross-mounts` through to the scan.

The core and the app are the same as chapter 8. This is the CLI's chapter.

```text
cd chapters/09-race-day
cargo run --release -p roomy -- --help
```

```text
Find out what's filling your disk, and clear out developer junk

Usage: roomy [OPTIONS] [PATH]

Arguments:
  [PATH]  The folder to look at [default: .]

Options:
  -n, --top <TOP>     How many of the biggest things to show [default: 20]
      --cross-mounts  Follow into other drives and mounted file systems
  -h, --help          Print help
  -V, --version       Print version
```

We didn't write any of that text, apart from the descriptions.

## Breaking it up

### Options as a struct

Up to now the CLI has picked its folder out of `env::args_os()` by hand. That's fine for one argument. Add `-n 50`, `--top 50`, `--top=50`, a flag, `--help`, and good error messages for typos, and you're writing a parser. Somebody has already written a very good one.

```text
 7  use clap::Parser;
 8  use roomy_core::{Progress, ScanOptions, Tree, format_size};
 9
10  /// Find out what's filling your disk.
11  #[derive(Parser)]
12  #[command(version, about)]
13  struct Cli {
14      /// The folder to look at
15      #[arg(default_value = ".")]
16      path: PathBuf,
17
18      /// How many of the biggest things to show
19      #[arg(short = 'n', long, default_value_t = 20)]
20      top: usize,
21
22      /// Follow into other drives and mounted file systems
23      #[arg(long)]
24      cross_mounts: bool,
25  }
```

You describe what you want as an ordinary struct, and `#[derive(Parser)]` [line 11] writes the parser. That's a *derive macro*. We've used them since chapter 3: `#[derive(Debug)]` writes the code that prints a struct for debugging. `derive(Parser)` is the same idea, just much bigger. It reads the struct's fields, their types and their attributes when the program is compiled, and generates all the code to fill one in from the command line.

The field types do a lot of the work.

- `path: PathBuf` [line 16] has no `short` or `long`, so it's a *positional* argument: the first thing that isn't an option. `default_value = "."` makes it optional.
- `top: usize` [line 20] gets `-n` and `--top` [line 19]. Because it's a `usize`, `clap` parses the text into a number, and `roomy -n lots` gets a polite error instead of a crash. `default_value_t = 20` gives the default as a real `usize`, not as text.
- `cross_mounts: bool` [line 24] becomes a flag. It's `false` unless you write `--cross-mounts`. `clap` turns the underscore into a dash for you.

The `///` doc comments [lines 14, 18 and 22] become the help text. `#[command(version, about)]` [line 12] asks for `--version`, and for an "about" line at the top of the help. With no value given, both come from `Cargo.toml`: the version from the workspace's `version = "1.0.0"`, the about line from the `roomy` package's `description`. Change the description in `cli/Cargo.toml` and the help changes with it.

Then using it is one line.

```text
27  fn main() -> ExitCode {
28      let cli = Cli::parse();
29      match top(&cli.path, cli.top, cli.cross_mounts) {
```

`Cli::parse()` [line 28] reads the arguments, and either hands back a filled-in `Cli` or prints a message and exits. If you asked for `--help` it prints the help and exits with success. If you made a mistake it says what, prints the usage, and exits with an error. Our code never sees bad input.

`--cross-mounts` changes one of the recommended options.

```text
39  fn scan(path: &Path, cross_mounts: bool) -> Result<Tree, String> {
40      let mut options = ScanOptions::recommended();
41      if cross_mounts {
42          options.one_file_system = false;
43      }
```

`let mut` [line 40] because we might change it. Rust variables can't be changed unless you say so. It's a small thing that makes code much easier to read: anything without `mut` is what it was when it was made.

### Release builds

Before we time anything, a word about builds. `cargo build` and `cargo run` make a *debug* build. It's compiled quickly, with no optimisation and with extra checks, such as panicking if a number overflows. It's what you want while writing code. It can easily be ten times slower than a release build.

`--release` turns the optimiser on. The workspace's `Cargo.toml` asks for a bit more than the default.

```text
19  [profile.release]
20  lto = true
21  codegen-units = 1
22  strip = true
```

- `lto = true` [line 20] is *link time optimisation*. Normally each crate is optimised on its own. With LTO the optimiser looks at the whole program at once, including `rayon` and the standard library, and can inline functions across them.
- `codegen-units = 1` [line 21] stops the compiler splitting each crate into pieces to compile in parallel. It's slower to build and makes slightly better code.
- `strip = true` [line 22] removes debugging symbols from the finished program.

Together they make the release build take a while longer to compile, and they make a small, quick program. The `roomy` command line tool comes out at under 700 KB on my Mac. That's the whole thing, with the parallel scanner and the argument parser in it, and no runtime to install.

### The disk cache

Run the CLI twice on the same folder.

```text
cargo run --release -p roomy -- ~/Code
cargo run --release -p roomy -- ~/Code
```

The second run is faster, maybe a lot faster. That isn't Roomy getting better at its job. The operating system keeps recently read disk data in spare memory, called the *page cache* or *disk cache*. The first run reads folders from the disk. The second finds most of them already in memory.

So there are two races to run.

- **Warm cache**: run once to fill the cache, then time. This measures how fast the program is.
- **Cold cache**: empty the cache first. This measures what happens the first time you scan, which is what people really do. On Linux you can drop the cache with `sync; echo 3 | sudo tee /proc/sys/vm/drop_caches`. On macOS, `sudo purge`. On Windows a reboot is the most reliable way.

With a cold cache on a spinning hard disk, all the parallelism in the world won't help much, because the disk head can only be in one place. On an SSD, parallel reads are much faster than one at a time, and that's where Roomy shines.

### hyperfine

Timing a program by hand, once, tells you very little. The time changes from run to run, and the first run is special. `hyperfine` runs commands several times, warms the cache first if you ask, and reports the average and the spread. Install it with your package manager (`brew install hyperfine`, `cargo install hyperfine`, `winget install sharkdp.hyperfine`, or `apt install hyperfine`).

First build Roomy for release and put it somewhere easy to type.

```text
cargo build --release -p roomy
cp target/release/roomy ~/roomy
```

Then race.

```text
hyperfine --warmup 1 '~/roomy ~/Code' 'du -sh ~/Code'
```

`--warmup 1` runs each command once before timing, to fill the cache. Then it times ten runs of each, and at the end tells you which won and by how much.

### Results

Here's what I saw on my Mac, an Apple Silicon laptop with an SSD, scanning a projects folder with about 465,000 files in it, 19 GB.

| Tool | Time |
|---|---|
| `roomy ~/Code` | 2.7 s |
| `du -sk ~/Code` | 7.9 s |

That's roughly three times quicker, with a warm cache. And they agree: Roomy said 19 GB, `du` said 18,512,120 KB, which is 18.96 GB. I also scanned my whole home folder, 2.47 million files and 258 GB, in about 16 seconds.

Before you put that on a poster, some honesty.

- **Your numbers will be different.** More cores, a faster SSD, a different file system, a different mix of big and small files: all of it changes the result. A folder of a hundred huge videos is quick for everyone, because there's almost nothing to count.
- **`du` does one thing at a time, on purpose.** It's been the same since before computers had more than one core, and it's not trying to win races. Beating it isn't a great achievement. It's a sign that the parallel scan is doing its job.
- **Roomy isn't the only fast one.** There are excellent parallel disk usage tools written in Rust already, such as `dua`, `dust` and `pdu`, and in Go, such as `gdu`. On a terminal, they're all quick. Roomy's reason to exist is the treemap, the junk finder and the app around them, not shaving the last tenth of a second.
- **Roomy builds a tree, which `du` doesn't.** `du` adds up as it goes and forgets. Roomy keeps every file in memory so the app can show them. That costs memory, about 100 bytes or so per file, and some time. We're fast in spite of it.

What the race does show is that the design from chapters 5 and 6, an owned tree built in parallel and turned into an arena afterwards, was the right one. The tree costs us very little, and every core is busy.

## Adding functionality

`clap` can check values for you, too. Let's refuse `-n 0`, which makes no sense.

1. In `cli/src/main.rs`, change the attribute on `top` [line 19] to

   ```text
   #[arg(short = 'n', long, default_value_t = 20, value_parser = clap::value_parser!(usize).range(1..))]
   ```

2. Run `cargo run -p roomy -- -n 0`.

You get an error saying 0 isn't in the range `1..`, and the usage line. We didn't write an `if`.

## Exercise

Add a `--json` flag that prints the top entries as JSON instead of bars, so other programs can use Roomy's output. Each entry should have its name, its size in bytes and whether it's a folder.

*Hint: Add `serde` and `serde_json` to `cli/Cargo.toml` (they're already in the workspace). Make a small `#[derive(Serialize)]` struct for an entry, collect a `Vec` of them, and print `serde_json::to_string_pretty`. Send the "Scanned..." line to standard error, as it already is, so it doesn't end up in the JSON.*

## That's it

That's the scanner finished, and Part 2 with it. It's fast, it's right, and it talks to you while it works. Now it's time to make the app worth opening. In Part 3 we build a proper results screen, starting with a list you can click through, folder by folder.

[Chapter 10: The list view](../10-the-list-view/README.md)
