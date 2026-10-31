# Chapter 2 - Walking a folder

[< Chapter 1: Hello, Cargo](../01-hello-cargo/README.md)

Last chapter we looked at one thing. This time we look at everything: every file in a folder, every file in every folder inside it, all the way down, and add up how much space they take. That's the heart of Roomy. Everything later on (the tree, the threads, the treemap) is a better way of doing what we do in this chapter.

In this chapter you will

- read a folder with `fs::read_dir`,
- walk a whole tree of folders with a function that calls itself,
- pass errors up with `?`,
- return two values at once with a tuple,
- turn a byte count into "4.2 GB",
- write your first unit tests.

## Before we begin

There's still just one file, `src/main.rs`. It has a new function to walk a folder, another to format sizes, and some tests at the bottom.

Build and run it on a folder you know. Your Downloads folder is a good start.

```text
cd chapters/02-walking-a-folder
cargo run -- ~/Downloads
```

(On Windows, `cargo run -- $HOME\Downloads` in PowerShell.) You should see something like

```text
/Users/me/Downloads: 3.4 GB in 1288 files
```

Now try a folder you're not allowed to read everything in. On a Mac, your whole home folder will do it. On Linux, try `/`.

```text
cargo run -- ~
Error: Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }
```

One folder we weren't allowed into, and the whole scan gave up. Hold that thought. Fixing it is the next chapter.

## Breaking it up

### main can fail

```text
 6  fn main() -> io::Result<()> {
 7      let path = env::args().nth(1).unwrap_or_else(|| String::from("."));
 8      let (bytes, files) = walk(Path::new(&path))?;
 9      println!("{path}: {} in {files} files", format_size(bytes));
10      Ok(())
11  }
```

Last time `main` returned nothing. Now it returns `io::Result<()>`. You met `Result` in chapter 1: it's either `Ok(something)` or `Err(error)`. `io::Result<T>` is short for `Result<T, io::Error>`, a result whose error is the standard library's input/output error. And `()`, pronounced *unit*, is Rust's "nothing". It's a type with exactly one value, also written `()`. So `io::Result<()>` means "either it worked and there's nothing to report, or here's an I/O error".

When `main` returns an `Err`, Rust prints the error and exits with a failure code. That's what you saw above. When it returns `Ok(())` [line 10], the program exits cleanly. Notice there's no `return` and no semicolon on line 10. The last expression in a function, without a semicolon, is what the function returns. You'll see this everywhere in Rust.

### The question mark

The `?` at the end of line 8 is the most useful character in Rust. It means: if this is `Ok`, take the value out and carry on; if it's an `Err`, return that error from *this* function, right now. Without it, line 8 would be

```rust
let (bytes, files) = match walk(Path::new(&path)) {
    Ok(totals) => totals,
    Err(error) => return Err(error),
};
```

`?` is that, in one character. It only works in a function that returns a `Result` itself, which is why `main` had to change.

### Path and String

`Path::new(&path)` [line 8] turns our `String` into a `Path`. A path is mostly text, but not quite. On Linux a file name can contain bytes that aren't valid text at all, and on Windows paths are made of 16 bit characters. `Path` handles all of that, and has handy methods for joining and splitting paths. Rust's file functions want a `Path`, and `Path::new` makes one by *borrowing* the string, so nothing gets copied.

### Tuples

`walk` returns two numbers: the total size, and how many files it found. Rust lets a function return several values at once by bundling them in a *tuple*, written with brackets: `(bytes, files)`. On line 8, `let (bytes, files) = ...` takes the tuple apart again into two variables. A tuple is great for two or three values that travel together for a short while. When they start living longer, a `struct` with named fields is better. We'll make one in the next chapter.

### Walking the folder

Here's the walk.

```text
15  fn walk(path: &Path) -> io::Result<(u64, u64)> {
16      let mut bytes = 0;
17      let mut files = 0;
18      for item in fs::read_dir(path)? {
19          let item = item?;
20          let meta = item.metadata()?;
21          if meta.is_dir() {
22              let (b, f) = walk(&item.path())?;
23              bytes += b;
24              files += f;
25          } else {
26              bytes += meta.len();
27              files += 1;
28          }
29      }
30      Ok((bytes, files))
31  }
```

`path: &Path` says `walk` borrows a path. It looks at it, it doesn't keep it. `-> io::Result<(u64, u64)>` says it returns two unsigned 64 bit numbers, or an error. A `u64` goes up to about 18 billion billion, which is enough bytes for any disk you'll ever own.

`let mut` [lines 16 and 17] makes variables we're allowed to change. Rust variables can't change unless you say `mut`. It sounds fussy, but it means that when you read a function, anything without `mut` is settled the moment it's made, and you can stop thinking about it.

`fs::read_dir(path)?` [line 18] opens the folder and gives us its contents, one item at a time. Opening can fail (no such folder, not allowed), hence the `?`. Then each *item* can fail too, separately, which is why line 19 has another `?`. It's unusual, but it happens: a file can be deleted between the folder being opened and us getting to it.

Line 19 is worth a second look. `let item = item?;` makes a new variable called `item` that hides the old one. This is called *shadowing*, and it's perfectly normal in Rust. The old `item` was a `Result`; the new one is what was inside it. There's no point thinking up two names for the same thing.

`item.metadata()` [line 20] gets the size and type of the item. One thing to know: it doesn't follow symbolic links. A link to a folder looks like a small file, not a folder, so we don't walk into it. That's exactly what we want. A link can point back up to a folder we're already inside, and following it would go round in circles forever.

If the item is a folder [line 21], we call `walk` on it. A function that calls itself is called *recursive*. Each call adds up its own folder and hands the totals back to the call above it, which adds them to its own. When it reaches a folder with no folders in it, it stops calling itself, so it always finishes. `item.path()` makes a new `PathBuf`, an owned path, and `&` lends it to `walk`.

If it's anything else [line 25], we add its size and count it.

### Formatting sizes

Nobody wants to read "3409834612 bytes". `format_size` turns that into "3.4 GB".

```text
37  fn format_size(bytes: u64) -> String {
38      const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];
39
40      if bytes < 1000 {
41          return format!("{bytes} B");
42      }
43      let mut value = bytes as f64;
44      let mut unit = "B";
45      for u in UNITS {
46          value /= 1000.0;
47          unit = u;
48          if value < 1000.0 {
49              break;
50          }
51      }
52      if value < 10.0 {
53          format!("{value:.1} {unit}")
54      } else {
55          format!("{value:.0} {unit}")
56      }
57  }
```

`const` [line 38] makes a constant, fixed when the program is compiled. `[&str; 6]` is its type: an array of exactly six pieces of text. `&str` is text we borrow rather than own, and these six are baked into the program.

`format!` [line 41] works just like `println!`, except it gives you the text as a `String` instead of printing it. `return` leaves the function early. We only need it here, because everything else falls through to the `if` at the end.

`bytes as f64` [line 43] turns the whole number into a floating point one so we can have fractions. Rust never converts between number types behind your back. You always ask.

The loop divides by a thousand until the number is small enough. `{value:.1}` prints it with one decimal place, `{value:.0}` with none. So 1,536,000 bytes is "1.5 MB", but 42 billion is "42 GB" rather than "42.0 GB". The decimal place only earns its keep when the number is small.

Why a thousand and not 1024? Computers like 1024, because it's 2 to the power 10, and Windows still counts that way. But macOS, Ubuntu, and every disk manufacturer count in thousands, so a "1 TB" disk holds a million million bytes. If Roomy counted in 1024s, your 1 TB disk would come out as 931 "GB", and you'd spend a minute wondering where the rest went. Counting in thousands, the numbers match the box.

### Tests

At the bottom of the file there are tests.

```text
59  #[cfg(test)]
60  mod tests {
61      use super::*;
62
63      #[test]
64      fn small_numbers_are_bytes() {
65          assert_eq!(format_size(0), "0 B");
66          assert_eq!(format_size(999), "999 B");
67      }
```

`#[...]` is an *attribute*, a note to the compiler. `#[cfg(test)]` says the module below only exists when we're testing, so none of this ends up in the real program. `mod tests` makes a module, a named box of code, and `use super::*` brings in everything from the file above so the tests can see `format_size`. Each function marked `#[test]` is a test. `assert_eq!` checks that two things are equal, and fails the test with a helpful message if they're not.

Run them with

```text
cargo test
```

```text
running 2 tests
test tests::small_numbers_are_bytes ... ok
test tests::big_numbers_get_units ... ok

test result: ok. 2 passed; 0 failed
```

The second test checks the biggest number a `u64` can hold, `u64::MAX`. It's 18 exabytes. If your disk is ever that full, Roomy will at least tell you so correctly. Notice the underscores in `1_536_000` [line 72]. Rust ignores them; they're there so you can count the zeros.

## Adding functionality

Let's count folders too.

1. Add `let mut folders = 0;` next to the other two counters in `walk`.
2. Make `walk` return three numbers: change `(u64, u64)` to `(u64, u64, u64)` in its signature and `Ok((bytes, files))` to `Ok((bytes, files, folders))`.
3. In the folder branch, take three values from the recursive call, add them all, and add one more to `folders` for the folder itself.
4. In `main`, take three values apart and print the third.

The compiler will point out every place you've missed. Let it. That's what it's for.

## Exercise

Make the folder to scan default to your home folder instead of the current one.

*Hint: The `HOME` environment variable has it on macOS and Linux, and `USERPROFILE` on Windows. `env::var("HOME")` returns a `Result`, so `.ok()` turns it into an `Option` you can chain with `.or_else(...)`.*

## That's it

We can add up a folder. But one locked folder anywhere inside it and the whole thing falls over, which on a real disk is more or less always. Next we'll learn to expect things to go wrong, and to carry on when they do.

[Chapter 3: When things go wrong](../03-when-things-go-wrong/README.md)
