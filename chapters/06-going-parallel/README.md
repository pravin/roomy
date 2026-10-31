# Chapter 6 - Going parallel

[< Chapter 5: A tree Rust is happy with](../05-a-tree-rust-is-happy-with/README.md)

Your computer has several cores, probably eight or more. Last chapter Roomy used one of them. The rest sat around waiting while one core walked your whole disk, one folder at a time. This chapter puts them all to work. It takes one new crate and changes about three lines of the scanner, and that's the happy part of the story: Rust's rules about ownership, which made us fuss over the tree last chapter, are exactly what make this so easy.

We'll also start timing the scan, so the CLI and the app both say how long it took. We'll need those numbers in chapter 9.

In this chapter you will

- add the `rayon` crate and meet `par_iter`,
- see why an owned tree splits across threads with no locks at all,
- learn, in plain words, what `Send` and `Sync` mean,
- time things with `std::time::Instant`,
- show "scanned in 2.7 s" in the CLI and in the app.

## Before we begin

The changes are small and spread out.

- `Cargo.toml` (the workspace one) and `core/Cargo.toml` gain `rayon`.
- `core/src/scan.rs` scans each folder's contents in parallel.
- `cli/src/main.rs` times the scan.
- `app/src-tauri/src/commands.rs` times the scan and sends `seconds` back to the window.
- `app/src/api.ts` and `app/src/main.ts` show the time.

Run the command line tool on a big folder, in release mode. Debug builds of Rust are slow, and timing them is a bit like timing a sprinter in wellies.

```text
cd chapters/06-going-parallel
cargo run --release -p roomy -- ~/Code
```

```text
Scanned /Users/me/Code in 2.7s: 464,806 files, 19 GB

   13 GB  █████████████░░░░░░░  partech/
  5.0 GB  █████░░░░░░░░░░░░░░░  personal/
  ...
```

Try the same folder with chapter 5's tool and you'll see the difference. Then run the app.

```text
cd app
npm install
npm run tauri dev
```

The results now say how long the scan took, under the total.

![Roomy showing how long the scan took](images/screenshot.png)

## Breaking it up

### Adding a crate

`rayon` is a library for doing data-parallel work: "do this to every item in a list, and use every core while you're at it". We add it to the workspace once, with a version.

```text
14  rayon = "1.12"
```

And then tell `core` it wants it.

```text
11  rayon.workspace = true
```

`rayon.workspace = true` means "use whatever version the workspace says". With three crates in the workspace, it saves us from having three different versions of the same thing.

### Three lines

Here's the heart of last chapter's `scan_dir`.

```text
49      // Anything we can't read is skipped.
50      entry.children = items
51          .filter_map(Result::ok)
52          .filter_map(|item| scan_item(&item))
53          .collect();
```

And here it is now.

```text
51      // Reading the names in one folder is quick. Looking inside each of the
52      // folders it holds is the slow part, so that's what we share out.
53      let items: Vec<DirEntry> = items.filter_map(Result::ok).collect();
54
55      entry.children = items.par_iter().filter_map(scan_item).collect();
```

That's it. That's the parallel scanner.

There are two steps. `fs::read_dir` gives us an *iterator*, something that hands out the folder's entries one at a time as the operating system reads them. Rayon can't share out an iterator like that very well, because nobody knows how long it is until it runs out. So first we collect the entries into a `Vec<DirEntry>` [line 53]. Reading names is cheap, so we lose nothing.

Then `par_iter()` [line 55] is the parallel version of `iter()`. It comes from `use rayon::prelude::*;` [line 5], which adds `par_iter` to every `Vec` and slice. A *prelude* is just a module of things a crate expects you to want all the time, so you import them in one go.

`filter_map(scan_item)` calls `scan_item` on every entry, keeps the `Some`s and drops the `None`s. Because we wrote `scan_item` to take a `&DirEntry`, we can pass the function itself rather than wrapping it in a closure. And `collect()` gathers the results into a `Vec<Entry>`, in the same order as the input.

### Why this works so well

Look at what happens when `scan_item` meets a folder.

```text
67      if file_type.is_dir() {
68          return Some(scan_dir(&item.path(), item.file_name()));
69      }
```

It calls `scan_dir`, which calls `par_iter` again, on that folder's contents. So the parallelism isn't only at the top. Every folder, at every depth, shares its subfolders out. Your home folder hands `Library`, `Code` and `Music` to different cores. `Code` hands its projects out in turn, and so on all the way down.

Rayon manages this with *work stealing*. It starts one thread per core. Each thread has a queue of jobs. When a thread runs out of work, it steals a job from the back of someone else's queue. A thread that hits a gigantic `node_modules` stays busy with it, and the others quietly take the rest of its siblings. Nobody plans who does what. It balances itself.

What's missing is just as important. There's no lock. There's no shared counter. There's no "please wait while I add this to the tree". Each call to `scan_dir` builds its own `Entry`, owns it, and hands it back to its caller when it's done. The caller is the only one who ever sees it. That's why we built an *owned* tree in chapter 5 and turned it into the arena afterwards. An owned tree is made of separate pieces that never touch each other until they're finished, which is exactly the shape that splits across threads.

Try to imagine doing this with the arena directly. Every thread would want to `push` into the same `Vec<Node>`, and every child would need to tell its parent its index. You'd need a lock around the `Vec`, and every one of millions of files would queue up for it. It would probably be slower than one thread.

### Send and Sync

If you got this wrong, if you tried to share something between threads that isn't safe to share, Rust would refuse to compile it. It does that with two *marker traits*. A trait, as we saw in chapter 4, is a promise that a type can do something. These two promise something about threads.

- A type is **`Send`** if it's safe to *move* to another thread. Nearly everything is. `Entry` is: it's made of an `OsString`, some numbers and a `Vec<Entry>`, and each of those is `Send`.
- A type is **`Sync`** if it's safe for several threads to *look at* it at the same time, through a shared `&` reference. A `DirEntry` is `Sync`, which is why many threads can hold `&DirEntry`s from the same `Vec` at once.

You never write `impl Send for Entry`. The compiler works it out: a struct is `Send` if everything in it is `Send`. The things that aren't are the things that really aren't safe, like `Rc`, a reference-counted pointer whose counter isn't safe to change from two threads. Put an `Rc` in `Entry` and `par_iter` would stop compiling, with an error that says `Rc<...>` cannot be sent between threads safely.

That's what people mean by "fearless concurrency". It isn't that threads stop being hard. It's that the hard mistakes, two threads stepping on the same data, are caught before the program ever runs.

### Timing

Measuring time takes two lines. Here's the CLI.

```text
23  /// Scans a folder and says how long it took.
24  fn scan(path: &Path) -> Result<Tree, String> {
25      let started = Instant::now();
26      let tree = roomy_core::scan(path).map_err(|e| e.to_string())?;
27      let root = &tree[tree.root()];
28      eprintln!(
29          "Scanned {} in {:.1}s: {} files, {}",
30          path.display(),
31          started.elapsed().as_secs_f64(),
32          thousands(root.files),
33          format_size(root.size)
34      );
35      Ok(tree)
36  }
```

`Instant::now()` [line 25] reads a clock that only ever goes forwards. (The wall clock can jump about when your computer syncs its time, which is no good for measuring.) `started.elapsed()` [line 31] gives a `Duration`, and `as_secs_f64()` turns that into seconds as an `f64`, a 64 bit floating point number. `{:.1}` prints it with one decimal place.

Notice that the scan time goes to `eprintln!`, standard error, and the list goes to `println!`, standard output. That way `roomy ~/Code > biggest.txt` saves the list and still shows you the time.

### Timing in the app

The app does the same thing inside the background thread.

```text
56  /// What a finished scan sends back to the window.
57  #[derive(Serialize)]
58  pub struct ScanSummary {
59      root: NodeView,
60      /// How long the scan took.
61      seconds: f64,
62  }
```

```text
69      let (result, seconds) = tauri::async_runtime::spawn_blocking(move || {
70          let started = Instant::now();
71          let tree = roomy_core::scan(&path);
72          (tree, started.elapsed().as_secs_f64())
73      })
74      .await
75      .map_err(|e| e.to_string())?;
```

The closure now returns a tuple: the scan's result, and how long it took [line 72]. `let (result, seconds) = ...` [line 69] takes the tuple apart again on the other side. We start the clock inside the closure on purpose, so we measure the scan and not the time it took to get a thread going.

`seconds` goes into `ScanSummary` [line 61]. `serde` turns it into a JSON number, and on the TypeScript side `api.ts` grows one line to match.

```text
26    seconds: number;
```

`main.ts` formats it. A small folder can take a few thousandths of a second, so under one second we count milliseconds, "4 ms". Under ten seconds we show one decimal, "2.7 s". Above that, the decimal is just noise. `Math.ceil` rounds up, so even the quickest scan says "1 ms" rather than "0 ms".

```text
16  function formatSeconds(seconds: number): string {
17    return seconds < 1 ? `${Math.ceil(seconds * 1000)} ms` : `${seconds.toFixed(seconds < 10 ? 1 : 0)} s`;
18  }
```

## Adding functionality

Let's see how many threads Rayon really uses.

1. Run the CLI with one thread. Rayon reads an environment variable for this:

   ```text
   RAYON_NUM_THREADS=1 cargo run --release -p roomy -- ~/Code
   ```

2. Run it again with 2, 4 and with the variable left out.
3. Write the times down. You'll see big gains up to the number of performance cores you have, then smaller ones. The disk and the operating system have limits too, and past a point we're waiting on them, not on the processor.

Run each one twice and use the second time. The first run reads from the disk. The second finds most of it already in memory, and that's a difference we'll come back to in chapter 9.

## Exercise

Make the CLI print how many cores Rayon is using, next to the time, as in "Scanned ~/Code in 2.7s on 10 threads".

*Hint: `rayon::current_num_threads()`. The CLI doesn't depend on `rayon` yet, so add it to `cli/Cargo.toml` with `rayon.workspace = true`.*

## That's it

Roomy is fast now. It's also silent. On a big disk you'll sit looking at a spinner for ten or twenty seconds with no idea whether it's nearly done or just getting started. Next chapter we fix that with live progress, and a Cancel button for when you change your mind.

[Chapter 7: Live progress](../07-live-progress/README.md)
