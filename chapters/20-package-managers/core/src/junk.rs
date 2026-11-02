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
        has_child(parent, self.next_to) && has_child(id, self.contains) && !off_limits(tree, parent)
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
    // A project you ran `npm install` in has a lockfile. A package someone
    // published, like npm itself, has a package.json but no lockfile.
    const JS_LOCKS: &[&str] = &[
        "package-lock.json",
        "npm-shrinkwrap.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "bun.lock",
        "bun.lockb",
    ];
    const GRADLE: &[&str] = &[
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ];
    vec![
        Box::new(ProjectArtifact {
            folder: "node_modules",
            next_to: JS_LOCKS,
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

/// Places Roomy leaves alone, as runs of folder names. Installed software
/// first: a project folder inside one belongs to a program you use, not to
/// a project of yours, and the program may stop working without it. npm,
/// for one, is itself a package with a `package.json` and a `node_modules`,
/// in `lib/node_modules/npm` inside every Node install.
const OFF_LIMITS: &[&[&str]] = &[
    &["node_modules"], // Packages inside packages: part of whatever installed them
    &["site-packages"],
    &["dist-packages"],
    &["mise", "installs"],
    &[".asdf", "installs"],
    &[".nvm", "versions"],
    &[".volta", "tools"],
    &["fnm", "node-versions"],
    &[".fnm", "node-versions"],
    &["pnpm", "global"],
    &[".bun", "install"],
    &[".pyenv", "versions"],
    &[".rustup"],
    &[".pub-cache"],
    &["fvm", "versions"],
    &["Android", "sdk"],
    &["google-cloud-sdk"],
    &[".linuxbrew"], // Homebrew on Linux; on a Mac it lives in /opt or /usr
    &[".vscode", "extensions"],
    &[".vscode-server"],
    &[".cursor", "extensions"],
    &["Program Files"],
    &["Program Files (x86)"],
    // And the Trash: it's already on its way out
    &[".Trash"],
    &[".Trashes"],
    &["$Recycle.Bin"],
    &["Trash", "files"],
];

/// Folders at the top of the disk that belong to the system or to installed
/// software, never to your projects.
const OFF_LIMITS_ROOTS: &[&str] = &[
    "/Applications",
    "/Library",
    "/System",
    "/usr",
    "/opt",
    "/nix",
    "/snap",
];

/// Should Roomy leave `id` and everything in it alone? Only called for
/// folders that already look like junk, so building the path here costs
/// next to nothing.
fn off_limits(tree: &Tree, id: NodeId) -> bool {
    let path = tree.path(id);
    if OFF_LIMITS_ROOTS.iter().any(|root| path.starts_with(root)) {
        return true;
    }
    let names: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    // An app bundle on a Mac, like Visual Studio Code.app, carries its own packages.
    if names.iter().any(|n| n.to_string_lossy().ends_with(".app")) {
        return true;
    }
    OFF_LIMITS.iter().any(|run| {
        names
            .windows(run.len())
            .any(|w| w.iter().zip(run.iter()).all(|(a, b)| *a == *b))
    })
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
                            file("package-lock.json", 1),
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

    /// The folders a Node install keeps npm in, as mise, nvm and Homebrew
    /// lay them out. Every one of them looks like a project, and none is junk.
    fn npm_inside(install: Vec<&str>) -> Tree {
        let npm = dir(
            "npm",
            vec![
                file("package.json", 1),
                // Some packages ship a lockfile, so that alone isn't enough
                file("npm-shrinkwrap.json", 1),
                dir("node_modules", vec![file("graceful-fs.js", 80)]),
            ],
        );
        let mut entry = dir("lib", vec![dir("node_modules", vec![npm])]);
        for name in install.into_iter().rev() {
            entry = dir(name, vec![entry]);
        }
        Tree::from_entry("/home/me".into(), dir("/home/me", vec![entry]))
    }

    #[test]
    fn leaves_npm_inside_node_installs_alone() {
        for install in [
            vec![".local", "share", "mise", "installs", "node", "25.9.0"],
            vec![".nvm", "versions", "node", "v22.13.1"],
            vec!["opt", "homebrew"],
        ] {
            let tree = npm_inside(install.clone());
            let found = find(&tree, tree.root(), &detectors());
            assert!(found.is_empty(), "{install:?}: {found:?}");
        }
    }

    #[test]
    fn needs_a_lockfile_for_node_modules() {
        let tree = Tree::from_entry(
            "/home/me".into(),
            dir(
                "/home/me",
                vec![dir(
                    "some-package",
                    vec![
                        file("package.json", 1),
                        dir("node_modules", vec![file("x.js", 50)]),
                    ],
                )],
            ),
        );
        assert!(find(&tree, tree.root(), &detectors()).is_empty());
    }

    #[test]
    fn leaves_apps_extensions_sdks_and_the_trash_alone() {
        let project = || {
            vec![
                file("package.json", 1),
                file("package-lock.json", 1),
                dir("node_modules", vec![file("x.js", 50)]),
            ]
        };
        let tree = Tree::from_entry(
            "/home/me".into(),
            dir(
                "/home/me",
                vec![
                    dir(
                        "Applications",
                        vec![dir(
                            "Visual Studio Code.app",
                            vec![dir("Contents", vec![dir("app", project())])],
                        )],
                    ),
                    dir(
                        ".vscode",
                        vec![dir("extensions", vec![dir("prettier", project())])],
                    ),
                    dir(".Trash", vec![dir("old-site", project())]),
                    dir(
                        "Tools",
                        vec![dir(
                            "google-cloud-sdk",
                            vec![dir("lib", vec![dir("__pycache__", vec![file("x.pyc", 9)])])],
                        )],
                    ),
                    dir(
                        ".venvs",
                        vec![dir("tool", vec![dir(".venv", vec![file("pyvenv.cfg", 1)])])],
                    ),
                ],
            ),
        );
        let found = find(&tree, tree.root(), &detectors());
        let paths: Vec<_> = found.iter().map(|j| tree.path(j.id)).collect();
        // Only the virtual environment, which isn't inside installed software
        assert_eq!(paths, [Path::new("/home/me/.venvs/tool/.venv")]);
    }

    #[test]
    fn leaves_the_system_alone() {
        let tree = Tree::from_entry(
            "/".into(),
            dir(
                "/",
                vec![dir(
                    "Library",
                    vec![dir(
                        "Frameworks",
                        vec![dir(
                            "Python.framework",
                            vec![dir("__pycache__", vec![file("x.pyc", 9)])],
                        )],
                    )],
                )],
            ),
        );
        assert!(find(&tree, tree.root(), &detectors()).is_empty());
    }
}
