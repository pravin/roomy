//! Squarified treemaps, as described by Mark Bruls, Kees Huizing and
//! Jarke J. van Wijk in 1999. Every file gets a box with an area in
//! proportion to its size, and the boxes are kept as close to square as we
//! can manage, because long thin slivers are hard to see and harder to click.

use serde::Serialize;

use crate::tree::{NodeId, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    fn area(&self) -> f64 {
        self.w * self.h
    }
}

/// Splits `rect` into one box per size. The sizes should be biggest first
/// and none of them zero. The boxes come back in the same order.
pub fn squarify(sizes: &[f64], rect: Rect) -> Vec<Rect> {
    let total: f64 = sizes.iter().sum();
    if sizes.is_empty() || total <= 0.0 || rect.area() <= 0.0 {
        return vec![Rect::new(rect.x, rect.y, 0.0, 0.0); sizes.len()];
    }

    // Turn sizes into areas, in the same units as the rectangle.
    let scale = rect.area() / total;
    let areas: Vec<f64> = sizes.iter().map(|s| s * scale).collect();

    let mut out = Vec::with_capacity(areas.len());
    let mut free = rect;
    let mut start = 0;
    while start < areas.len() {
        // Lay a row along the short side of whatever space is left. Keep
        // adding boxes to the row for as long as that makes the worst box
        // in it more square, not less.
        let side = free.w.min(free.h);
        let mut end = start + 1;
        let mut sum = areas[start];
        let mut current = worst(&areas[start..end], sum, side);
        while end < areas.len() {
            let next_sum = sum + areas[end];
            let next = worst(&areas[start..=end], next_sum, side);
            if next > current {
                break;
            }
            sum = next_sum;
            current = next;
            end += 1;
        }
        free = lay_row(&areas[start..end], sum, free, &mut out);
        start = end;
    }
    out
}

/// The worst aspect ratio in a row, where 1.0 means a perfect square.
fn worst(row: &[f64], sum: f64, side: f64) -> f64 {
    let (min, max) = row
        .iter()
        .fold((f64::MAX, 0.0f64), |(lo, hi), &a| (lo.min(a), hi.max(a)));
    let (sum2, side2) = (sum * sum, side * side);
    f64::max(side2 * max / sum2, sum2 / (side2 * min))
}

/// Places one row of boxes and returns the space that's left over.
fn lay_row(row: &[f64], sum: f64, free: Rect, out: &mut Vec<Rect>) -> Rect {
    if free.w >= free.h {
        // Wider than tall: the row is a column down the left hand side.
        let thickness = sum / free.h;
        let mut y = free.y;
        for &area in row {
            let h = area / thickness;
            out.push(Rect::new(free.x, y, thickness, h));
            y += h;
        }
        Rect::new(free.x + thickness, free.y, free.w - thickness, free.h)
    } else {
        // Taller than wide: the row goes along the top.
        let thickness = sum / free.w;
        let mut x = free.x;
        for &area in row {
            let w = area / thickness;
            out.push(Rect::new(x, free.y, w, thickness));
            x += w;
        }
        Rect::new(free.x, free.y + thickness, free.w, free.h - thickness)
    }
}

/// One box in a finished treemap.
#[derive(Clone, Debug, Serialize)]
pub struct Tile {
    pub id: NodeId,
    pub rect: Rect,
    /// 1 for the children of the folder we are looking at, 2 for their
    /// children, and so on.
    pub depth: u32,
}

/// A folder needs to be at least this big to have its name shown.
pub const LABEL_WIDTH: f64 = 64.0;
pub const LABEL_HEIGHT: f64 = 40.0;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// How many levels of folders to show at once.
    pub max_depth: u32,
    /// Boxes smaller than this, in pixels, aren't worth drawing.
    pub min_side: f64,
    /// Space between a folder's edge and the boxes inside it.
    pub padding: f64,
    /// Room at the top of each folder for its name.
    pub header: f64,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout {
            max_depth: 8,
            min_side: 3.0,
            padding: 1.5,
            header: 16.0,
        }
    }
}

/// Lays out the contents of the folder `id` in a `width` by `height` area.
/// Folders are drawn as boxes with their own contents inside them.
pub fn layout(tree: &Tree, id: NodeId, width: f64, height: f64, options: &Layout) -> Vec<Tile> {
    let mut tiles = Vec::new();
    layout_children(
        tree,
        id,
        Rect::new(0.0, 0.0, width, height),
        1,
        options,
        &mut tiles,
    );
    tiles
}

fn layout_children(
    tree: &Tree,
    id: NodeId,
    rect: Rect,
    depth: u32,
    options: &Layout,
    tiles: &mut Vec<Tile>,
) {
    if depth > options.max_depth {
        return;
    }
    // Children are already biggest first. Empty ones would get no area.
    let children: Vec<NodeId> = tree[id]
        .children
        .iter()
        .copied()
        .filter(|&c| tree[c].size > 0)
        .collect();
    let sizes: Vec<f64> = children.iter().map(|&c| tree[c].size as f64).collect();

    for (child, r) in children.into_iter().zip(squarify(&sizes, rect)) {
        if r.w < options.min_side || r.h < options.min_side {
            continue;
        }
        tiles.push(Tile {
            id: child,
            rect: r,
            depth,
        });
        if tree[child].children.is_empty() {
            continue;
        }
        // Only folders big enough to read get a name across the top. The
        // rest keep their space for what's inside them.
        let top = if r.w >= LABEL_WIDTH && r.h >= LABEL_HEIGHT {
            options.header
        } else {
            options.padding
        };
        let inner = Rect::new(
            r.x + options.padding,
            r.y + top,
            r.w - 2.0 * options.padding,
            r.h - top - options.padding,
        );
        if inner.w >= options.min_side && inner.h >= options.min_side {
            layout_children(tree, child, inner, depth + 1, options, tiles);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_example_from_the_paper() {
        // Bruls, Huizing and van Wijk lay out these seven sizes in a 6 by 4
        // rectangle. The two sixes end up side by side as 3 by 2 boxes.
        let rects = squarify(
            &[6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0],
            Rect::new(0.0, 0.0, 6.0, 4.0),
        );
        assert_eq!(rects[0], Rect::new(0.0, 0.0, 3.0, 2.0));
        assert_eq!(rects[1], Rect::new(0.0, 2.0, 3.0, 2.0));
    }

    #[test]
    fn areas_match_sizes_and_fill_the_space() {
        let sizes = [50.0, 30.0, 10.0, 5.0, 3.0, 2.0];
        let rect = Rect::new(10.0, 20.0, 400.0, 300.0);
        let rects = squarify(&sizes, rect);
        let total: f64 = rects.iter().map(Rect::area).sum();
        assert!(close(total, rect.area()));
        for (size, r) in sizes.iter().zip(&rects) {
            assert!(close(r.area(), size / 100.0 * rect.area()));
            assert!(r.x >= rect.x - 1e-9 && r.x + r.w <= rect.x + rect.w + 1e-9);
            assert!(r.y >= rect.y - 1e-9 && r.y + r.h <= rect.y + rect.h + 1e-9);
        }
    }

    #[test]
    fn nothing_to_lay_out() {
        assert!(squarify(&[], Rect::new(0.0, 0.0, 10.0, 10.0)).is_empty());
    }

    #[test]
    fn folders_contain_their_children() {
        let tree = crate::tree::tests::sample();
        let tiles = layout(&tree, tree.root(), 800.0, 600.0, &Layout::default());
        let photos = tiles
            .iter()
            .find(|t| tree[t.id].display_name() == "photos")
            .unwrap();
        let a = tiles
            .iter()
            .find(|t| tree[t.id].display_name() == "a.jpg")
            .unwrap();
        assert_eq!(a.depth, 2);
        assert!(a.rect.x >= photos.rect.x && a.rect.y >= photos.rect.y);
        assert!(a.rect.x + a.rect.w <= photos.rect.x + photos.rect.w);
    }
}
