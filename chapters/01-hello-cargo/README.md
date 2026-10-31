# Chapter 1 - Hello, Cargo

Your disk is full again. You didn't put 400 GB of anything on it, and yet here we are. Something is eating it, and the tools that come with your computer are vague about what.

Over twenty chapters we'll fix that by building **Roomy**, a fast desktop app that scans a disk, draws what it finds as a treemap, and hunts down the developer junk (`node_modules`, `target/`, `.venv`, package caches) that you can throw away. We'll write it in Rust, and you don't need to know any Rust to start. By chapter 20 Roomy runs on macOS, Windows and Linux, updates itself, and installs from Homebrew, winget, Scoop, Flathub, the AUR and crates.io.

We start small. Very small.

In this chapter you will

- install Rust and set up an editor,
- make a project with Cargo and run it,
- read the command line and ask the operating system about a file or folder,
- meet `let`, `String`, `Result`, `expect` and `println!`.

## Before we begin

You should have programmed before, in any language. If you know what a variable, a function and a loop are, you're ready. You don't need to know anything about Rust, or about how disks and file systems work. That's what we're here for.

Every chapter is a folder you can build on its own. This one is `chapters/01-hello-cargo`. Chapter 2 starts from a copy of chapter 1 and adds to it, chapter 3 starts from chapter 2, and so on, so the last folder is the finished program. If you get stuck, open the next folder and see how I did it.

## Install Rust

Rust is installed with a small tool called **rustup**. It downloads the compiler, keeps it up to date, and lets you switch versions if you ever need to.

### macOS and Linux

Open a terminal and type

```text
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Accept the defaults. When it finishes, open a new terminal (so it picks up the new `PATH`) and check

```text
rustc --version
cargo --version
```

On macOS you'll also want Apple's command line tools, for the linker. If you've ever used `git` you probably have them. If not, `xcode-select --install`. On Linux, make sure you have a C compiler (`sudo apt install build-essential` on Debian and Ubuntu).

### Windows

Download and run **rustup-init.exe** from [rustup.rs](https://rustup.rs). It will tell you that you need the Visual Studio C++ build tools if you don't have them, and offer to install them. Say yes. (You want the *Desktop development with C++* workload, or just the *MSVC build tools* and a *Windows SDK*.) Then open a new PowerShell and check `rustc --version` as above.

### An editor

Use whatever you like. If you have no preference, use [VS Code](https://code.visualstudio.com) with the **rust-analyzer** extension. It shows you types as you write, points at mistakes before you compile, and jumps to the definition of anything you click. Rust has a fussy compiler, and an editor that tells you what it's going to fuss about is worth a lot.

## Cargo

You'll hardly ever call the Rust compiler, `rustc`, yourself. Cargo calls it for you. Cargo is Rust's build tool and package manager rolled into one: it makes new projects, compiles them, runs them, tests them, and downloads the libraries (Rust calls them *crates*) they use.

To make a new project you'd type

```text
cargo new roomy
```

That gives you a folder with two things in it. `Cargo.toml` describes the project, and `src/main.rs` is the program. I've done that already in `chapters/01-hello-cargo`. Here's `Cargo.toml`.

```toml
[package]
name = "roomy"
version = "0.1.0"
edition = "2024"
```

The name is what the program will be called. The edition is the version of the Rust language we're writing. Editions come out every three years or so, and they let Rust tidy up old mistakes without breaking anybody's code: each crate says which edition it's written in, and crates of different editions work together fine.

## The code

Here it is. All of it.

```text
 1  use std::env;
 2  use std::fs;
 3
 4  fn main() {
 5      println!("Hello, Roomy!");
 6
 7      // The first thing typed after the program's name, or "." (the current
 8      // folder) if there isn't one.
 9      let path = env::args().nth(1).unwrap_or_else(|| String::from("."));
10
11      let meta = fs::metadata(&path).expect("couldn't look at that");
12      if meta.is_dir() {
13          println!("{path} is a folder");
14      } else {
15          println!("{path} is a file of {} bytes", meta.len());
16      }
17  }
```

Build it and run it from the chapter folder.

```text
cd chapters/01-hello-cargo
cargo run
```

The first time, Cargo compiles the program and then runs it.

```text
   Compiling roomy v0.1.0 (.../chapters/01-hello-cargo)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.61s
     Running `target/debug/roomy`
Hello, Roomy!
. is a folder
```

Now give it something to look at. Anything after `--` goes to our program instead of to Cargo.

```text
cargo run -- Cargo.toml
Hello, Roomy!
Cargo.toml is a file of 60 bytes
```

Congratulations! You have just written your first Rust program. It doesn't find out what's filling your disk yet, but it can already ask the operating system about a file, which is where all of that starts.

The compiled program is in `target/debug/roomy` (`roomy.exe` on Windows). You can run it directly too. `target/` is where Cargo keeps everything it builds. It can get big, and it's one of the things Roomy will be finding for you later.

## Breaking it up

### Bringing names into scope

```text
 1  use std::env;
 2  use std::fs;
```

`std` is Rust's standard library. It comes with every Rust program, and it's organised into *modules*: `std::env` is about the program's environment (its arguments and environment variables), `std::fs` is about the file system. A `use` line brings a module's name into scope, so we can write `env::args()` instead of `std::env::args()` every time. Nothing gets copied or loaded. It's purely about names.

### main

```text
 4  fn main() {
```

Every Rust program starts at `main`. `fn` declares a function. This one takes no arguments and returns nothing, so there's no return type. When `main` returns, the program ends.

### println!

```text
 5      println!("Hello, Roomy!");
```

`println!` prints a line. The `!` means it's a *macro*, not a function. A macro writes code for you at compile time. `println!` is a macro because it can do something no ordinary Rust function can: take any number of arguments of any type, and check, while compiling, that they match the `{}` placeholders in the text. Get them wrong and the program doesn't build, rather than printing nonsense later.

### Reading the command line

```text
 9      let path = env::args().nth(1).unwrap_or_else(|| String::from("."));
```

There's a lot packed into this line, so let's go left to right.

`let` makes a new variable. We didn't say what type it is; Rust worked it out from the right-hand side. (Hover over `path` in VS Code and rust-analyzer will tell you: it's a `String`.) Variables in Rust can't be changed after they're set unless you ask for that with `let mut`. We'll need `mut` in the next chapter.

`env::args()` gives us the command line, one piece at a time. The first piece is always the program's own name, so the folder we want is the second, `nth(1)`, counting from zero.

But what if there isn't a second piece? Lots of languages would hand you `null` and let you find out the hard way. Rust doesn't have `null`. Instead, `nth` returns an `Option`, which is either `Some(value)` or `None`, and the compiler won't let you use the value until you've said what to do about `None`. `unwrap_or_else` says it: if there's a value, use it, and otherwise call this little function to make one.

`|| String::from(".")` is that little function, a *closure*. The `||` is its (empty) list of arguments, and the rest is what it returns. `"."` means the current folder. `String::from` turns the text `"."`, which is baked into the program, into a `String`, which is text the program owns and can change. Both arms have to give the same type, and `nth` gives a `String`, so we make one.

Why `unwrap_or_else` with a closure, and not the simpler `unwrap_or(String::from("."))`? `unwrap_or` makes its default every time, even when it isn't needed. `unwrap_or_else` only calls the closure when it has to. For one tiny string it makes no difference at all, but it's a habit worth having.

### Asking about a file

```text
11      let meta = fs::metadata(&path).expect("couldn't look at that");
```

`fs::metadata` asks the operating system what it knows about a path: is it a file or a folder, how big is it, when was it changed. It doesn't open the file or read it, so it's quick.

Two new things. First, `&path`. The `&` means we *lend* `path` to `fs::metadata` instead of giving it away. This is Rust's big idea, and we'll keep meeting it: every value has one owner, and when you pass a value to a function, ownership moves to that function unless you lend it with `&`. `fs::metadata` only needs to look at the path for a moment, so a loan is all it gets. We get to keep `path` and use it again on line 13.

Second, `expect`. Asking the operating system can go wrong. The path might not exist, or we might not be allowed to look. So `fs::metadata` returns a `Result`, which is either `Ok(metadata)` or `Err(error)`, and, just like `Option`, Rust won't let us use the metadata until we've dealt with the error. `expect` deals with it bluntly: if it's `Ok`, give me what's inside; if it's `Err`, stop the program with this message.

```text
cargo run -- no-such-file
Hello, Roomy!
thread 'main' panicked at src/main.rs:11:36:
couldn't look at that: Os { code: 2, kind: NotFound, message: "No such file or directory" }
```

Stopping like this is called a *panic*. It's fine for a first program. It's not fine for a product, and in chapter 3 we'll handle errors properly.

### Folder or file?

```text
12      if meta.is_dir() {
13          println!("{path} is a folder");
14      } else {
15          println!("{path} is a file of {} bytes", meta.len());
16      }
```

`is_dir` and `len` are *methods* on the metadata: functions that belong to a type and are called with a dot. Note there are no brackets around the `if` condition. Rust doesn't want them.

`{path}` in the text is replaced by the variable `path`. The bare `{}` is replaced by the next argument, `meta.len()`, the file's size in bytes.

## Adding functionality

Let's have Roomy say how big the file is in kilobytes too.

1. After line 15, add a line that divides `meta.len()` by 1000.
2. Print it with `println!("That's {} KB", meta.len() / 1000);`.
3. Run it on a big file, say a photo in your Downloads folder.

You'll notice the answer is always a whole number. `len()` returns a `u64`, an unsigned 64 bit integer, and dividing integers gives an integer. Try `meta.len() as f64 / 1000.0` to get a fraction instead. Rust never turns integers into floating point numbers behind your back; you always ask with `as`. We'll do this properly, with MB and GB, in the next chapter.

## Exercise

Make Roomy say whether the path is a symbolic link (a shortcut that points somewhere else). Try it on a link you make with `ln -s Cargo.toml link` (on Windows, `mklink link Cargo.toml` in an administrator Command Prompt).

*Hint: `fs::metadata` follows links and tells you about the thing at the other end. `fs::symlink_metadata` doesn't. Its result has `file_type().is_symlink()`.*

## That's it

That was short. Next we'll stop looking at one thing and start looking at everything inside a folder, all the way down, and add it all up.

[Chapter 2: Walking a folder](../02-walking-a-folder/README.md)
