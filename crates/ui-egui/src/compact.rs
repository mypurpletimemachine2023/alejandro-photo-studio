//! A runtime projection of the editor. It never replaces or persists the desktop dock.
use crate::{
    PhotocraftApp, Tool, UiState,
    compact_policy::{self, Preference},
};
use egui::{Context, Id, TouchId, TouchPhase, Ui, vec2};
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drawer {
    New,
    Tools,
    Layers,
    Properties,
    Options,
    More,
}

#[derive(Default)]
pub(crate) struct Runtime {
    pub active: bool,
    pub navigate: bool,
    preference: Preference,
    initialized: bool,
    changed: bool,
    drawer: Option<Drawer>,
    contacts: Vec<TouchId>,
    blocked: bool,
    checkpoint: Option<Checkpoint>,
    prepared_frame: Option<u64>,
    viewport: [f32; 4],
    controls: Vec<(&'static str, [f32; 4])>,
    new_size: [u32; 2],
}

struct Checkpoint {
    ui: UiState,
    state: Option<photocraft_engine::DocState>,
}

impl PhotocraftApp {
    pub fn compact_layout_needs_init(&self) -> bool {
        !self.compact.initialized
    }
    pub fn initialize_compact_layout(&mut self, stored: Option<&str>) {
        if !self.compact.initialized {
            self.compact.preference = Preference::parse(stored.unwrap_or("auto"));
            self.compact.initialized = true;
        }
    }
    pub fn take_compact_layout_change(&mut self) -> Option<&'static str> {
        std::mem::take(&mut self.compact.changed).then(|| self.compact.preference.id())
    }
    /// Read-only host evidence. No document data or editing commands are exposed.
    pub fn compact_layout_state(&self) -> String {
        json!({"version":1,"applied":if self.compact.active {"compact"} else {"desktop"},
            "preferred":self.compact.preference.id(),"pane":self.compact.drawer.map_or("canvas", |p| match p {
                Drawer::New=>"new",Drawer::Tools=>"tools",Drawer::Layers=>"layers",Drawer::Properties=>"properties",Drawer::Options=>"options",Drawer::More=>"more"}),
            "interaction":if self.compact.navigate {"navigate"} else {"edit"},"viewport":self.compact.viewport,
            "controls":self.compact.controls.iter().map(|(id,rect)|json!({"id":id,"rect":rect})).collect::<Vec<_>>()}).to_string()
    }
}

pub(crate) fn prepare(app: &mut PhotocraftApp, ctx: &Context) {
    let frame = ctx.cumulative_frame_nr();
    if app.compact.prepared_frame == Some(frame) {
        return;
    }
    app.compact.prepared_frame = Some(frame);
    let r = ctx.content_rect();
    app.compact.viewport = [r.left(), r.top(), r.width(), r.height()];
    let held = ctx.input(|i| i.pointer.any_down()) || !app.compact.contacts.is_empty() || app.drag.is_some();
    app.compact.active = compact_policy::resolve(app.compact.preference, app.compact.active, r.width(), r.height(), held);
    app.compact.controls.clear();
    if !app.compact.active {
        return;
    }
    // Ownership is decided before any canvas preview/commit sees this frame's pointer events.
    let events = ctx.input(|i| i.events.clone());
    let mut cancel = false;
    let was_blocked = app.compact.blocked;
    let had_contacts = !app.compact.contacts.is_empty();
    if was_blocked && !had_contacts {
        app.compact.blocked = false;
    }
    for event in events {
        match event {
            egui::Event::Touch { id, phase, pos, .. } => match phase {
                TouchPhase::Start => {
                    if app.compact.contacts.is_empty() && app.last_canvas_rect.contains(pos) && app.ui.dialogs.is_empty() {
                        app.compact.checkpoint = Some(Checkpoint { ui: app.ui.clone(), state: app.session.active().cloned() });
                    }
                    if app.compact.contacts.len() < 10 && !app.compact.contacts.contains(&id) {
                        app.compact.contacts.push(id);
                    }
                    if app.compact.contacts.len() > 1 {
                        cancel = true;
                        app.compact.blocked = true;
                    }
                }
                TouchPhase::Cancel => {
                    app.compact.contacts.retain(|t| *t != id);
                    cancel = true;
                    app.compact.blocked = true;
                }
                TouchPhase::End => {
                    app.compact.contacts.retain(|t| *t != id);
                }
                TouchPhase::Move => {}
            },
            egui::Event::WindowFocused(false) if !app.compact.contacts.is_empty() => {
                app.compact.contacts.clear();
                cancel = true;
                app.compact.blocked = true;
            }
            _ => {}
        }
    }
    if cancel {
        cancel_preview(app);
    }
    if app.compact.contacts.is_empty() && !cancel {
        // Keep a cancelled gesture's release inert too; clear only on the following frame.
        if was_blocked && !had_contacts {
            app.compact.blocked = false;
        }
        app.compact.checkpoint = None;
    }
}

fn cancel_preview(app: &mut PhotocraftApp) {
    if let Some(saved) = app.compact.checkpoint.take() {
        // Never replace a document changed by a background job or another owner.
        if let (Some(before), Some(now)) = (saved.state, app.session.active_mut())
            && before.doc.id == now.doc.id
            && before.revision == now.revision
            && before.saved_revision == now.saved_revision
        {
            *now = before;
            app.ui = saved.ui;
        }
    }
    app.drag = None;
    app.live_stroke = None;
    app.press_stroke = false;
    app.move_preview = None;
    app.patch_preview = None;
    app.trail = None;
    app.cancelled_vector_drag = true;
}

pub(crate) fn navigating(app: &PhotocraftApp) -> bool {
    app.compact.active && app.compact.navigate
}
pub(crate) fn blocked(app: &PhotocraftApp) -> bool {
    app.compact.active && app.compact.blocked
}

fn command(app: &mut PhotocraftApp, ctx: &Context, id: &str) {
    if let Err(e) = crate::menus::invoke(app, ctx, id, json!({})) {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}
fn button(app: &mut PhotocraftApp, ui: &mut Ui, id: &'static str, label: &str, selected: bool) -> bool {
    let r = ui.add(egui::Button::new(label).selected(selected).min_size(vec2(44.0, 44.0)));
    app.compact.controls.push((id, [r.rect.left(), r.rect.top(), r.rect.width(), r.rect.height()]));
    r.clicked()
}
fn touch_style(ui: &mut Ui) {
    ui.spacing_mut().interact_size = vec2(44.0, 44.0);
    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
}

pub(crate) fn show(app: &mut PhotocraftApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = crate::theme::Tokens::get(&ctx);
    egui::Panel::top("aca-photo-compact-bar").frame(egui::Frame::NONE.fill(t.chrome).inner_margin(4)).show(ui, |ui| {
        touch_style(ui);
        ui.horizontal(|ui| {
            // MenuButton ignores interact_size.x; Save/Open still need a real 44pt target.
            let file = ui.add(egui::Button::new("File").min_size(vec2(44.0, 44.0)));
            app.compact.controls.push(("file", [file.rect.left(), file.rect.top(), file.rect.width(), file.rect.height()]));
            egui::Popup::menu(&file).show(|ui| {
                touch_style(ui);
                if ui.button("New image…").clicked() {
                    app.compact.new_size = [512, 512];
                    app.compact.drawer = Some(Drawer::New);
                    ui.close();
                }
                for (label, id) in [
                    ("Open…", "file.open"),
                    ("Save project", "file.save"),
                    ("Save as…", "file.saveAs"),
                    ("Export PNG", "file.export.quickExportAsPng"),
                    ("Export options…", "file.export.exportAs"),
                ] {
                    if ui.button(label).clicked() {
                        command(app, &ctx, id);
                        ui.close();
                    }
                }
            });
            if button(app, ui, "tools", "Tools", false) {
                app.compact.drawer = Some(Drawer::Tools);
            }
            if button(app, ui, "undo", "Undo", false) {
                command(app, &ctx, "edit.undo");
            }
            if button(app, ui, "redo", "Redo", false) {
                command(app, &ctx, "edit.redo");
            }
            if button(app, ui, "more", "More", false) {
                app.compact.drawer = Some(Drawer::More);
            }
        });
        ui.horizontal(|ui| {
            if button(app, ui, "edit", "Edit", !app.compact.navigate) {
                app.compact.navigate = false;
            }
            if button(app, ui, "navigate", "Navigate", app.compact.navigate) {
                app.compact.navigate = true;
            }
            if button(app, ui, "layers", "Layers", app.compact.drawer == Some(Drawer::Layers)) {
                app.compact.drawer = Some(Drawer::Layers);
            }
            if button(app, ui, "properties", "Properties", app.compact.drawer == Some(Drawer::Properties)) {
                app.compact.drawer = Some(Drawer::Properties);
            }
        });
    });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| crate::canvas::document_area(app, ui));
    show_drawer(app, &ctx);
}

fn show_drawer(app: &mut PhotocraftApp, ctx: &Context) {
    let Some(pane) = app.compact.drawer else { return };
    let r = ctx.content_rect();
    let modal = egui::Modal::new(Id::new("aca-photo-compact-drawer")).show(ctx, |ui| {
        touch_style(ui);
        ui.set_width((r.width() - 32.0).clamp(200.0, 440.0));
        ui.horizontal(|ui| {
            if ui.button("Back to canvas").clicked() {
                app.compact.drawer = None;
            }
            ui.label(match pane {
                Drawer::New => "New image",
                Drawer::Tools => "Tools",
                Drawer::Layers => "Layers",
                Drawer::Properties => "Properties",
                Drawer::Options => "Tool options",
                Drawer::More => "More",
            });
        });
        egui::ScrollArea::vertical().max_height((r.height() - 100.0).max(80.0)).auto_shrink([false, false]).show(ui, |ui| match pane {
            Drawer::New => {
                for (label, value) in ["Width (px)", "Height (px)"].into_iter().zip(app.compact.new_size.iter_mut()) {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add_sized([110.0, 44.0], egui::DragValue::new(value).range(1..=4096));
                    });
                }
                if ui.button("Create image").clicked() {
                    let [width, height] = app.compact.new_size;
                    if let Err(e) = app.run("file.new", json!({"width":width.clamp(1,4096),"height":height.clamp(1,4096)})) {
                        app.ui.status = e;
                    } else {
                        app.compact.drawer = None;
                    }
                }
            }
            Drawer::Tools => {
                for tool in Tool::ALL {
                    if ui.add(egui::Button::new(tool.label()).selected(tool == app.ui.tool).min_size(vec2(ui.available_width(), 44.0))).clicked() {
                        crate::tool_memory::select(app, tool);
                        app.compact.navigate = false;
                        app.compact.drawer = None;
                    }
                }
            }
            Drawer::Layers => {
                if ui.button("Add paint layer").clicked()
                    && let Err(e) = app.run("layer.new.layer", json!({}))
                {
                    app.ui.status = e;
                }
                crate::panels::dock_body(app, ui, crate::dock::Group::Layers, 0);
            }
            Drawer::Properties => crate::panels::dock_body(app, ui, crate::dock::Group::Properties, 0),
            Drawer::Options => {
                if app.ui.tool.is_brushlike() {
                    let before = app.session.tools.brush.clone();
                    let mut after = before.clone();
                    ui.add(egui::Slider::new(&mut after.size, 1.0..=400.0).text("Brush size"));
                    ui.add(egui::Slider::new(&mut after.opacity, 0.0..=1.0).text("Opacity"));
                    crate::brush_panel::commit_gesture(app, ctx, &before, &after);
                }
                if ui.button("Foreground colour…").clicked() {
                    crate::color_picker_ui::open(app, "foreground");
                    app.compact.drawer = None;
                }
                if app.ui.tool == Tool::Crop {
                    if ui.button("Apply crop").clicked() {
                        crate::canvas::commit_crop(app);
                        app.compact.drawer = None;
                    }
                    if ui.button("Cancel crop").clicked() {
                        crate::crop_ui::cancel(app);
                        app.compact.drawer = None;
                    }
                }
            }
            Drawer::More => {
                ui.label("Layout (saved separately from artwork)");
                for p in Preference::ALL {
                    if ui.selectable_label(app.compact.preference == p, p.label()).clicked() {
                        app.compact.preference = p;
                        app.compact.changed = true;
                        app.compact.drawer = None;
                    }
                }
                if ui.button("Tool options / colour").clicked() {
                    app.compact.drawer = Some(Drawer::Options);
                }
                if ui.button("Fit image").clicked() {
                    command(app, ctx, "view.fitOnScreen");
                    app.compact.drawer = None;
                }
                if ui.button("App tutorial").clicked() {
                    command(app, ctx, "help.discord");
                    app.compact.drawer = None;
                }
            }
        });
    });
    if modal.should_close() {
        app.compact.drawer = None;
    }
}

pub(crate) fn desktop_controls(app: &mut PhotocraftApp, ctx: &Context) {
    let r = ctx.content_rect();
    egui::Area::new(Id::new("aca-photo-layout-control")).fixed_pos(egui::pos2((r.right() - 92.0).max(r.left()), (r.bottom() - 56.0).max(r.top()))).show(
        ctx,
        |ui| {
            if button(app, ui, "layout", "Layout", false) {
                app.compact.drawer = Some(Drawer::More);
            }
        },
    );
    show_drawer(app, ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aca_mobile_choice_and_drawers_never_serialize_into_desktop_or_document() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let before = serde_json::to_value(&app.ui).unwrap();
        app.initialize_compact_layout(Some("compact"));
        app.compact.drawer = Some(Drawer::Layers);
        app.compact.navigate = true;
        assert_eq!(serde_json::to_value(&app.ui).unwrap(), before);
        assert!(app.session.documents().is_empty());
        assert_eq!(app.take_compact_layout_change(), None);
    }
    #[test]
    fn aca_mobile_cancelled_brush_preview_is_dropped_without_an_undo_step() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width":32,"height":32})).unwrap();
        let state = app.session.active().unwrap().clone();
        app.compact.checkpoint = Some(Checkpoint { ui: app.ui.clone(), state: Some(state.clone()) });
        app.drag = Some(crate::canvas::Drag::new(Tool::Brush, [2.0, 2.0], vec![[2.0, 2.0, 1.0]], egui::Modifiers::NONE, false));
        cancel_preview(&mut app);
        assert!(app.drag.is_none());
        assert!(app.live_stroke.is_none());
        assert_eq!(app.session.active().unwrap().revision, state.revision);
        assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &state.doc));
    }
    #[test]
    fn aca_mobile_actual_toolbar_stays_44pt_and_canvas_survives_portrait_landscape() {
        for (w, h) in [(390.0, 747.0), (844.0, 293.0)] {
            let ctx = Context::default();
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
            PhotocraftApp::setup_context(&ctx, app.ui.theme);
            app.run("file.new", json!({"width":32,"height":32})).unwrap();
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))), ..Default::default() };
            let mut output = ctx.run_ui(raw, |ui| {
                prepare(&mut app, &ctx);
                show(&mut app, ui);
            });
            output.textures_delta.clear();
            assert!(app.compact.active);
            assert!(app.compact.controls.iter().any(|(id, _)| *id == "file"), "Save/Open File target must be measured");
            for (id, [x, y, width, height]) in &app.compact.controls {
                assert!(*width >= 44.0 && *height >= 44.0, "{id}: {width}x{height}");
                assert!(*x >= 0.0 && *y >= 0.0 && x + width <= w + 1.0 && y + height <= h + 1.0, "{id} outside {w}x{h}");
            }
            assert!(app.last_canvas_rect.width() > 300.0 && app.last_canvas_rect.height() > 120.0, "{:?}", app.last_canvas_rect);
        }
    }
    #[test]
    fn aca_mobile_actual_second_finger_and_release_cancel_then_one_finger_paints() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width":32,"height":32})).unwrap();
        let ctx = Context::default();
        PhotocraftApp::setup_context(&ctx, app.ui.theme);
        crate::tool_memory::select(&mut app, Tool::Brush);
        let touch = |id, phase, pos| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: TouchId(id), phase, pos, force: None };
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let frame = |app: &mut PhotocraftApp, events| {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(390.0, 747.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                prepare(app, &ctx);
                show(app, ui);
            });
            out.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        let at = app.last_canvas_rect.center();
        let revision = app.session.active().unwrap().revision;
        frame(&mut app, vec![touch(1, TouchPhase::Start, at), egui::Event::PointerMoved(at), button(at, true)]);
        let moved = at + vec2(10.0, 6.0);
        frame(&mut app, vec![touch(1, TouchPhase::Move, moved), egui::Event::PointerMoved(moved)]);
        assert!(app.drag.is_some(), "the actual brush preview started");
        frame(&mut app, vec![touch(2, TouchPhase::Start, moved + vec2(40.0, 0.0))]);
        assert!(blocked(&app));
        assert!(app.drag.is_none());
        frame(
            &mut app,
            vec![touch(1, TouchPhase::End, moved), touch(2, TouchPhase::End, moved + vec2(40.0, 0.0)), button(moved, false), egui::Event::PointerGone],
        );
        assert!(blocked(&app), "the release must not become a tool click");
        assert_eq!(app.session.active().unwrap().revision, revision);
        frame(&mut app, vec![touch(3, TouchPhase::Start, at), egui::Event::PointerMoved(at), button(at, true)]);
        assert!(!blocked(&app), "a fresh single-finger press is editable");
        frame(&mut app, vec![touch(3, TouchPhase::Move, moved), egui::Event::PointerMoved(moved)]);
        frame(&mut app, vec![touch(3, TouchPhase::End, moved), button(moved, false), egui::Event::PointerGone]);
        assert!(app.session.active().unwrap().revision > revision, "the next real stroke commits");
    }
    #[test]
    fn aca_mobile_cancellation_cannot_restore_over_another_revision() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width":32,"height":32})).unwrap();
        app.compact.checkpoint = Some(Checkpoint { ui: app.ui.clone(), state: app.session.active().cloned() });
        app.run("layer.new.layer", json!({})).unwrap();
        let current = app.session.active().unwrap().clone();
        cancel_preview(&mut app);
        assert_eq!(app.session.active().unwrap().revision, current.revision);
        assert!(std::sync::Arc::ptr_eq(&app.session.active().unwrap().doc, &current.doc));
    }
}
