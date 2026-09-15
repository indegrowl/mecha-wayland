//! The atlas as a caller and a backend see it, end to end.

use atlas::prelude::*;
use geometry::Rect;

fn solid(width: u32, height: u32, format: Format, value: u8) -> Bitmap {
    Bitmap {
        width,
        height,
        format,
        pixels: vec![value; (width * height) as usize * format.bytes()],
    }
}

fn icon(side: u32) -> Bitmap {
    solid(side, side, Format::R8, 255)
}

fn image(w: u32, h: u32) -> Bitmap {
    solid(w, h, Format::Rgba8, 128)
}

#[test]
fn ids_are_dense_across_classes_and_class_answers_each() {
    let mut atlas = Atlas::new();
    let a = atlas.insert(Class::Icon, &icon(32)).unwrap();
    let b = atlas.insert(Class::Image, &image(32, 16)).unwrap();
    assert_eq!(
        (a, b),
        (SpriteId(0), SpriteId(1)),
        "sprites count icons and images together"
    );
    let ta = atlas.sprite(a).tile;
    let tb = atlas.sprite(b).tile;
    assert_eq!(ta.atlas, AtlasId(0), "the first page made");
    assert_eq!(tb.atlas, AtlasId(1), "the second, of another class");
    assert_eq!(atlas.class(AtlasId(0)), Class::Icon);
    assert_eq!(atlas.class(AtlasId(1)), Class::Image);
    assert_eq!(atlas.pages().count(), 2);
    assert_eq!(
        atlas.pages().map(|p| p.id()).collect::<Vec<_>>(),
        vec![AtlasId(0), AtlasId(1)]
    );
}

#[test]
fn a_sprite_records_its_master_size_and_padded_bounds() {
    let mut atlas = Atlas::new();
    let a = atlas.insert(Class::Image, &image(100, 50)).unwrap();
    let s = atlas.sprite(a);
    assert_eq!(s.size, geometry::Size::new(100.0, 50.0));
    assert_eq!(s.tile.bounds, Rect::new(16.0, 16.0, 100.0, 50.0));
}

#[test]
fn insert_rejects_the_wrong_class_or_format_and_too_large() {
    let mut atlas = Atlas::new();
    assert!(
        matches!(atlas.insert(Class::Icon, &image(8, 8)), Err(Error::Class)),
        "Rgba8 into Icon"
    );
    assert!(
        matches!(atlas.insert(Class::Image, &icon(8)), Err(Error::Class)),
        "R8 into Image"
    );
    assert!(matches!(
        atlas.insert(Class::Glyph, &icon(8)),
        Err(Error::Class)
    ));
    assert!(matches!(
        atlas.insert(Class::External, &image(8, 8)),
        Err(Error::Class)
    ));
    match atlas.insert(Class::Image, &image(1000, 1000)) {
        Err(Error::TooLarge {
            width: 1000,
            height: 1000,
            max: 992,
        }) => {}
        other => panic!("expected TooLarge, got {other:?}"),
    }
    assert!(
        atlas
            .insert(Class::Image, &image(1000, 1000).fit(992))
            .is_ok()
    );
    assert_eq!(
        atlas.pages().count(),
        1,
        "the rejected inserts made no page"
    );
}

#[test]
fn inserts_share_a_page_until_it_is_full_and_earlier_tiles_stay() {
    let mut atlas = Atlas::new();
    let a = atlas.insert(Class::Icon, &icon(64)).unwrap();
    let b = atlas.insert(Class::Icon, &icon(64)).unwrap();
    let (ta, tb) = (atlas.sprite(a).tile, atlas.sprite(b).tile);
    assert_eq!(ta.atlas, tb.atlas);
    assert_ne!(ta.bounds, tb.bounds);
    // A 64 icon takes a 96 slot: 10 per shelf, 10 shelves, 100 per page.
    let mut last = b;
    for _ in 0..98 {
        last = atlas.insert(Class::Icon, &icon(64)).unwrap();
    }
    assert_eq!(
        atlas.sprite(last).tile.atlas,
        AtlasId(0),
        "the hundredth fits page 0"
    );
    let spill = atlas.insert(Class::Icon, &icon(64)).unwrap();
    assert_eq!(
        atlas.sprite(spill).tile.atlas,
        AtlasId(1),
        "the next opens a page"
    );
    assert_eq!(atlas.sprite(a).tile, ta, "nothing moved");
    assert_eq!(atlas.pages().count(), 2);
}

#[test]
fn drain_dirty_yields_a_new_page_whole_then_only_what_changed() {
    let mut atlas = Atlas::new();
    atlas.insert(Class::Image, &image(8, 8)).unwrap();
    let mut seen: Vec<(AtlasId, Cell)> = Vec::new();
    atlas.drain_dirty(|p, c| seen.push((p.id(), c)));
    assert_eq!(seen.len(), 256, "a new page is all cells");
    assert!(seen.iter().all(|(id, _)| *id == AtlasId(0)));
    seen.clear();
    atlas.drain_dirty(|p, c| seen.push((p.id(), c)));
    assert!(seen.is_empty(), "a second drain has nothing");

    // A second image lands at (16 + 48, 16) on the same shelf... whatever the
    // exact spot, its cells are the only ones dirty now.
    let b = atlas.insert(Class::Image, &image(8, 8)).unwrap();
    let r = atlas.sprite(b).tile.bounds;
    atlas.drain_dirty(|p, c| seen.push((p.id(), c)));
    assert!(!seen.is_empty());
    for (_, c) in &seen {
        let cx = c.col as f32 * 64.0;
        let cy = c.row as f32 * 64.0;
        assert!(
            cx < r.right() && cx + 64.0 > r.x() && cy < r.bottom() && cy + 64.0 > r.y(),
            "cell {c:?} touches {r:?}"
        );
    }
}

#[test]
fn a_backend_reads_a_dirty_cell_at_every_level() {
    let mut atlas = Atlas::new();
    let a = atlas.insert(Class::Image, &image(64, 64)).unwrap();
    let r = atlas.sprite(a).tile.bounds; // (16,16) 64x64
    let mut cells = Vec::new();
    atlas.drain_dirty(|p, c| cells.push((p.id(), c)));
    assert_eq!(cells.len(), 256);
    let page = atlas.pages().next().unwrap();
    let mut out = Vec::new();
    for k in 0..page.levels() {
        page.cell(k, Cell { col: 0, row: 0 }, &mut out);
        let side = (64 >> k) as usize;
        assert_eq!(out.len(), side * side * 4, "level {k}");
        // The image's centre in this cell: (48, 48) at level 0, halving per level.
        let cx = (r.x() as usize + 32) >> k;
        let cy = (r.y() as usize + 32) >> k;
        let i = (cy * side + cx) * 4;
        assert_eq!(
            &out[i..i + 4],
            &[128, 128, 128, 128],
            "level {k} holds the image's grey"
        );
    }
}

const INTER: &[u8] = include_bytes!("fixtures/Inter-Regular.ttf");

#[test]
fn a_glyph_miss_dirties_only_the_cells_its_bitmap_crossed() {
    let mut atlas = Atlas::new();
    let inter = atlas.add_font(INTER).unwrap();
    let (_, a) = atlas.lookup(&[inter], 'a').unwrap();
    let g = atlas.glyph(inter, a, 14);
    let mut cells = Vec::new();
    atlas.drain_dirty(|_, c| cells.push(c));
    assert_eq!(cells.len(), 256, "the glyph page is new");
    cells.clear();
    let (_, b) = atlas.lookup(&[inter], 'b').unwrap();
    let gb = atlas.glyph(inter, b, 14);
    atlas.drain_dirty(|_, c| cells.push(c));
    assert!(
        !cells.is_empty() && cells.len() <= 2,
        "one or two cells: {cells:?}"
    );
    assert_eq!(gb.tile.atlas, g.tile.atlas);
    assert_eq!(atlas.class(g.tile.atlas), Class::Glyph);
    assert_eq!(
        atlas.pages().next().unwrap().levels(),
        1,
        "glyph pages have no mips"
    );
}
