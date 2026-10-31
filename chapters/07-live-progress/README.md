# Chapter 7 - Live progress

[< Chapter 6: Going parallel](../06-going-parallel/README.md)

Roomy is fast, but on a whole disk "fast" still means fifteen or twenty seconds. That's a long time to stare at a spinner. Is it working? Is it stuck? Is it nearly done? People give up on apps that don't say.

This chapter gives the scanner a set of counters that any thread can read while the scan is still going. The command line tool shows a running total on one line of the terminal. The app shows a big, ticking number of bytes, the file count, the time so far and the folder it's in, ten times a second. And there's a Cancel button, for when you pick your whole disk by mistake.

In this chapter you will

- meet atomics: numbers that many threads can change at once without a lock,
- learn what `Ordering::Relaxed` means, and why it's all we need here,
- share one value between threads with `Arc`, and lock one with `Mutex`,
- stop a scan from another thread with an `AtomicBool`,
- start a thread that borrows local variables, with `std::thread::scope`,
- send events from Rust to the web page, and listen for them in TypeScript,
- count the things we couldn't read.

## Before we begin

The scanner gets most of the changes.

- `core/src/scan.rs` gains a `Progress` struct, and `scan` takes a `&Progress`.
- `core/src/lib.rs` exports `Progress`.
- `cli/src/main.rs` runs the scan on a second thread and prints the counters while it waits.
- `app/src-tauri/src/main.rs` keeps the current scan's `Progress` in the app state.
- `app/src-tauri/src/commands.rs` sends a `scan-progress` event ten times a second and has a new `cancel_scan` command.
- `app/src/api.ts`, `app/src/main.ts` and `app/index.html` show the live numbers and the Cancel button.

Run the command line tool on something big.

```text
cd chapters/07-live-progress
cargo run --release -p roomy -- ~
```

While it works you'll see one line that keeps changing.

```text
Scanning... 1,204,331 files, 96 GB
```

When it's done, the line clears and the usual summary appears, with a note if anything couldn't be read. Now the app.

```text
cd app
npm install
npm run tauri dev
```

Pick a big folder. The scanning screen counts up as it goes, and Cancel takes you back to the start.

![Roomy counting files as it scans](images/screenshot.png)

## Breaking it up

### Counters for many threads

Here's what the scanner keeps track of.

```text
26  /// Counters the scanner updates as it goes. Any thread can read them while
27  /// a scan is running, which is how the app shows a live total.
28  #[derive(Debug, Default)]
29  pub struct Progress {
30      files: AtomicU64,
31      bytes: AtomicU64,
32      errors: AtomicU64,
33      cancelled: AtomicBool,
34      current: Mutex<PathBuf>,
35  }
```

Last chapter I made a fuss about the scanner having no shared state, and here we are adding some. That's fine, as long as it's the right kind.

A plain `u64` can't be shared like this. If two threads both do `files += 1` at the same moment, each reads the old value, each adds one, and each writes back. You've counted two files and the number went up by one. Rust won't even let you try: you'd need a `&mut` to change it, and you can't have two `&mut`s at once.

An `AtomicU64` [line 30] is a `u64` that the processor itself updates in one indivisible step. "Atomic" is the old Greek sense of the word, meaning it can't be cut in half. Two threads adding at the same moment always end up with both additions counted. And you can do it through a plain shared `&`, which is the trick that makes it work across threads.

```text
155      progress.files.fetch_add(1, Ordering::Relaxed);
156      progress.bytes.fetch_add(size, Ordering::Relaxed);
```

`fetch_add` adds to the number and hands back what it was before. We don't care what it was before, so we ignore it. Reading one is `load` [line 43].

### Ordering::Relaxed

Every atomic operation asks for an `Ordering`. This is the part of atomics that makes people's heads hurt, so here's the short version.

Processors and compilers reorder memory operations to go faster. Most of the time you can't tell. When one thread *publishes* something for another, say "I've filled in this buffer, and now I set a flag to say it's ready", the order matters a lot. The other thread mustn't see the flag before it sees the buffer. The stronger orderings, `Acquire`, `Release` and `SeqCst`, are how you ask for that.

We don't publish anything. Each counter is just a number, and nothing else depends on reading it in the right order relative to anything else. If the window shows 1,204,331 files when the true figure is 1,204,338 by the time the pixels hit the screen, nobody is hurt. `Relaxed` says "make this one operation atomic, and don't bother with any ordering guarantees beyond that". It's the cheapest kind there is, and it's exactly what counters need.

### The current folder

The scanning screen shows which folder Roomy is in. A path isn't a number, so it can't be atomic. It goes in a `Mutex` instead [line 34].

A `Mutex<PathBuf>` is a `PathBuf` with a lock around it. To touch the path you call `lock()`, which waits until no other thread has the lock and then gives you a *guard*. The guard acts like a `&mut PathBuf`. When the guard goes out of scope, the lock is let go. You can't forget to unlock, because there's no unlock to forget.

But we have a pool of threads, all in different folders, all wanting to write the path. If each waited its turn, we'd be back to a queue. So the scanner doesn't wait.

```text
103      if let Ok(mut current) = progress.current.try_lock() {
104          path.clone_into(&mut current);
105      }
```

`try_lock` [line 103] takes the lock if it's free, and gives up at once if it isn't. If some other thread is writing its folder right now, fine, that one wins and we carry on. The screen shows *a* folder Roomy is in, which is all anyone wants to know. `clone_into` copies the path into the `PathBuf` that's already there, reusing its memory instead of making a new one for every folder.

Reading it from another thread does wait, but only for the moment it takes to copy a path.

```text
55      pub fn current(&self) -> PathBuf {
56          self.current.lock().map(|p| p.clone()).unwrap_or_default()
57      }
```

`lock()` returns a `Result`, because a mutex can be *poisoned*: if a thread panics while holding the lock, the data might be half changed, and Rust tells the next person. For a path on a progress screen we don't care, so `unwrap_or_default()` gives an empty path in that case.

### Counting errors

In chapter 3 we started counting things we couldn't read. Chapter 5 dropped that, because the tree only marked unreadable folders. Now it comes back as a counter.

```text
117      let items: Vec<DirEntry> = items
118          .filter_map(|item| item.inspect_err(|_| progress.error()).ok())
119          .collect();
```

`inspect_err` [line 118] lets us peek at an error as it goes by, without changing the `Result`. We count it, then `ok()` turns the `Result` into an `Option`, and `filter_map` drops the `None`s. The `error` method is private to the module (it has no `pub`), so only the scanner can add errors. Everyone else can only read them.

### Cancelling

Stopping a thread from the outside is a bad idea in any language. It could be halfway through anything. The polite way is to ask it to stop, and have it check every so often. That's what `cancelled` is for.

```text
59      /// Asks a running scan to stop as soon as it can.
60      pub fn cancel(&self) {
61          self.cancelled.store(true, Ordering::Relaxed);
62      }
```

The scanner checks it each time it enters a folder.

```text
100      if progress.is_cancelled() {
101          return entry;
102      }
```

Once the flag is set, every thread hands back an empty folder at its next step. The whole tree folds up in a few milliseconds. `scan` checks once more at the end, and says why there's no tree.

```text
85      if progress.is_cancelled() {
86          return Err(ScanError::Cancelled);
87      }
```

`ScanError` grows a `Cancelled` variant for this [line 23]. There's a test too, which sets the flag before the scan even starts and checks we get `Cancelled` back.

### A thread that borrows

The command line tool has a problem. `roomy_core::scan` doesn't return until it's finished, and we want to print while it runs. So the scan goes on a second thread, and the main thread prints.

```text
27      let progress = Progress::new();
28      let started = Instant::now();
29      let show = io::stderr().is_terminal();
30
31      // A scoped thread can borrow `progress`, because Rust knows the thread
32      // ends before this function does.
33      let tree = thread::scope(|s| {
34          let scanner = s.spawn(|| roomy_core::scan(path, &progress));
35          while !scanner.is_finished() {
36              if show {
37                  eprint!(
38                      "\rScanning... {} files, {}   ",
39                      thousands(progress.files()),
40                      format_size(progress.bytes())
41                  );
42              }
43              thread::sleep(Duration::from_millis(100));
44          }
45          scanner.join().unwrap()
46      });
```

The scanning thread needs `progress` and `path`, and both belong to this function. Normally that's not allowed. A thread started with `std::thread::spawn` might run for ever, long after this function has returned and its local variables are gone, so it can only take things it owns.

`thread::scope` [line 33] fixes that. Every thread started inside the scope, with `s.spawn`, is guaranteed to have finished by the time `scope` returns. Rust knows this, so it lets those threads borrow anything that lives longer than the scope. The scanner borrows `progress` [line 34], the main thread reads the same `progress` in the loop [line 39], and nothing has to be copied or wrapped.

The loop checks `is_finished()` [line 35], prints, and sleeps for a tenth of a second. `join()` [line 45] gets the scanner's return value. It returns a `Result` too, which is an `Err` if the thread panicked, and we `unwrap` it because a panic in the scanner is a bug we'd want to see.

The `\r` at the start of the line [line 38] is a carriage return. It moves the cursor back to the start of the line without starting a new one, so each print overwrites the last. When we're done, we print 60 spaces to wipe the line [line 48]. And we only do any of this if standard error is a terminal [line 29]. If someone pipes Roomy into a file, they don't want a thousand "Scanning..." lines in it.

### Arc: sharing without borrowing

The app has a harder version of the same problem. The scan runs on one thread, and the Cancel button arrives later, as a different command, on a different thread. There's no function they both live inside, so nothing can be borrowed. They need to *share ownership* of the `Progress`.

That's what `Arc` is for. An `Arc<Progress>` is a pointer to a `Progress` that keeps count of how many `Arc`s point at it. Cloning an `Arc` doesn't copy the `Progress`. It makes another pointer and adds one to the count. When the last `Arc` goes away, the `Progress` is freed. The A stands for *atomic*: the count is an atomic number, so `Arc`s can be cloned and dropped on any thread. (Its single-threaded cousin is `Rc`, which last chapter promised would refuse to cross threads.)

The app state gets a place to keep one.

```text
16      /// The scan in progress, so it can be cancelled.
17      progress: Mutex<Option<Arc<Progress>>>,
```

Read it from the inside out. An `Arc<Progress>` we can share. `Option`, because most of the time no scan is running. `Mutex`, because two commands might touch it at once.

```text
93      let progress = Arc::new(Progress::new());
94      *app.state::<AppState>().progress.lock().unwrap() = Some(progress.clone());
```

The `scan` command makes the `Progress`, puts one `Arc` in the app state [line 94], and keeps the other. Now `cancel_scan` can find it.

```text
132  #[tauri::command]
133  pub fn cancel_scan(state: State<AppState>) {
134      if let Some(progress) = state.progress.lock().unwrap().as_ref() {
135          progress.cancel();
136      }
137  }
```

When the scan finishes, `scan` takes it out again [line 121], so a late Cancel can't cancel anything.

### Events

Inside the background thread, the app does what the CLI does, with one difference: instead of printing, it *emits an event*.

```text
101      let (result, seconds, errors) = tauri::async_runtime::spawn_blocking(move || {
102          thread::scope(|s| {
103              let scanner = s.spawn(|| {
104                  let tree = roomy_core::scan(&root, &progress);
105                  (tree, started.elapsed().as_secs_f64())
106              });
107              while !scanner.is_finished() {
108                  handle
109                      .emit("scan-progress", ProgressView::new(&progress, started))
110                      .ok();
111                  thread::sleep(Duration::from_millis(100));
112              }
113              let (tree, seconds) = scanner.join().unwrap();
114              (tree, seconds, progress.errors())
115          })
116      })
```

So far the window has always started the conversation: it calls a command, Rust answers. An event goes the other way. Rust sends it whenever it likes, and anything in the window that's listening hears it. `emit` [line 109] comes from Tauri's `Emitter` trait, which is why we import it. It takes a name and anything that's `Serialize`.

We could send `Progress` itself, but it's full of atomics and a mutex, which don't turn into JSON. So we take a snapshot of plain numbers.

```text
58  #[derive(Clone, Serialize)]
59  struct ProgressView {
60      files: u64,
61      bytes: u64,
62      errors: u64,
63      current: String,
64      seconds: f64,
65  }
```

`handle` is a clone of the `AppHandle` [line 98], moved into the closure, because the closure needs its own. Most Tauri handles are cheap to clone for exactly this reason. And `emit` returns a `Result`. If the window has gone, there's nobody to tell, so `.ok()` [line 110] throws the error away on purpose.

The time is measured inside the scanner thread [line 105] rather than after the loop. The loop sleeps in tenths of a second, and we don't want up to 100 ms of napping in our benchmark.

### Listening

On the TypeScript side, `api.ts` wraps Tauri's `listen`.

```text
43  export const onProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
44    listen<Progress>("scan-progress", (e) => f(e.payload));
```

`listen` calls our function every time a `scan-progress` event arrives, with the JSON already turned back into an object. It returns a function that stops listening. `main.ts` starts listening before the scan and stops afterwards, whatever happened.

```text
27    // Rust tells us how it's getting on ten times a second.
28    const stop = await api.onProgress((p) => {
29      $("#scan-bytes").textContent = formatSize(p.bytes);
30      $("#scan-files").textContent = `${formatCount(p.files)} files`;
31      $("#scan-time").textContent = formatSeconds(p.seconds);
32      $("#scan-current").textContent = p.current;
33    });
34    try {
35      const summary = await api.scan(path);
36      await showResults(summary);
37    } catch (error) {
38      // A cancelled scan just goes back to the start.
39      if (!String(error).includes("cancelled")) {
40        message(String(error), { title: "Roomy", kind: "error" });
41      }
42      showScreen("start");
43    } finally {
44      stop();
45    }
```

A cancelled scan comes back as an error, "the scan was cancelled", from `ScanError`'s `#[error]` text. You asked for it, so there's no error box [line 39]. We just go back to the start.

The current folder can be long. The stylesheet shows it right-to-left with an ellipsis, so you see the end of the path, the interesting part, rather than `/Users/me/Lib...` every time.

## Adding functionality

The CLI's progress line doesn't show the time. Let's add it.

1. In `cli/src/main.rs`, the loop already has `started` in scope.
2. Change the format string on line 38 to `"\rScanning... {} files, {} in {:.0}s   "`.
3. Add `started.elapsed().as_secs_f64()` as a third argument.

That's the whole change. The scoped thread didn't need to know.

## Exercise

Make the CLI stop cleanly when you press Ctrl+C. At the moment the operating system just kills it. Instead, the first Ctrl+C should cancel the scan and print "Cancelled", the way the app's button does.

*Hint: The `ctrlc` crate gives you `ctrlc::set_handler`, which takes a closure that must be `'static`. A borrowed `&Progress` won't do, so you'll need the `Progress` in an `Arc`, like the app has.*

## That's it

Roomy talks to you while it works. Before we show off any more numbers, though, we should make sure they're right. Next chapter we find out that the size of a file isn't one number, that some files live in two folders at once, and that macOS shows you some of your disk twice.

[Chapter 8: Sizes that don't lie](../08-sizes-that-dont-lie/README.md)
