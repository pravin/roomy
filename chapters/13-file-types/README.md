# Chapter 13 - File types

[< Chapter 12: Drawing the treemap](../12-drawing-the-treemap/README.md)

The treemap has shape, but every file is the same blue. A big blue box could be a film you forgot about or a database you need. This chapter sorts files into kinds by the ends of their names, gives each kind a colour, and adds a legend underneath the map that says how much of each you have. Point at a kind in the legend and everything else on the map fades, so you can see where it all is.

In this chapter you will

- write an `enum` that Rust sends to TypeScript as a plain string,
- `match` on file extensions,
- write your own iterator, to walk everything below a folder,
- add up the space each kind of file takes,
- colour the treemap and the list, and add a legend that highlights.

## Before we begin

One new Rust file, `core/src/kinds.rs`. `core/src/tree.rs` gets a `descendants` iterator. In the app, `commands.rs` sends a `category` with every node and every tile, and has a new `categories` command. `treemap.ts` swaps its one blue for a table of colours, `browse.ts` draws the legend, and `index.html` has a place to put it.

```text
cd chapters/13-file-types/app
npm install
npm run tauri dev
```

Scan your home folder. The map has colours now: red for video, orange for images, purple for audio, green for code, blue for documents and so on. Under the map is a row of chips, biggest first, with the total for each kind. Move the mouse over *Video* and every box that isn't video goes grey. It's usually a surprise.

![The treemap coloured by file type, with images highlighted](images/screenshot.png)

## Breaking it up

### The categories

```text
 9  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
10  #[serde(rename_all = "lowercase")]
11  pub enum Category {
12      Folder,
13      Video,
14      Image,
15      Audio,
16      Archive,
17      Document,
18      Code,
19      App,
20      Other,
21  }
```

An `enum` is a type that is exactly one of a fixed list of things. We met one in chapter 3, `ScanError`, whose variants carried data. These don't carry anything. They're just names, and Rust stores each one as a single byte.

serde turns an enum like this into a string, `"Video"`. `rename_all = "lowercase"` [line 10] makes it `"video"`, which is what JavaScript people expect. In `api.ts` the same list is a TypeScript union type:

```ts
export type Category =
  | "folder" | "video" | "image" | "audio" | "archive"
  | "document" | "code" | "app" | "other";
```

If the two lists ever disagree, TypeScript will happily accept a string it doesn't know, so keep them in step by hand. (There are crates that generate the TypeScript from the Rust, like `ts-rs` and `specta`. For one enum it isn't worth it.)

`Eq` and `Hash` are there so a `Category` could be a key in a `HashMap`. We don't use one in the end, but it costs nothing and you'll want it the moment you do.

### Matching on names

```text
37      pub fn from_name(name: &str) -> Category {
38          let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) else {
39              return Category::Other;
40          };
41          match ext.to_ascii_lowercase().as_str() {
42              "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" | "flv" => Category::Video,
```

`Path::extension` [line 38] gives back the part after the last dot, if there is one. It knows that `.bashrc` is a hidden file with no extension, not a file called nothing with the extension `bashrc`, which is the kind of thing I'd get wrong with a quick `split('.')`.

It returns an `Option<&OsStr>`, because file names don't have to be text (remember chapter 5). `and_then(|e| e.to_str())` turns that into an `Option<&str>`, which is `None` if there isn't an extension *or* it isn't valid UTF-8. `let ... else` [line 38] says "if this doesn't match, do the `else` and leave". It's the tidiest way to give up early in Rust.

Then a `match` on the lowercased string [line 41]. `|` means "or". Each arm is a list of extensions and the category they belong to, and `_` [line 55] catches everything else.

The lists are a matter of taste. I put `.json` and `.yaml` under code, and `.o` and `.rlib`, because they nearly always turn up in a programming project. You'll disagree with some of them. Change them.

### Apps that are folders

```text
59      pub fn of(tree: &Tree, id: NodeId) -> Category {
60          let node = &tree[id];
61          match node.kind {
62              // A macOS app is a folder, but nobody thinks of it as one.
63              Kind::Dir if node.name.to_string_lossy().ends_with(".app") => Category::App,
64              Kind::Dir => Category::Folder,
65              _ => Category::from_name(&node.name.to_string_lossy()),
66          }
67      }
```

On a Mac, `Xcode.app` is a folder with ten thousand files in it, but Finder shows it as a single thing, and so should we. The `if` on line 63 is a *match guard*. The arm only matches when the pattern matches *and* the guard is true. If it isn't, Rust tries the next arm, so every other folder ends up at line 64.

### Walking everything below a folder

To add up the space each kind takes, we need every file below a folder, however deep. The tree's nodes are in one big `Vec`, but a folder's contents aren't next to each other in it, so we have to walk. Chapter 2 walked with recursion. This time we write an *iterator*, so that the caller can use it in a `for` loop.

```text
155  /// Walks a part of the tree, depth first. Made by `Tree::descendants`.
156  pub struct Descendants<'a> {
157      tree: &'a Tree,
158      stack: Vec<NodeId>,
159  }
160
161  impl Iterator for Descendants<'_> {
162      type Item = NodeId;
163
164      fn next(&mut self) -> Option<NodeId> {
165          let id = self.stack.pop()?;
166          self.stack
167              .extend(self.tree[id].children.iter().rev().copied());
168          Some(id)
169      }
170  }
```

An iterator in Rust is anything that implements the `Iterator` trait. That means one method, `next`, which hands back `Some(item)` or `None` when it's finished. Everything else you get for free: `for` loops, `map`, `filter`, `count`, `sum` and the other fifty-odd methods.

`Descendants` keeps a *stack* of nodes still to visit. Each call pops one [line 165], pushes its children [line 167], and returns it. The `?` on `pop()` returns `None` from `next` when the stack is empty, which ends the iteration. We push the children backwards (`rev`), so the biggest comes off the stack first.

`'a` [line 156] is a *lifetime*. `Descendants` holds a reference to the tree, and Rust needs to be told that the iterator can't outlive the tree it walks. `<'a>` names "however long the tree is borrowed for", and `&'a Tree` says our reference lasts that long. You don't choose the lifetime, Rust works it out where you call `descendants`. You just have to admit there is one.

Why not recursion again? A stack on the heap can be as deep as you like. Recursion uses the thread's stack, and a ridiculously deep folder tree could overflow it. Also, you can't easily stop a recursive function halfway and pick it up later, which is exactly what an iterator does every time it returns.

### Adding up

```text
78  pub fn totals(tree: &Tree, id: NodeId) -> Vec<CategoryTotal> {
79      let mut totals: Vec<CategoryTotal> = Category::ALL
80          .iter()
81          .filter(|&&c| c != Category::Folder)
82          .map(|&category| CategoryTotal {
83              category,
84              size: 0,
85              files: 0,
86          })
87          .collect();
88
89      for node_id in tree.descendants(id) {
90          let node = &tree[node_id];
91          if node.kind == Kind::Dir {
92              continue;
93          }
94          let category = Category::from_name(&node.name.to_string_lossy());
95          let total = totals.iter_mut().find(|t| t.category == category).unwrap();
96          total.size += node.size;
97          total.files += 1;
98      }
99      totals.retain(|t| t.files > 0);
100     totals.sort_by_key(|t| std::cmp::Reverse(t.size));
101     totals
```

We start with a zero total for every kind except folders [lines 79 to 87], then walk every file and add it to its kind's total. `iter_mut` [line 95] gives mutable references, so we can change the total in place. With eight kinds a `find` is as quick as a hash map would be. Finally we drop the kinds with no files [line 99] and sort biggest first. `Reverse` flips the order, so the sort goes high to low.

`|&&c|` on line 81 looks odd. `iter()` gives references to the items, and `filter` passes its closure a reference to *those*, so `c` arrives as `&&Category`. Writing `&&c` in the pattern unwraps both, and since `Category` is `Copy`, we get a plain value.

The `categories` command [commands.rs, lines 224 to 230] does nothing but call `totals` for the folder you're looking at.

### Colours

`treemap.ts` has a table with one colour for each category, and another with the names to show.

```ts
export const COLOURS: Record<Category, string> = {
  folder: "#7d8796",
  video: "#e5566c",
  ...
```

`Record<Category, string>` is TypeScript for "an object with exactly one string for each category". Leave one out and it won't compile, which is the next best thing to the Rust and TypeScript lists checking each other.

I picked colours that are easy to tell apart, work on both light and dark backgrounds, and don't lean on red against green, which a lot of people can't tell apart. Video is red because films are usually the biggest thing on a disk and you want to spot them.

### Highlighting

```text
166    private paintFile(ctx: CanvasRenderingContext2D, t: Tile) {
167      const faded = this.highlight !== null && this.highlight !== t.category;
168      const colour = COLOURS[t.category];
169      if (faded) {
170        ctx.fillStyle = dark.matches ? "#2a2f37" : "#dde1e6";
171        ctx.fillRect(t.x + 0.5, t.y + 0.5, t.w - 1, t.h - 1);
172        return;
173      }
```

When a category is highlighted, files of any other kind are painted flat grey. The legend chips set it when the mouse comes in and clear it when it leaves (`browse.ts`, [lines 118 and 119]). `setHighlight` [treemap.ts, line 124] repaints the base canvas from last chapter, which takes a few milliseconds even with thousands of boxes, so it keeps up with the mouse.

## Adding functionality

Let's give fonts a category of their own.

1. In `kinds.rs`, add `Font` to the enum after `App`, and to `ALL`, which becomes `[Category; 10]`.
2. Add an arm to the `match`: `"ttf" | "otf" | "woff" | "woff2" => Category::Font,`.
3. In `api.ts`, add `| "font"` to the `Category` type.
4. In `treemap.ts`, add `font: "#d4c34a"` to `COLOURS` and `font: "Fonts"` to `NAMES`.

TypeScript will complain until you do step 4, because of that `Record`. Rust won't complain if you forget step 1's change to `ALL`, though. That one you have to check by hand.

## Exercise

Make the legend clickable: clicking a chip should keep the highlight on until you click it again, instead of only while the mouse is over it.

*Hint: Keep the clicked category in a variable in `browse.ts`. On `mouseleave`, set the highlight back to that variable instead of `null`.*

## That's it

Now you can see what's taking the space, and what kind of thing it is. The obvious next question is "can I get rid of it?". Next chapter we add buttons to show a file in Finder or Explorer, open it, and move it to the Trash.

[Chapter 14: Safe delete](../14-safe-delete/README.md)
