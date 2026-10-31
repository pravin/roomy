//! Developer mode: folders that programming tools fill up and that you can
//! delete without losing anything, because the tool will make them again.

use std::path::Path;

use serde::Serialize;

use crate::tree::{Kind, NodeId, Tree};

/// Something that can spot one sort of junk folder.
pub trait Detector: Send + Sync {
    /// A short name, like "node_modules".
    fn name(&self) -> &str;
    /// What the folder is and how to get it back, in one sentence.
    fn description(&self) -> &str;
    /// Is this folder one of ours?
    fn matches(&self, tree: &Tree, id: NodeId) -> bool;
}

/// A folder a build tool makes inside a project, like `node_modules` next
/// to `package.json`. Requiring the project file means we leave alone any
/// folder that only happens to have the same name.
pub struct ProjectArtifact {
    pub folder: &'static str,
    /// At least one of these must sit next to the folder. Empty means no check.
    pub next_to: &'static [&'static str],
    /// At least one of these must be inside the folder. Empty means no check.
    pub contains: &'static [&'static str],
    pub description: &'static str,
}

impl Detector for ProjectArtifact {
    fn name(&self) -> &str {
        self.folder
    }

    fn description(&self) -> &str {
        self.description
    }

    fn matches(&self, tree: &Tree, id: NodeId) -> bool {
        let node = &tree[id];
        if node.kind != Kind::Dir || node.name != self.folder {
            return false;
        }
        let has_child = |folder: NodeId, names: &[&str]| {
            names.is_empty()
                || tree[folder]
                    .children
                    .iter()
                    .any(|&c| names.iter().any(|n| tree[c].name == *n))
        };
        let Some(parent) = node.parent else {
            return false; // The folder you scanned is never junk
        };
        has_child(parent, self.next_to) && has_child(id, self.contains)
    }
}

/// A cache that lives at a fixed place in your home folder.
pub struct KnownCache {
    pub name: &'static str,
    /// The end of its path, like `.npm/_cacache`.
    pub path: &'static str,
    pub description: &'static str,
}

impl Detector for KnownCache {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn matches(&self, tree: &Tree, id: NodeId) -> bool {
        let node = &tree[id];
        let wanted = Path::new(self.path);
        // Building the full path is slow, so check the last part first.
        node.kind == Kind::Dir
            && node.parent.is_some()
            && wanted.file_name() == Some(node.name.as_os_str())
            && tree.path(id).ends_with(wanted)
    }
}

/// Every detector Roomy knows about.
pub fn detectors() -> Vec<Box<dyn Detector>> {
    const JS: &[&str] = &["package.json"];
    const GRADLE: &[&str] = &[
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ];
    vec![
        Box::new(ProjectArtifact {
            folder: "node_modules",
            next_to: JS,
            contains: &[],
            description: "JavaScript packages. Run `npm install` to get them back.",
        }),
        Box::new(ProjectArtifact {
            folder: "target",
            next_to: &["Cargo.toml"],
            contains: &[],
            description: "Rust build output. `cargo build` makes it again.",
        }),
        Box::new(ProjectArtifact {
            folder: ".venv",
            next_to: &[],
            contains: &["pyvenv.cfg"],
            description: "A Python virtual environment. Recreate it with `python -m venv`.",
        }),
        Box::new(ProjectArtifact {
            folder: "venv",
            next_to: &[],
            contains: &["pyvenv.cfg"],
            description: "A Python virtual environment. Recreate it with `python -m venv`.",
        }),
        Box::new(ProjectArtifact {
            folder: "__pycache__",
            next_to: &[],
            contains: &[],
            description: "Compiled Python. Python makes it again when it needs it.",
        }),
        Box::new(ProjectArtifact {
            folder: ".next",
            next_to: JS,
            contains: &[],
            description: "Next.js build output. Made again by `next build` or `next dev`.",
        }),
        Box::new(ProjectArtifact {
            folder: ".svelte-kit",
            next_to: JS,
            contains: &[],
            description: "SvelteKit build output. Made again by `vite dev`.",
        }),
        Box::new(ProjectArtifact {
            folder: ".parcel-cache",
            next_to: JS,
            contains: &[],
            description: "Parcel's cache. Parcel fills it again on the next build.",
        }),
        Box::new(ProjectArtifact {
            folder: "build",
            next_to: GRADLE,
            contains: &[],
            description: "Gradle build output. `gradle build` makes it again.",
        }),
        Box::new(ProjectArtifact {
            folder: ".gradle",
            next_to: GRADLE,
            contains: &[],
            description: "Gradle's per-project cache.",
        }),
        Box::new(ProjectArtifact {
            folder: "Pods",
            next_to: &["Podfile"],
            contains: &[],
            description: "CocoaPods dependencies. Run `pod install` to get them back.",
        }),
        Box::new(ProjectArtifact {
            folder: ".dart_tool",
            next_to: &["pubspec.yaml"],
            contains: &[],
            description: "Dart and Flutter tool output. Made again by `flutter pub get`.",
        }),
        Box::new(ProjectArtifact {
            folder: ".zig-cache",
            next_to: &["build.zig"],
            contains: &[],
            description: "Zig's build cache.",
        }),
        Box::new(KnownCache {
            name: "Xcode DerivedData",
            path: "Library/Developer/Xcode/DerivedData",
            description: "Xcode's build products and indexes. Xcode rebuilds them.",
        }),
        Box::new(KnownCache {
            name: "npm cache",
            path: ".npm/_cacache",
            description: "Every package npm has ever downloaded.",
        }),
        Box::new(KnownCache {
            name: "Cargo registry",
            path: ".cargo/registry",
            description: "Downloaded Rust crates. Cargo downloads them again when needed.",
        }),
        Box::new(KnownCache {
            name: "Gradle caches",
            path: ".gradle/caches",
            description: "Downloaded Gradle dependencies and build caches.",
        }),
        Box::new(KnownCache {
            name: "pip cache",
            path: ".cache/pip",
            description: "Every Python package pip has downloaded.",
        }),
        Box::new(KnownCache {
            name: "pip cache",
            path: "Library/Caches/pip",
            description: "Every Python package pip has downloaded.",
        }),
        Box::new(KnownCache {
            name: "Homebrew cache",
            path: "Library/Caches/Homebrew",
            description: "Downloads Homebrew kept after installing. `brew cleanup` empties it too.",
        }),
        Box::new(KnownCache {
            name: "Yarn cache",
            path: "Library/Caches/Yarn",
            description: "Every package Yarn has downloaded.",
        }),
        Box::new(KnownCache {
            name: "Yarn cache",
            path: ".cache/yarn",
            description: "Every package Yarn has downloaded.",
        }),
    ]
}

/// One junk folder we found.
#[derive(Clone, Debug, Serialize)]
pub struct JunkItem {
    pub id: NodeId,
    pub name: String,
    pub description: String,
    pub size: u64,
}

/// Looks for junk below `start`, biggest first. Once a folder is junk we
/// don't look inside it, so a `node_modules` inside a `node_modules` only
/// counts once.
pub fn find(tree: &Tree, start: NodeId, detectors: &[Box<dyn Detector>]) -> Vec<JunkItem> {
    let mut found = Vec::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        for &child in &tree[id].children {
            if tree[child].kind != Kind::Dir {
                continue;
            }
            match detectors.iter().find(|d| d.matches(tree, child)) {
                Some(detector) => found.push(JunkItem {
                    id: child,
                    name: detector.name().to_string(),
                    description: detector.description().to_string(),
                    size: tree[child].size,
                }),
                None => stack.push(child),
            }
        }
    }
    found.sort_by_key(|j| std::cmp::Reverse(j.size));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::tests::{dir, file};

    fn projects() -> Tree {
        Tree::from_entry(
            "/home/me".into(),
            dir(
                "/home/me",
                vec![
                    dir(
                        "website",
                        vec![
                            file("package.json", 1),
                            dir("node_modules", vec![file("react.js", 500)]),
                        ],
                    ),
                    // Not a JavaScript project, so not junk
                    dir("notes", vec![dir("node_modules", vec![file("x", 50)])]),
                    dir(
                        "game",
                        vec![
                            file("Cargo.toml", 1),
                            dir("target", vec![file("game", 900)]),
                        ],
                    ),
                    dir(".npm", vec![dir("_cacache", vec![file("blob", 70)])]),
                ],
            ),
        )
    }

    #[test]
    fn finds_project_junk_and_caches() {
        let tree = projects();
        let found = find(&tree, tree.root(), &detectors());
        let names: Vec<&str> = found.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, ["target", "node_modules", "npm cache"]);
        assert_eq!(
            tree.path(found[1].id),
            Path::new("/home/me/website/node_modules")
        );
    }
}
