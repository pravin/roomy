# Chapter 8 - Sizes that don't lie

[< Chapter 7: Live progress](../07-live-progress/README.md)

Here's a question that sounds too easy: how big is a file?

Make a text file with one letter in it. Its length is 1 byte. But the disk doesn't hand out space one byte at a time. It hands it out in blocks, usually 4096 bytes, so that one letter is taking up 4 KB. Now make a hundred thousand of those, which is roughly what a JavaScript project's `node_modules` is, and the two ways of counting are hundreds of megabytes apart.

It gets worse. Some files live in two folders at once. Some disks are mounted inside other disks. And on a Mac, scanning `/` finds your whole home folder twice. A disk analyser that gets these wrong will tell you things that aren't true, and people will notice, because they'll compare it with Finder or Explorer or `du`.

This chapter makes Roomy's numbers honest. When we're done, they agree with `du`.

In this chapter you will

- learn the difference between a file's length and the space it uses on disk,
- write code that only exists on some operating systems, with `#[cfg]`,
- count a hard-linked file once, with a `HashSet` behind a `Mutex`,
- keep the scan on one disk, and leave out folders that aren't really there,
- bundle the choices up in a `ScanOptions` struct that `serde` can save later,
- put a lifetime on a struct, and find out it isn't scary.

## Before we begin

Nearly all the work is in `core/src/scan.rs`.

- It gains `ScanOptions`, a `Context` struct, a `first_sighting` function, and two versions of a `platform` module.
- `scan` takes a `&ScanOptions`.
- `core/src/lib.rs` exports `ScanOptions`.
- `cli/src/main.rs` and `app/src-tauri/src/commands.rs` pass in `ScanOptions::recommended()`.

There are two new tests, for excluded folders and for hard links.

```text
cd chapters/08-sizes-that-dont-lie
cargo test -p roomy-core
cargo run --release -p roomy -- ~/Code
```

Compare the total with `du`, which counts space on disk in kilobytes.

```text
du -sk ~/Code
```

On my machine Roomy said 19 GB and `du` said 18,512,120 KB. That's 18.96 GB in Roomy's thousands, which rounds to 19. Last chapter's Roomy said something rather different. The app looks the same as before, with better numbers in it.

## Breaking it up

### Length and size on disk

Up to now we've used `meta.len()`, the file's length. That's the number of bytes you'd get if you read the file from start to end. It's what you care about if you're emailing the file to someone. When your disk is full, what you care about is how much *disk* the file is using.

Unix keeps that number too. It's in the file's metadata as a count of blocks.

```text
236      /// Space actually used on disk. Unix counts it in 512 byte blocks, no
237      /// matter how big the file system's real blocks are.
238      pub fn disk_size(meta: &Metadata) -> u64 {
239          meta.blocks() * 512
240      }
```

The 512 is history. Disks used to have 512 byte sectors, the field was defined in those units, and it stayed that way when blocks got bigger. A one byte file on a disk with 4 KB blocks reports 8 blocks: 8 times 512 is 4096.

It isn't always bigger than the length. A *sparse* file, like a virtual machine's disk image, can claim to be 64 GB long and use 3 GB, because the parts that are all zeros were never written. Some file systems compress files, too. Space on disk is the number that adds up to what your disk says is used, which is the point of Roomy.

Folders take space as well, for the list of names inside them. So now each folder adds its own size to its contents' [line 193], and so does the folder we started at [line 125].

### Code for one operating system

`blocks()` isn't on every `Metadata`. It comes from the `MetadataExt` trait in `std::os::unix::fs` [line 233], which only exists on Unix-like systems: Linux, macOS and the BSDs. On Windows that module isn't there, and the program wouldn't compile.

Rust's answer is *conditional compilation*.

```text
230  #[cfg(unix)]
231  mod platform {
```

```text
262  #[cfg(not(unix))]
263  mod platform {
```

`#[cfg(unix)]` [line 230] means "only compile this when building for Unix". `#[cfg(not(unix))]` [line 262] means the opposite. So there are two modules called `platform`, and any one build only ever sees one of them. They have the same four functions with the same signatures, and the rest of `scan.rs` calls `platform::disk_size` without knowing or caring which one it got.

This is different from an `if`. With an `if`, both branches are compiled and one is chosen when the program runs. With `cfg`, the other branch is never compiled at all, so it can call functions that don't exist on this system.

There's also a `cfg!` macro, which you can use in ordinary expressions.

```text
46              one_file_system: !cfg!(target_os = "macos"),
```

`cfg!(target_os = "macos")` is just `true` or `false`, worked out when the program is built. Both sides still have to compile everywhere, so it's for choosing values, not for calling platform functions.

### Honest about Windows

Here's the Windows version.

```text
267      /// Windows does have a way to ask for the space used on disk
268      /// (`GetCompressedFileSizeW`), but the standard library doesn't call it.
269      /// The file's length is close enough, except for compressed files.
270      pub fn disk_size(meta: &Metadata) -> u64 {
271          meta.len()
272      }
```

It's simpler than I'd like. Windows can tell you the space a file uses, but Rust's standard library doesn't ask, and calling it ourselves means talking to the Windows API directly. That's a fine project (if you'd like to see how, I wrote [a whole tutorial on the Windows API](https://github.com/pravin/win32)), but it would double the length of this chapter. Hard links exist on Windows too, but the standard library's way of reading a file's link count isn't stable yet. So on Windows, Roomy counts lengths and doesn't spot hard links. For most folders the difference is small. For a folder of tiny files it can be noticeable. I've left it as the exercise, for the brave.

### Hard links

On Unix, a file's name and the file itself are separate things. The file is a numbered record on the disk, called an *inode*. A folder is a list of names, each pointing at an inode. Usually one name points at each file, but there's nothing stopping two names, in two folders, from pointing at the same one. That's a *hard link*. Both names are equally real. Delete one and the file is still there under the other.

Package managers love them. `pnpm` keeps one copy of every package and hard-links it into each project. Time Machine and some backup tools do the same. Count each name as a separate file and Roomy would say you've used far more space than you have.

The metadata tells us how many names a file has (`nlink`) and which inode it is (`ino`), and which device it's on (`dev`), because inode numbers are only unique within one disk. So `(dev, ino)` names a file uniquely.

```text
246      pub fn link_key(meta: &Metadata) -> Option<(u64, u64)> {
247          (meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
248      }
```

`bool::then` [line 247] turns a `bool` into an `Option`: `Some(the closure's result)` if it's true, `None` if it's false. Most files have one name, and those we don't need to remember at all. Only files with more than one name get a key.

Then the first time we see a key, we count the file's size. Every other time, it counts as a file with no size.

```text
221  /// A file with hard links shows up in more than one folder, but its bytes
222  /// are only on the disk once. We count it the first time we see it.
223  fn first_sighting(ctx: &Context, meta: &Metadata) -> bool {
224      match platform::link_key(meta) {
225          Some(key) => ctx.seen_links.lock().unwrap().insert(key),
226          None => true,
227      }
228  }
```

`HashSet::insert` returns `true` if the value wasn't already there. That's exactly "first sighting", so we return it as it is [line 225].

The set is shared by every scanning thread, so it lives behind a `Mutex`. Last chapter I said a lock that every file queues for would be slow. This one isn't, because only files with hard links ever touch it, and that's a tiny fraction of them.

Which folder gets the bytes, and which gets zero? Whichever the scanner reaches first, and with many threads that can change from one scan to the next. The total is always right. The share between the two folders is a coin toss. `du` has the same quirk.

### Staying on one disk

On Linux and macOS, other disks don't get letters. They're *mounted* at a folder inside the main one: a USB stick at `/media/me/STICK`, a network share at `/mnt/nas`, a Mac's external drive at `/Volumes/Backup`. Scan `/` and you'd wander into all of them. If one of them is your 4 TB network backup, that's a long wait for an answer to the wrong question.

Every file's metadata says which device it's on. So we remember the device we started on, and don't go into a folder that's on a different one.

```text
188      if file_type.is_dir() {
189          if ctx.options.one_file_system && platform::device(&meta) != ctx.root_device {
190              return None;
191          }
```

`du -x` and `find -xdev` do the same thing. On Windows each drive has its own letter, so you can't get to drive D: from drive C: by accident, and `device` just returns 0 for everything.

### Things that aren't there

Linux has folders that aren't on any disk at all. `/proc` is the kernel describing running programs as files. `/sys` is the hardware. They take no space, and some of their "files" claim to be enormous. Those get left out by name.

macOS has a stranger problem. Since Catalina, the system lives on a read-only volume and your files live on a separate *Data* volume. The two are stitched together with what Apple calls *firmlinks*, so `/Users` appears where you'd expect it. But the Data volume is also visible, in full, at `/System/Volumes/Data`. Scan `/` and you'd count your whole home folder twice.

So each platform has a list of folders to skip.

```text
250      pub fn default_excludes() -> Vec<PathBuf> {
251          let paths: &[&str] = if cfg!(target_os = "macos") {
252              // /System/Volumes/Data is your data again, seen from another angle.
253              &["/System/Volumes", "/dev"]
254          } else {
255              // Files the kernel makes up on the spot. They take no space.
256              &["/proc", "/sys", "/dev", "/run"]
257          };
258          paths.iter().map(PathBuf::from).collect()
259      }
```

The firmlinks also explain the odd line we saw earlier. Your home folder really is on a different device from `/`, so on a Mac "stay on one disk" would skip `/Users`. On macOS we leave that option off and rely on the exclude list.

That has a cost, and I'd rather you heard it from me. On a Mac, if you scan `/` with an external drive plugged in, Roomy will go into `/Volumes` and count it too. You can add it to the exclude list. Chapter 17 adds a settings screen to make that easy.

Checking the exclude list happens before anything else, for every entry [line 174]. It's a `Vec`, and `contains` checks each item in turn, which is fine for a list of four.

### ScanOptions

The choices go in one struct.

```text
28  /// Choices that change what gets scanned.
29  #[derive(Clone, Debug, Default, Serialize, Deserialize)]
30  #[serde(default)]
31  pub struct ScanOptions {
32      /// Stay on the drive we started on. Don't wander into other disks,
33      /// network shares or anything else mounted inside it.
34      pub one_file_system: bool,
35      /// Folders to leave out completely.
36      pub exclude: Vec<PathBuf>,
37  }
```

`Default` gives us `ScanOptions::default()`: `false` and an empty list. The tests use that, so they scan exactly what's in their temporary folder and nothing is left out. `recommended()` [line 41] is what Roomy uses for real.

`Serialize` and `Deserialize` aren't used yet. They're for chapter 17, when the options get saved to a settings file. `#[serde(default)]` [line 30] says that if a field is missing when reading one back, use the default for it. That means a settings file written by an older Roomy, before we'd thought of some new option, still loads.

The test for excludes shows a nice bit of syntax.

```text
331          let options = ScanOptions {
332              exclude: vec![dir.path().join("skip")],
333              ..ScanOptions::default()
334          };
```

`..ScanOptions::default()` [line 333] means "and take every field I didn't mention from this one". It's handy for structs with lots of fields where you only want to change one.

### A struct that borrows

The scanner now needs four things wherever it goes: the options, the progress counters, the device we started on, and the set of hard links. Passing four arguments down through every call gets old, so they go in a struct.

```text
99  /// Everything the scanning threads share.
100  struct Context<'a> {
101      options: &'a ScanOptions,
102      progress: &'a Progress,
103      root_device: u64,
104      /// Hard links we have already counted, as (device, inode).
105      seen_links: Mutex<HashSet<(u64, u64)>>,
106  }
```

`Context` holds *references*, `&ScanOptions` and `&Progress`, not the things themselves. Whenever a struct holds a reference, Rust wants to know how long that reference is good for, so the struct gets a *lifetime parameter*, written `'a` [line 100]. Read `Context<'a>` as "a `Context` that's borrowing things which live for some stretch of time called `'a`". The compiler then makes sure no `Context` outlives what it borrowed.

You rarely have to think harder than that. We make a `Context` at the top of `scan` [line 118], pass `&ctx` down to every call, and it's dropped when `scan` returns, long before the options and progress it borrowed from the caller. The compiler checks all of that and says nothing, which is how you know it's happy.

`Context` is shared between all of Rayon's threads, so it must be `Sync`. References to `Sync` things are `Sync`, a `u64` is, and a `Mutex` makes whatever's inside it `Sync`. So the whole struct is, and we didn't have to say so.

## Adding functionality

Let's see how much hard links were inflating things. If you use `pnpm`, or keep backups made by `rsync --link-dest`, the difference can be dramatic.

1. In `core/src/scan.rs`, change line 247 to `None` instead of the `then` expression, so nothing ever looks hard-linked. (Write it as `let _ = meta; None`, so the compiler doesn't warn that `meta` is unused.)
2. Run the CLI on your projects folder and write down the total.
3. Put line 247 back and run it again.

If the two numbers are the same, you don't have many hard links. Try `/usr` on Linux, or your Time Machine folder if you have one.

## Exercise

Give the CLI a second number. After "Scanned ~/Code in 2.7s: 464,806 files, 19 GB", show the total *length* of the files too, as in "(17 GB of data)". The difference between the two is the space lost to block sizes.

*Hint: `Progress` would need a second byte counter, added to with `meta.len()` in `scan_item`, next to the one that adds `size`. For the brave, make Windows count space on disk properly too: the `windows` crate has `GetCompressedFileSizeW` in `Windows::Win32::Storage::FileSystem`.*

## That's it

The numbers are right. Next chapter we find out how fast they are. We'll give the command line tool proper options and a `--help`, build it for release, and race it against `du`.

[Chapter 9: Race day](../09-race-day/README.md)
