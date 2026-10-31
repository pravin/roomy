# Chapter 5 - A tree Rust is happy with

[< Chapter 4: A window of your own](../04-a-window-of-your-own/README.md)

Roomy tells you your home folder holds 214 GB. Lovely. *Where?*

To answer that, adding up isn't enough. We have to remember what we found: every file, every folder, and what's inside what. That's a tree. The folder you scanned is the root, the things in it are its children, their contents are their children, and so on down.

Trees are where many people's first week with Rust goes wrong. In most languages you'd give every node a list of its children and a pointer back to its parent, and get on with your day. In Rust that innocent parent pointer starts a fight with the compiler that you will lose. So this chapter is about building a tree the way Rust likes: first as one big nested value, then flattened into a list where nodes refer to each other by number.

In this chapter you will

- build a tree in which every node owns its children,
- find out why a pointer back to the parent is a problem in Rust,
- flatten the tree into an *arena*: one `Vec`, with nodes pointing at each other by index,
- wrap an index in a *newtype* so it can't be mixed up with other numbers,
- implement the `Index` trait so `tree[id]` works,
- keep the tree in the app between commands, behind a `RwLock`,
- show the biggest things in a folder, in the window and in the terminal.

## Before we begin

One new file, `core/src/tree.rs`. `core/src/scan.rs` builds a tree instead of adding up totals. The command line tool lists the twenty biggest things in the folder, with bars. The app keeps the tree after scanning and shows the fifteen biggest things on the results screen.

Run the command line tool on a folder with a few things in it.

```text
cd chapters/05-a-tree-rust-is-happy-with
cargo run --release -p roomy -- ~/Code
```

```text
Scanned /Users/me/Code: 465,102 files, 19 GB

   13 GB  █████████████░░░░░░░  partners/
  5.0 GB  █████░░░░░░░░░░░░░░░  personal/
  1.2 GB  █░░░░░░░░░░░░░░░░░░░  experiments/
   74 MB  ░░░░░░░░░░░░░░░░░░░░  dotfiles/
   12 KB  ░░░░░░░░░░░░░░░░░░░░  .DS_Store
```

And the app:

```text
cd app
npm install
npm run tauri dev
```

Pick a folder, and under the total you get the biggest things inside it, biggest first.

One thing has gone missing since last chapter: the count of things we couldn't read. The tree remembers which folders were locked (each node has an `unreadable` flag), but the running count is gone. It comes back in chapter 7, when the scan has somewhere to keep counters as it goes.

![Roomy listing the biggest things in a folder](images/screenshot.png)

## Breaking it up

### A tree that owns its children

The obvious tree in Rust is a struct with a list of more of itself.

```text
20  #[derive(Debug)]
21  pub struct Entry {
22      pub name: OsString,
23      pub kind: Kind,
24      /// Bytes on disk. For a folder, everything inside it too.
25      pub size: u64,
26      /// How many files are in here, counting every level down.
27      pub files: u64,
28      /// We weren't allowed to look inside this folder.
29      pub unreadable: bool,
30      pub children: Vec<Entry>,
31  }
```

An `Entry` owns its `children` [line 30], and each child owns its own children, and so on. The root owns the whole tree. When the root goes away, everything goes with it. Rust is perfectly happy with this. Ownership flows one way, from the top down, and nobody shares anything.

`OsString` [line 22] is the owned version of an operating system string, the raw name the file system gave us. We keep the raw name rather than turning it into a `String`, because a name that isn't valid text would be mangled, and later we'll need the exact name to find the file again (to delete it, say).

(The comment on `size` says *bytes on disk*. For now it's actually the file's length, which isn't quite the same thing. Chapter 8 is all about the difference.)

`Kind` [line 23] is a small enum, and `#[serde(rename_all = "lowercase")]` above it means it reaches the page as `"file"`, `"dir"`, `"symlink"` or `"other"`.

The scanner builds this tree directly. It's chapter 3's `walk`, except that instead of adding to running totals, each call returns an `Entry` for its folder.

```text
33  fn scan_dir(path: &Path, name: OsString) -> Entry {
...
49      // Anything we can't read is skipped.
50      entry.children = items
51          .filter_map(Result::ok)
52          .filter_map(|item| scan_item(&item))
53          .collect();
54      entry.size = entry.children.iter().map(|c| c.size).sum();
55      entry.files = entry.children.iter().map(|c| c.files).sum();
56      entry
57  }
```

This is our first real use of *iterators*. `items` is the folder's contents, one at a time. `filter_map(Result::ok)` [line 51] turns each `Result` into an `Option` and keeps only the good ones. `filter_map(|item| scan_item(&item))` [line 52] turns each item into an `Entry`, dropping any that come back `None`. `collect()` [line 53] gathers what's left into a `Vec<Entry>`. Rust knows to make a `Vec` because that's the type of `entry.children`.

Lines 54 and 55 add up the children. `iter()` lends us each child in turn, `map` picks out its size, and `sum` adds them. Iterator chains like these compile to the same machine code as a hand-written loop. You're not paying anything for the tidiness.

`scan_item` uses `?` in a new way.

```text
59  fn scan_item(item: &DirEntry) -> Option<Entry> {
60      // This doesn't follow symbolic links, which is what we want. Following
61      // them would count things twice, or go round in circles forever.
62      let meta = item.metadata().ok()?;
```

`?` works on `Option` too. In a function that returns an `Option`, `?` on a `None` returns `None` straight away. `.ok()` turns the `Result` into an `Option`, throwing the error away, and `?` says "if there wasn't one, skip this item".

### Why not just add a parent?

The owned tree is easy to build, but there's something it can't do. Given a file deep inside, how do you find its full path? Or the folder it's in? You'd walk *up* the tree, from child to parent. And an `Entry` has no idea who its parent is.

In another language you'd add a field, `parent: *Entry` or similar. Try it in Rust and you'll find there's no type that works. A plain reference, `&Entry`, would be a child borrowing its parent while the parent owns the child. The parent can never be changed again (it's lent out), and it can never be moved, because the references would point at the old place. Rust doesn't allow a struct to hold a reference into itself, and that's essentially what this is.

There are ways around it. You can wrap every node in `Rc<RefCell<...>>`, reference counted shared ownership with borrow rules checked while the program runs. It works, but it's slow, wordy, and turns compile-time errors into run-time panics. Most Rust programmers who've been there will tell you to do something else.

### The arena

The something else is called an *arena*. Put every node in one `Vec`, and have nodes refer to each other by their position in it.

```text
45  /// One file or folder in a `Tree`.
46  #[derive(Debug)]
47  pub struct Node {
48      pub name: OsString,
49      pub kind: Kind,
50      pub size: u64,
51      pub files: u64,
52      pub unreadable: bool,
53      pub parent: Option<NodeId>,
54      /// Biggest first.
55      pub children: Vec<NodeId>,
56  }
...
67  #[derive(Debug)]
68  pub struct Tree {
69      root_path: PathBuf,
70      nodes: Vec<Node>,
71  }
```

The `Tree` owns a `Vec` of every `Node`. Nobody else owns anything. A node's `parent` [line 53] and `children` [line 55] are just numbers, positions in that `Vec`. Numbers can point anywhere, up or down, and the borrow checker has nothing to object to, because nothing is borrowing anything. The parent is `Option<NodeId>` because the root doesn't have one.

It's also faster. All the nodes sit next to each other in memory, which processors love, and there's one big allocation instead of one per node.

The cost is that the compiler can no longer check that a number really points at a node. If we kept a `NodeId` from an old scan and used it on a new one, it would happily point at the wrong file. We'll be careful about that, and the app checks every id it's given.

### A number that's not just a number

```text
33  /// The number of a node in the tree. It's an index into a `Vec`, wrapped up
34  /// so it can't be mixed up with any other number.
35  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
36  #[serde(transparent)]
37  pub struct NodeId(pub u32);
38
39  impl NodeId {
40      fn index(self) -> usize {
41          self.0 as usize
42      }
43  }
```

We could use a plain `usize` as the id. But then nothing stops you passing a file count where an id was wanted, or adding two ids together, which means nothing. Wrapping the number in a struct of its own, `NodeId(u32)`, makes it a different type. This is called the *newtype* pattern. It costs nothing at run time (a `NodeId` is exactly a `u32` in memory), but the compiler now knows that ids are ids.

Why `u32` and not `usize`? `usize` is 64 bits on a 64-bit computer. `u32` goes up to four billion, more files than any disk has, and halves the memory taken by every id. With millions of nodes, that adds up.

`self.0` [line 41] is the first (and only) field of the tuple struct. `index` isn't `pub`, so only the tree can turn an id back into a position.

`#[serde(transparent)]` [line 36] sends a `NodeId` to the page as a plain number, `42`, rather than as something like `[42]`.

### Flattening

The scanner still builds the easy owned tree, and `from_entry` flattens it into the arena.

```text
76      pub fn from_entry(root_path: PathBuf, entry: Entry) -> Tree {
77          let mut tree = Tree {
78              root_path,
79              nodes: Vec::new(),
80          };
81          tree.add(entry, None);
82          tree
83      }
84
85      fn add(&mut self, mut entry: Entry, parent: Option<NodeId>) -> NodeId {
86          let id = NodeId(self.nodes.len() as u32);
87          entry.children.sort_by_key(|c| std::cmp::Reverse(c.size));
88          let children = std::mem::take(&mut entry.children);
89          self.nodes.push(Node {
90              name: entry.name,
91              kind: entry.kind,
92              size: entry.size,
93              files: entry.files,
94              unreadable: entry.unreadable,
95              parent,
96              children: Vec::with_capacity(children.len()),
97          });
98          for child in children {
99              let child_id = self.add(child, Some(id));
100             self.nodes[id.index()].children.push(child_id);
101         }
102         id
103     }
```

`add` takes an `Entry` by value, which means it takes ownership. The caller hands the entry over and never sees it again. That's what we want: the owned tree is used up as the arena is filled, so we never have two copies of millions of names in memory.

The new node's id is just where it will land in the `Vec` [line 86]. Then we sort its children biggest first [line 87]. Everything that shows a folder wants the biggest things first, so we sort once here and never again. `sort_by_key` sorts by a key we give it; `Reverse` flips the order so it's largest first.

Line 88 is a little trick. We want to move the children out of `entry` and into the loop, but also move `entry.name` into the new node. Rust would complain if we tried to use `entry` after moving part of it in certain ways, so `std::mem::take` swaps an empty `Vec` into `entry.children` and hands us the old one. Now the children are ours, separately.

We push the node [line 89] with an empty list of children, then add each child, recursively, and push the child's id onto our list [line 100]. Notice we can't hold on to a reference to our new node while doing that, because `self.add` needs to change `self.nodes`. So we look it up again by index each time. That's the arena way: hold ids, not references.

Because each node is pushed before its children, the root is always node 0.

### Asking the tree things

```text
117      /// The full path of a node on disk.
118      pub fn path(&self, id: NodeId) -> PathBuf {
119          let mut path = self.root_path.clone();
120          for ancestor in self.ancestors(id).into_iter().skip(1) {
121              path.push(&self[ancestor].name);
122          }
123          path
124      }
125
126      /// The nodes from the root down to `id`, both included.
127      pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
128          let mut chain = vec![id];
129          let mut current = id;
130          while let Some(parent) = self[current].parent {
131              chain.push(parent);
132              current = parent;
133          }
134          chain.reverse();
135          chain
136      }
```

Here's the parent pointer earning its keep. `ancestors` climbs from a node to the root, collecting ids, then reverses the list so it reads root first. `while let` [line 130] keeps looping as long as the pattern matches, here as long as there's a parent.

`path` rebuilds a node's full path from the root path and the names on the way down. We don't store a full path in every node, which would take a lot of memory for millions of files. We work it out when someone asks, which is rarely.

### tree[id]

```text
139  impl Index<NodeId> for Tree {
140      type Output = Node;
141
142      fn index(&self, id: NodeId) -> &Node {
143          &self.nodes[id.index()]
144      }
145  }
```

Implementing the standard `Index` trait means we can write `tree[id]` to get a node, just like indexing a `Vec`. `type Output = Node` says what indexing gives you. Like a `Vec`, it panics if the id is out of range. For when we're not sure an id is good (it came from the page, say) there's `get` [line 113], which returns an `Option` instead.

### Keeping the tree in the app

In chapter 4 the scan command returned a total and forgot everything. Now the app needs to keep the tree, so the page can ask about it afterwards. Tauri gives every app a place for that, its *state*.

```text
10  /// What the app remembers between one command and the next.
11  #[derive(Default)]
12  pub struct AppState {
13      /// The last finished scan. Lots of commands can read it at once, but
14      /// only one at a time can change it.
15      tree: RwLock<Option<Tree>>,
16  }
...
21          .manage(AppState::default())
```

`.manage` hands Tauri a value to look after, and any command can ask for it. It's `Option<Tree>` because before the first scan there isn't a tree.

Commands can run on several threads at once, so the tree is shared between threads, and in Rust shared things that can change must be protected. `RwLock` is a *read-write lock*. Any number of threads can read through it at the same time, or one thread can write, never both. It's the same rule as `&` and `&mut`, enforced while the program runs, because the compiler can't know which commands will run when. Rust won't let you share an unprotected value between threads at all. The program doesn't compile.

The scan command stores its tree when it's done.

```text
74      *app.state::<AppState>().tree.write().unwrap() = Some(tree);
```

`app.state::<AppState>()` fetches our state. `write()` waits until nobody is reading, then gives us the lock, and `unwrap()` deals with the one way that can fail (another thread panicked while holding the lock, which shouldn't happen). The `*` means "the thing the lock guards", and we replace it with the new tree. The lock is released at the end of the line, when the temporary guard goes away.

### Looking at a folder

The second command lets the page look at a folder.

```text
87  #[tauri::command]
88  pub fn folder(state: State<AppState>, id: Option<NodeId>) -> Result<FolderView, String> {
89      with_tree(&state, |tree| {
90          let id = id.unwrap_or(tree.root());
91          check(tree, id)?;
92          Ok(FolderView {
93              node: NodeView::new(tree, id),
94              path: tree.path(id),
95              children: tree[id]
96                  .children
97                  .iter()
98                  .map(|&c| NodeView::new(tree, c))
99                  .collect(),
100         })
101     })
102 }
```

`State<AppState>` [line 88] asks Tauri for our state. Tauri sees the argument's type and passes it in; the page never knows. `id` comes from the page. It's an `Option`, so the page can leave it out to mean "the folder that was scanned".

`with_tree` is a small helper [lines 42 to 46]. It takes the read lock, complains if there's no tree yet, and runs a closure with the tree. Every command that looks at the tree uses it, so that boring part is written once. `impl FnOnce(&Tree) -> Result<T, String>` means "any closure that takes a tree and returns a result". `T` is a *generic* type: whatever type the closure returns, `with_tree` returns too.

`check` [line 91] makes sure the id really is in the tree before we index with it, so a bad id from the page gets an error message rather than crashing the app.

`NodeView` is what the page gets for each node.

```text
14  #[derive(Serialize)]
15  #[serde(rename_all = "camelCase")]
16  pub struct NodeView {
17      id: NodeId,
18      name: String,
19      size: u64,
20      files: u64,
21      kind: Kind,
22      unreadable: bool,
23      has_children: bool,
24  }
```

Why not send `Node` itself? It has `OsString` names, which JSON can't hold, and ids of children the page doesn't need. A view type gives the page exactly what it needs, in the shape it wants. `rename_all = "camelCase"` sends `has_children` as `hasChildren`, the way TypeScript code normally spells it. And `display_name()` turns the raw name into readable text, swapping any bytes that aren't valid text for `�`.

### The page

The page asks for the scanned folder and lists the biggest things in it.

```text
28  async function showResults(summary: api.ScanSummary) {
29    const folder = await api.folder();
...
35    $("#biggest").replaceChildren(
36      ...folder.children.slice(0, 15).map((child) =>
37        h(
38          "div",
39          { class: "item" },
40          icon(child.kind === "dir" ? "folder" : "file"),
41          h("span", { class: "label" }, child.name),
42          h("span", { class: "size" }, formatSize(child.size)),
43        ),
44      ),
45    );
```

`h` from `dom.ts` makes an element: a tag, some attributes, and the things to put inside it. Anything that's text goes in as text, never as HTML. That matters. File names come from your disk, and a file can be called `<img src=x onerror=alert(1)>.txt`. If we built HTML by gluing strings together, that name would run as code inside our app. With `h` it's just a funny name.

### The command line

The command line tool now shows the top twenty.

```text
39      for &id in root.children.iter().take(count) {
40          let node = &tree[id];
41          let slash = if node.children.is_empty() { "" } else { "/" };
42          println!(
43              "{:>8}  {}  {}{slash}",
44              format_size(node.size),
45              bar(node.size, root.size, 20),
46              node.display_name()
47          );
48      }
```

`for &id in ...` [line 39] takes each `&NodeId` from the list and copies the id out; `NodeId` is `Copy`, so that's free. `take(count)` stops after twenty. `{:>8}` right-aligns the size in eight characters so the bars line up. `if` is an expression in Rust, so line 41 picks one of two strings and puts it straight into a variable.

`bar` draws the bar out of two characters, and `thousands` puts commas in big numbers. Both have tests at the bottom of the file.

## Adding functionality

Let's show how many files each folder holds in the terminal list.

1. In `cli/src/main.rs`, find the `println!` in `top`.
2. Add a column, `{:>10}`, after the bar, and pass it `thousands(node.files)`.
3. Run it on your home folder.

It's often surprising which folder has the most *files*, as opposed to the most bytes. It's usually not the one with the most bytes.

## Exercise

Make the command line tool show the biggest things *two* levels down: for each of the top ten, list its own top three, indented underneath it.

*Hint: In the loop in `top`, after printing a node, loop over `tree[id].children.iter().take(3)` and print each with some spaces in front. The tree's children are already sorted.*

## That's it

We have a tree, and Rust is happy with it. The scan is no faster than before, though, and on a big disk you wait. Your computer has eight, ten, maybe sixteen cores, and we're using one. Next chapter we use them all, and the owned `Entry` tree we built here turns out to be exactly the right shape for it.

[Chapter 6: Going parallel](../06-going-parallel/README.md)
