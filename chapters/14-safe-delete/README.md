# Chapter 14 - Safe delete

[< Chapter 13: File types](../13-file-types/README.md)

Roomy can show you what's filling your disk. Now it can help you empty it. This chapter adds three buttons to the bar at the bottom: one to open a file, one to show it in Finder or Explorer, and one to move it to the Trash.

That last one deserves some care. A program that deletes files is a program that can delete the *wrong* files. So Roomy never deletes anything for good. Everything goes to the Trash (the Recycle Bin on Windows), where you can get it back, and it always asks first.

In this chapter you will

- take a node out of the tree and make every folder above it smaller,
- see why storing parents in an arena pays off,
- move files to the Trash with the `trash` crate,
- show and open files with Tauri's opener plugin,
- take a *write* lock on the tree,
- ask before doing anything, with a native dialog.

## Before we begin

One new TypeScript file, `app/src/actions.ts`. In Rust, `core/src/tree.rs` gets `remove` and `contains`, and `commands.rs` gets `reveal`, `open_item` and `trash`. The app's `Cargo.toml` gains two dependencies, `trash` and `tauri-plugin-opener`, and `main.rs` switches the plugin on. `browse.ts` adds the buttons.

```text
cd chapters/14-safe-delete/app
npm install
npm run tauri dev
```

Scan a folder you don't mind losing a file from. Make a test one if you like:

```text
mkdir -p /tmp/roomy-test && cd /tmp/roomy-test
head -c 50000000 /dev/zero > big.bin
head -c 5000000 /dev/zero > small.bin
```

Click `big.bin`. The bar at the bottom now has *Open*, *Show in Finder* and *Move to Trash*. Click *Move to Trash*, read the question, and say yes. The file disappears from the list and the map, the total at the bottom drops by 50 MB, and if you look in the Trash, there it is.

![A folder picked, with Open, Show in Finder and Move to Trash](images/screenshot.png)

## Breaking it up

### Taking a node out of the tree

```text
148      pub fn remove(&mut self, id: NodeId) -> u64 {
149          let Some(parent) = self[id].parent else {
150              return 0; // Can't remove the root
151          };
152          let (size, files) = (self[id].size, self[id].files);
153          self.nodes[parent.index()].children.retain(|&c| c != id);
154          self.nodes[id.index()].parent = None;
155
156          let mut current = Some(parent);
157          while let Some(p) = current {
158              let node = &mut self.nodes[p.index()];
159              node.size -= size;
160              node.files -= files;
161              current = node.parent;
162          }
163          size
164      }
```

When a file goes, every folder that contains it gets smaller by the same amount: its folder, that folder's folder, and so on up to the one you scanned. Without that, the totals would be wrong until you scanned again.

Here's where the work in chapter 5 pays off. Back then we turned the tree of nested `Entry`s into a flat list of `Node`s that point at each other by number, so that a child could know its parent. In a nested tree, a child has no way to reach the folder it's in. Here we need exactly that: we start at the parent [line 156] and climb, following `parent` each time [line 161], until we run out of parents at the root.

`while let` [line 157] is a loop that keeps going as long as the pattern matches. It's `if let` that repeats. When `current` becomes `None`, the loop stops.

`retain` [line 153] keeps the children that pass the test and drops the rest, so the node is no longer one of its parent's children. We also clear its own `parent` [line 154]. The node itself stays in the `Vec`. Taking it out would shift every node after it down by one, and then every `NodeId` pointing past it would point at the wrong thing. A node that nobody can reach is harmless, and it costs a few bytes until the next scan.

Line 158 borrows a node *mutably*, `&mut self.nodes[...]`, so we can change it. That's only allowed because nothing else is borrowing `self.nodes` at the same time. If you tried to hold on to `node` and look at another node in the same loop, the compiler would stop you. Getting the parent's number out [line 161] before the next turn of the loop is what keeps it happy.

### Is it still there?

```text
168      pub fn contains(&self, id: NodeId) -> bool {
169          if id.index() >= self.nodes.len() {
170              return false;
171          }
172          let top = *self.ancestors(id).first().unwrap();
173          top == self.root()
174      }
```

The window holds on to `NodeId`s: what's picked, what's in the list. After a delete some of them point at nodes that are gone. A removed node has no parent, and anything that was inside it still has its parents, but the chain stops at the removed node instead of the root. So "is it still in the tree?" is "if I climb all the way up, do I reach the root?" [lines 172 and 173]. `ancestors` has been there since chapter 5, where `path` uses it, and chapter 10's breadcrumbs use it too.

`check` in `commands.rs` now uses `contains` instead of `get` [line 67], so every command turns down a node that's been deleted with a plain message instead of showing something that isn't there.

### The write lock

Back in chapter 5 we put the tree in a `RwLock`. Until now everybody only read it. `trash` changes it, so it needs the other half of the lock.

```text
57  fn with_tree_mut<T>(
58      state: &AppState,
59      f: impl FnOnce(&mut Tree) -> Result<T, String>,
60  ) -> Result<T, String> {
61      let mut tree = state.tree.write().unwrap();
62      let tree = tree.as_mut().ok_or("nothing has been scanned yet")?;
63      f(tree)
64  }
```

It's `with_tree` with `write()` instead of `read()` and `&mut` instead of `&`. Many readers can hold the lock at once, but a writer has it to itself. While `trash` is running, any other command that wants the tree waits until it's done. That's exactly what we want. Nobody should lay out a treemap from a tree that's halfway through losing a folder.

### Moving things to the Trash

```text
276  pub fn trash(state: State<AppState>, ids: Vec<NodeId>) -> Result<TrashResult, String> {
277      with_tree_mut(&state, |tree| {
278          let mut result = TrashResult {
279              freed: 0,
280              failed: Vec::new(),
281          };
282          for id in ids {
283              // Skip anything already gone, say because its folder went first.
284              if !tree.contains(id) || id == tree.root() {
285                  continue;
286              }
287              let path = tree.path(id);
288              match trash::delete(&path) {
289                  Ok(()) => result.freed += tree.remove(id),
290                  Err(e) => result.failed.push(format!("{}: {e}", path.display())),
291              }
292          }
293          Ok(result)
294      })
295  }
```

The [`trash`](https://crates.io/crates/trash) crate does the platform work. On macOS it asks Finder to do it, so *Put Back* works. On Windows it uses the shell's Recycle Bin. On Linux it follows the freedesktop.org Trash specification, the same one your file manager uses. We never call `std::fs::remove_file`, which deletes for good. I'd rather Roomy free space one step later than be the app that ate somebody's thesis.

It takes a list of ids, because later chapters move a hundred things at once. Each one can fail on its own (a file in use on Windows, say), so instead of stopping at the first failure we keep going and collect the failures [line 290]. The tree only changes for things that actually went [line 289]. The window gets back how much was freed and what didn't work.

We never trash the root [line 284]. You can't delete the folder you're looking at from inside itself.

### Show and open

```text
246  pub fn reveal(app: AppHandle, state: State<AppState>, id: NodeId) -> Result<(), String> {
247      let path = with_tree(&state, |tree| {
248          check(tree, id)?;
249          Ok(tree.path(id))
250      })?;
251      app.opener()
252          .reveal_item_in_dir(path)
253          .map_err(|e| e.to_string())
254  }
```

Tauri's opener plugin knows how to open a Finder or Explorer window with a file selected, and how to open a file in whatever app usually opens it. `app.opener()` comes from the `OpenerExt` trait, which the plugin adds to `AppHandle`. That's a pattern you'll see a lot in Tauri: a plugin is switched on in `main.rs` with `.plugin(tauri_plugin_opener::init())`, and then its methods appear on the app handle.

We get the path inside `with_tree` and do the opening *outside* it [lines 247 to 253], so we don't hold the lock while Finder takes its time.

`open_item` [line 258] is the same with `open_path`. Its second argument picks which app to open with, and `None::<&str>` [line 264] means "the usual one". The `::<&str>` is there because a bare `None` doesn't say what type it would have held, and Rust needs to know.

We call both from Rust, so the window doesn't need permission to use the opener itself. Rust only opens paths it found in its own scan.

### Asking first

`actions.ts` holds things any part of the window can do to a file. For now that's moving to the Trash.

```text
20  export async function trashItems(ids: api.NodeId[], what: string, size: number): Promise<number | null> {
21    const sure = await ask(
22      `Move ${what} (${formatSize(size)}) to the ${trashName}?\n\nYou can put things back from the ${trashName} until you empty it.`,
23      { title: "Roomy", kind: "warning", okLabel: `Move to ${trashName}`, cancelLabel: "Cancel" },
24    );
25    if (!sure) return null;
```

`ask` comes from the dialog plugin we added for the folder picker in chapter 4. It shows a real system dialog, not a web page one, and waits for an answer. The button says *Move to Trash* rather than *OK*, so nobody has to read the question to know what they're agreeing to.

It even uses the right word. Windows calls it the Recycle Bin, everyone else calls it the Trash, and Finder is Explorer on Windows and Files on Linux [lines 10 to 16]. We guess from the browser's user agent, which is the one thing a web page always knows about the system it's on.

Back in `browse.ts`, `remove` [line 202] calls `trashItems` and, if anything happened, `refresh`es the folder. If the folder you were looking at has itself gone, `refresh` goes back to the top.

## Adding functionality

Let's give the Delete key a job.

1. In `browse.ts`, export a function `removePicked()` that calls `remove(picked)` when `picked` isn't `null`.
2. In `main.ts`, in the `keydown` handler, call it when `e.key === "Delete"`, or when `e.key === "Backspace"` and `e.metaKey` is held (that's ⌘⌫, the Mac's own shortcut for Move to Trash).

The dialog still asks first, so it's safe to press by accident.

## Exercise

Show how much you've freed this session in the bar at the bottom: "Freed 3.2 GB so far".

*Hint: `trashItems` returns the bytes freed. Add them up in a variable in `browse.ts` and show it in `renderDetails` when nothing is picked.*

## That's it

You can now find a big file and get rid of it. But the biggest wins for programmers aren't single files. They're thousands of `node_modules` and `target` folders, spread all over your disk, that you'll never need again. Next chapter Roomy learns to find them all at once.

[Chapter 15: Developer mode](../15-developer-mode/README.md)
