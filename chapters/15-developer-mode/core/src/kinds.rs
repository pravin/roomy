use std::path::Path;

use serde::Serialize;

use crate::tree::{Kind, NodeId, Tree};

/// Rough groups of files, so the treemap can colour them and we can say
/// "you have 40 GB of video".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Folder,
    Video,
    Image,
    Audio,
    Archive,
    Document,
    Code,
    App,
    Other,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Folder,
        Category::Video,
        Category::Image,
        Category::Audio,
        Category::Archive,
        Category::Document,
        Category::Code,
        Category::App,
        Category::Other,
    ];

    /// Guesses a file's category from the end of its name.
    pub fn from_name(name: &str) -> Category {
        let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) else {
            return Category::Other;
        };
        match ext.to_ascii_lowercase().as_str() {
            "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" | "flv" => Category::Video,
            "jpg" | "jpeg" | "png" | "gif" | "heic" | "webp" | "tif" | "tiff" | "bmp" | "raw"
            | "cr2" | "nef" | "arw" | "dng" | "psd" | "svg" => Category::Image,
            "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "aiff" => Category::Audio,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "dmg" | "iso"
            | "img" | "pkg" | "deb" | "rpm" | "msi" => Category::Archive,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "txt"
            | "md" | "rtf" | "epub" | "csv" | "pages" | "numbers" | "key" => Category::Document,
            "rs" | "c" | "h" | "cpp" | "hpp" | "js" | "ts" | "tsx" | "jsx" | "py" | "go"
            | "java" | "kt" | "swift" | "rb" | "php" | "cs" | "json" | "toml" | "yaml" | "yml"
            | "html" | "css" | "sh" | "sql" | "o" | "a" | "rlib" | "rmeta" | "pyc" | "class"
            | "jar" | "wasm" => Category::Code,
            "exe" | "dll" | "so" | "dylib" | "app" | "appimage" | "apk" | "ipa" => Category::App,
            _ => Category::Other,
        }
    }

    pub fn of(tree: &Tree, id: NodeId) -> Category {
        let node = &tree[id];
        match node.kind {
            // A macOS app is a folder, but nobody thinks of it as one.
            Kind::Dir if node.name.to_string_lossy().ends_with(".app") => Category::App,
            Kind::Dir => Category::Folder,
            _ => Category::from_name(&node.name.to_string_lossy()),
        }
    }
}

/// How much space each category takes up below a folder.
#[derive(Clone, Debug, Serialize)]
pub struct CategoryTotal {
    pub category: Category,
    pub size: u64,
    pub files: u64,
}

pub fn totals(tree: &Tree, id: NodeId) -> Vec<CategoryTotal> {
    let mut totals: Vec<CategoryTotal> = Category::ALL
        .iter()
        .filter(|&&c| c != Category::Folder)
        .map(|&category| CategoryTotal {
            category,
            size: 0,
            files: 0,
        })
        .collect();

    for node_id in tree.descendants(id) {
        let node = &tree[node_id];
        if node.kind == Kind::Dir {
            continue;
        }
        let category = Category::from_name(&node.name.to_string_lossy());
        let total = totals.iter_mut().find(|t| t.category == category).unwrap();
        total.size += node.size;
        total.files += 1;
    }
    totals.retain(|t| t.files > 0);
    totals.sort_by_key(|t| std::cmp::Reverse(t.size));
    totals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_case_blind() {
        assert_eq!(Category::from_name("Holiday.MOV"), Category::Video);
        assert_eq!(Category::from_name("main.rs"), Category::Code);
        assert_eq!(Category::from_name("Makefile"), Category::Other);
        assert_eq!(Category::from_name(".bashrc"), Category::Other);
    }

    #[test]
    fn totals_add_up() {
        let tree = crate::tree::tests::sample();
        let totals = totals(&tree, tree.root());
        assert_eq!(totals[0].category, Category::Image);
        assert_eq!(totals[0].size, 500);
        assert_eq!(totals.iter().map(|t| t.files).sum::<u64>(), 4);
    }
}
