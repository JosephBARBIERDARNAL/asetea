//! Headless end-to-end tests that drive the tools and commands like a user would.

use crate::app::*;
use crate::ase;
use crate::doc::*;
use eframe::egui;

const RED: Rgba = [255, 0, 0, 255];
const NONE: Rgba = [0; 4];

fn app(w: i32, h: i32) -> (App, egui::Context) {
    let mut a = App::base();
    a.new_sprite(w, h, None);
    a.fg = RED;
    (a, egui::Context::default())
}

fn stroke(a: &mut App, tool: Tool, pts: &[(i32, i32)], mods: egui::Modifiers) {
    a.tool = tool;
    let r = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 500.0));
    a.press(pts[0], false, mods, tool, egui::Pos2::ZERO, r);
    for p in &pts[1..] {
        a.drag_to(*p, mods, egui::Vec2::ZERO);
    }
    a.release(*pts.last().unwrap());
}

fn px(a: &App, x: i32, y: i32) -> Rgba {
    let d = &a.docs[a.active];
    d.sprite.composite(d.frame).get(x, y)
}

fn count(a: &App) -> usize {
    let d = &a.docs[a.active];
    d.sprite
        .composite(d.frame)
        .data
        .chunks_exact(4)
        .filter(|p| p[3] != 0)
        .count()
}

#[test]
fn pencil_undo_redo_and_save() {
    let (mut a, ctx) = app(16, 16);
    let m = egui::Modifiers::NONE;
    stroke(&mut a, Tool::Pencil, &[(2, 2), (6, 2)], m);
    assert_eq!(count(&a), 5);
    assert_eq!(px(&a, 4, 2), RED);
    assert!(a.docs[0].modified());
    // Shift+click continues with a straight line from the last point.
    stroke(&mut a, Tool::Pencil, &[(6, 6)], egui::Modifiers::SHIFT);
    assert_eq!(count(&a), 9);
    a.run(Cmd::Undo, &ctx);
    assert_eq!(count(&a), 5);
    a.run(Cmd::Undo, &ctx);
    assert_eq!(count(&a), 0);
    assert!(!a.docs[0].modified());
    a.run(Cmd::Redo, &ctx);
    assert_eq!(count(&a), 5);
    stroke(&mut a, Tool::Eraser, &[(2, 2), (3, 2)], m);
    assert_eq!(count(&a), 3);
    let (b, _) = ase::load(&ase::save(&a.docs[0].sprite)).unwrap();
    assert!(b.composite(0) == a.docs[0].sprite.composite(0));
}

#[test]
fn shapes_fill_and_symmetry() {
    let (mut a, ctx) = app(16, 16);
    let m = egui::Modifiers::NONE;
    stroke(&mut a, Tool::Rect, &[(2, 2), (5, 9), (9, 9)], m);
    assert_eq!(
        count(&a),
        28,
        "outline of an 8x8 rectangle, preview strokes must not accumulate"
    );
    a.fg = [0, 0, 255, 255];
    stroke(&mut a, Tool::Bucket, &[(5, 5)], m);
    assert_eq!(count(&a), 64);
    assert_eq!(px(&a, 5, 5), [0, 0, 255, 255]);
    assert_eq!(px(&a, 2, 2), RED);
    assert_eq!(px(&a, 0, 0), NONE);
    a.filled = true;
    a.run(Cmd::NewLayer, &ctx);
    stroke(&mut a, Tool::Ellipse, &[(10, 10), (15, 15)], m);
    assert_eq!(px(&a, 12, 12), [0, 0, 255, 255]);
    assert_eq!(px(&a, 10, 10), NONE);
    a.sym_h = true;
    a.sym_v = true;
    a.run(Cmd::NewLayer, &ctx);
    a.fg = [0, 255, 0, 255];
    stroke(&mut a, Tool::Pencil, &[(0, 0)], m);
    for p in [(0, 0), (15, 0), (0, 15), (15, 15)] {
        assert_eq!(px(&a, p.0, p.1), [0, 255, 0, 255]);
    }
    assert_eq!(a.docs[0].sprite.layers.len(), 3);
    a.run(Cmd::Flatten, &ctx);
    assert_eq!(a.docs[0].sprite.layers.len(), 1);
    assert_eq!(px(&a, 15, 15), [0, 255, 0, 255]);
    assert_eq!(px(&a, 5, 5), [0, 0, 255, 255]);
}

#[test]
fn pixel_perfect_removes_corners() {
    let (mut a, _) = app(16, 16);
    a.pixel_perfect = true;
    stroke(
        &mut a,
        Tool::Pencil,
        &[(1, 1), (2, 1), (2, 2), (3, 2), (3, 3)],
        egui::Modifiers::NONE,
    );
    assert_eq!(px(&a, 2, 1), NONE);
    assert_eq!(px(&a, 3, 2), NONE);
    assert_eq!(count(&a), 3);
}

#[test]
fn selection_move_copy_paste() {
    let (mut a, ctx) = app(16, 16);
    let m = egui::Modifiers::NONE;
    stroke(&mut a, Tool::Pencil, &[(1, 1), (3, 1)], m);
    stroke(&mut a, Tool::Marquee, &[(0, 0), (4, 2)], m);
    assert_eq!(a.docs[0].sel.as_ref().unwrap().bounds(), Some((0, 0, 5, 3)));
    // Dragging inside the selection moves the pixels.
    stroke(&mut a, Tool::Marquee, &[(1, 1), (6, 8)], m);
    assert!(a.docs[0].floating.is_some());
    assert_eq!(a.docs[0].sel.as_ref().unwrap().bounds(), Some((5, 7, 5, 3)));
    a.run(Cmd::Deselect, &ctx);
    assert!(a.docs[0].floating.is_none());
    assert_eq!(px(&a, 6, 8), RED);
    assert_eq!(px(&a, 1, 1), NONE);
    assert_eq!(count(&a), 3);
    // One undo reverts the whole move.
    a.run(Cmd::Undo, &ctx);
    assert_eq!(px(&a, 1, 1), RED);
    assert_eq!(count(&a), 3);
    // Copy, paste, place.
    a.run(Cmd::SelectAll, &ctx);
    a.run(Cmd::Copy, &ctx);
    a.run(Cmd::NewFrame, &ctx);
    a.run(Cmd::Clear, &ctx);
    assert_eq!(count(&a), 0);
    a.run(Cmd::Paste, &ctx);
    assert_eq!(a.tool, Tool::Move);
    stroke(&mut a, Tool::Move, &[(2, 2), (2, 4)], m);
    a.run(Cmd::Cancel, &ctx);
    assert_eq!(px(&a, 1, 3), RED);
    assert_eq!(count(&a), 3);
    // Frame 1 is untouched and independent of frame 2.
    a.run(Cmd::PrevFrame, &ctx);
    assert_eq!(px(&a, 1, 1), RED);
    // Wand + delete.
    stroke(&mut a, Tool::Wand, &[(1, 1)], m);
    assert_eq!(a.docs[0].sel.as_ref().unwrap().bounds(), Some((1, 1, 3, 1)));
    a.run(Cmd::Clear, &ctx);
    assert_eq!(count(&a), 0);
}

#[test]
fn locked_and_hidden_layers_refuse_edits() {
    let (mut a, _) = app(8, 8);
    a.docs[0].sprite.layers[0].editable = false;
    stroke(&mut a, Tool::Pencil, &[(1, 1)], egui::Modifiers::NONE);
    assert_eq!(count(&a), 0);
    assert!(a.toast.is_some());
    assert!(!a.docs[0].modified());
}

#[test]
fn sprite_transforms_and_frames() {
    let (mut a, ctx) = app(8, 4);
    stroke(&mut a, Tool::Pencil, &[(0, 0)], egui::Modifiers::NONE);
    a.run(Cmd::Rot90Cw, &ctx);
    assert_eq!((a.docs[0].sprite.w, a.docs[0].sprite.h), (4, 8));
    assert_eq!(px(&a, 3, 0), RED);
    a.run(Cmd::FlipH, &ctx);
    assert_eq!(px(&a, 0, 0), RED);
    a.run(Cmd::Trim, &ctx);
    assert_eq!((a.docs[0].sprite.w, a.docs[0].sprite.h), (1, 1));
    a.run(Cmd::Undo, &ctx);
    assert_eq!((a.docs[0].sprite.w, a.docs[0].sprite.h), (4, 8));
    a.run(Cmd::NewFrame, &ctx);
    a.run(Cmd::NewEmptyFrame, &ctx);
    assert_eq!(a.docs[0].sprite.nframes(), 3);
    assert_eq!(a.docs[0].frame, 2);
    assert_eq!(count(&a), 0);
    a.run(Cmd::DelFrame, &ctx);
    assert_eq!(a.docs[0].sprite.nframes(), 2);
    a.run(Cmd::ModeIndexed, &ctx);
    let (b, _) = ase::load(&ase::save(&a.docs[0].sprite)).unwrap();
    assert!(b.mode == ColorMode::Indexed);
    assert!(b.composite(0) == a.docs[0].sprite.composite(0));
    // Playback order helpers.
    let mut sign = 0;
    assert_eq!(advance(3, (1, 3, 0), &mut sign), 1);
    assert_eq!(advance(1, (1, 3, 1), &mut sign), 3);
    let mut f = 1;
    let mut seen = vec![];
    for _ in 0..5 {
        f = advance(f, (1, 3, 2), &mut sign);
        seen.push(f);
    }
    assert_eq!(seen, vec![2, 3, 2, 1, 2]);
}

#[test]
fn groups_and_layer_moves_on_sample() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("The_Dude.aseprite");
    let mut a = App::base();
    let ctx = egui::Context::default();
    a.open_path(&path);
    assert_eq!(a.docs.len(), 1, "{:?}", a.dialog.is_some());
    let before = a.docs[0].sprite.composite(5);
    let names = |a: &App| {
        a.docs[0]
            .sprite
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()
    };
    let orig = names(&a);
    // Move the "Refs" group (with its children) up past "Rear Arm" and back down.
    a.docs[0].layer = 1;
    a.run(Cmd::LayerUp, &ctx);
    assert_eq!(names(&a)[1], "Rear Arm");
    assert_eq!(a.docs[0].sprite.layers[a.docs[0].layer].name, "Refs");
    a.run(Cmd::LayerDown, &ctx);
    assert_eq!(names(&a), orig);
    assert!(a.docs[0].sprite.composite(5) == before);
    // Every command can run on a real file without panicking.
    let cmds: Vec<Cmd> = a.cmds.iter().map(|c| c.cmd).collect();
    for c in cmds {
        if matches!(
            c,
            Cmd::Open | Cmd::Save | Cmd::SaveAs | Cmd::Quit | Cmd::CloseDoc | Cmd::ImportPalette | Cmd::ExportPalette
        ) {
            continue;
        }
        a.run(c, &ctx);
        a.dialog = None;
        let d = &a.docs[0];
        assert!(d.layer < d.sprite.layers.len() && d.frame < d.sprite.nframes(), "{c:?}");
        for l in &d.sprite.layers {
            assert_eq!(l.cels.len(), d.sprite.nframes(), "{c:?}");
        }
        ase::load(&ase::save(&d.sprite)).unwrap_or_else(|e| panic!("{c:?}: {e}"));
    }
}

#[test]
fn edit_and_save_real_file_to_disk() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("copy.aseprite");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("The_Dude_in_Air.aseprite"),
        &path,
    )
    .unwrap();
    let mut a = App::base();
    a.open_path(&path);
    a.fg = RED;
    let other_frame = a.docs[0].sprite.composite(7);
    stroke(&mut a, Tool::Pencil, &[(3, 3), (9, 3)], egui::Modifiers::NONE);
    assert!(a.docs[0].modified());
    assert!(a.save_doc(0, false));
    assert!(!a.docs[0].modified());
    let (b, warnings) = ase::load(&std::fs::read(&path).unwrap()).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(b.composite(0).get(5, 3), RED);
    assert!(b.composite(7) == other_frame);
    assert_eq!(b.tags.len(), a.docs[0].sprite.tags.len());
    assert_eq!(b.layers.iter().filter(|l| l.group).count(), 1);
    assert!(!dir.join("copy.aseprite.tmp").exists());
}

#[test]
fn exports_and_image_import() {
    use crate::export;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("target/test-tmp/export");
    std::fs::create_dir_all(&dir).unwrap();
    let (s, _) = ase::load(&std::fs::read(root.join("The_Dude_in_Air.aseprite")).unwrap()).unwrap();
    export::save_png(&dir.join("frame.png"), &s.composite(2), 2).unwrap();
    let back = export::import_image(&dir.join("frame.png")).unwrap();
    assert_eq!((back.w, back.h), (256, 256));
    assert!(back.composite(0) == s.composite(2).scale_nearest(256, 256));
    assert_eq!(export::save_png_sequence(&dir.join("seq.png"), &s, 0, 3, 1).unwrap(), 4);
    assert!(dir.join("seq_004.png").exists());
    export::save_gif(&dir.join("anim.gif"), &s, 0, 4, 1).unwrap();
    let gif = export::import_image(&dir.join("anim.gif")).unwrap();
    assert_eq!((gif.nframes(), gif.w, gif.h), (5, 128, 128));
    assert_eq!(gif.durations[0], s.durations[0] / 10 * 10);
    export::save_sheet(&dir.join("sheet.png"), &s, 5, 1, 1, true).unwrap();
    let sheet = export::import_image(&dir.join("sheet.png")).unwrap();
    assert_eq!((sheet.w, sheet.h), (5 * 128 + 6, 4 * 128 + 5));
    assert!(sheet.composite(0).crop(1 + 129, 1, 128, 128) == s.composite(1));
    let json = std::fs::read_to_string(dir.join("sheet.json")).unwrap();
    assert!(json.contains("\"frameTags\"") && json.contains("\"Wall Jump\""));
    assert_eq!(json.matches("\"duration\"").count(), 20);
}
