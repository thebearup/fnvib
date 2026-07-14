use egui::{
    Align2, CentralPanel, Color32, ComboBox, DragValue, FontId, Pos2, Rect,
    Rounding, SidePanel, Stroke, Vec2,
};
use std::collections::HashMap;

use crate::content_lists::ContentData;
use crate::model::layout::Layout;
use crate::model::room::{Cardinal, Doorway, Room, PIECE_SIZE};
use crate::plugin::Plugin;

const CELL_PX: f32 = 30.0;

const C_BG: Color32 = Color32::from_gray(22);
const C_GRID: Color32 = Color32::from_gray(42);
const C_AXIS: Color32 = Color32::from_gray(65);
const C_ROOM: Color32 = Color32::from_rgb(42, 68, 115);
const C_ROOM_COR: Color32 = Color32::from_rgb(30, 80, 68);
const C_ROOM_SEL: Color32 = Color32::from_rgb(65, 115, 200);
const C_ROOM_SEL_COR: Color32 = Color32::from_rgb(45, 125, 105);
const C_ROOM_HOVER: Color32 = Color32::from_rgb(80, 125, 205);
const C_PIECE_LINE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 25);
const C_PREVIEW: Color32 = Color32::from_rgba_premultiplied(65, 95, 180, 80);
const C_PREVIEW_BDR: Color32 = Color32::from_rgb(120, 155, 255);
const C_DOOR: Color32 = Color32::from_rgb(255, 195, 40);
const C_BORDER: Color32 = Color32::WHITE;

#[derive(PartialEq, Clone, Copy)]
enum Tool {
    Select,
    Draw,
    Erase,
}

pub struct FnvibApp {
    layout: Layout,
    /// Tracks whether `layout.id` has been manually edited.
    /// When false, the ID field is auto-synced from the name.
    id_edited: bool,
    placements: HashMap<String, (i32, i32)>,
    content: ContentData,
    kit_names: Vec<String>,
    /// Sorted list names per UI category. An uncategorised list appears under every key.
    lists_by_cat: HashMap<String, Vec<String>>,
    selected_room: Option<usize>,
    tool: Tool,
    drag_start: Option<(i32, i32)>,
    view_offset: Vec2,
    status: String,
    next_room_num: usize,
    load_path: String,
    // add-doorway form state
    new_door_dir: Cardinal,
    new_door_offset: u32,
    new_door_links_to: String,
}

impl FnvibApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        let content = crate::content_lists::load(std::path::Path::new("content_lists.toml"))
            .unwrap_or_default();
        let mut kit_names: Vec<String> = content.kits.keys().cloned().collect();
        kit_names.sort();

        const ALL_CATS: &[&str] = &[
            "furniture", "floor_clutter", "wall_decorations", "lights",
        ];
        let mut lists_by_cat: HashMap<String, Vec<String>> =
            ALL_CATS.iter().map(|&c| (c.to_string(), Vec::new())).collect();
        let mut all_list_names: Vec<String> = content.lists.keys().cloned().collect();
        all_list_names.sort();
        for name in &all_list_names {
            match content.list_categories.get(name) {
                Some(cats) if !cats.is_empty() => {
                    for cat in cats {
                        if let Some(bucket) = lists_by_cat.get_mut(cat.as_str()) {
                            bucket.push(name.clone());
                        }
                    }
                }
                _ => {
                    for &c in ALL_CATS {
                        lists_by_cat.get_mut(c).unwrap().push(name.clone());
                    }
                }
            }
        }

        Self {
            layout: Layout {
                name: "New Cell".into(),
                id: String::new(),
                description: String::new(),
                rooms: Vec::new(),
            },
            id_edited: false,
            placements: HashMap::new(),
            content,
            kit_names,
            lists_by_cat,
            selected_room: None,
            tool: Tool::Select,
            drag_start: None,
            view_offset: Vec2::ZERO,
            status: String::new(),
            next_room_num: 1,
            load_path: String::new(),
            new_door_dir: Cardinal::North,
            new_door_offset: 0,
            new_door_links_to: String::new(),
        }
    }

    fn room_at_grid(&self, gx: i32, gy: i32) -> Option<usize> {
        for (i, room) in self.layout.rooms.iter().enumerate() {
            if let Some(&(rx, ry)) = self.placements.get(&room.id) {
                if gx >= rx && gx < rx + room.width as i32
                    && gy >= ry && gy < ry + room.length as i32
                {
                    return Some(i);
                }
            }
        }
        None
    }

    fn overlaps(&self, gx: i32, gy: i32, w: u32, l: u32, skip: Option<usize>) -> bool {
        for (i, room) in self.layout.rooms.iter().enumerate() {
            if skip == Some(i) { continue; }
            if let Some(&(rx, ry)) = self.placements.get(&room.id) {
                let rw = room.width as i32;
                let rl = room.length as i32;
                if gx < rx + rw && gx + w as i32 > rx && gy < ry + rl && gy + l as i32 > ry {
                    return true;
                }
            }
        }
        false
    }

    fn create_room(&mut self, gx: i32, gy: i32, w: u32, l: u32) {
        if self.overlaps(gx, gy, w, l, None) {
            self.status = "Overlaps existing room".into();
            return;
        }
        let num = self.next_room_num;
        self.next_room_num += 1;
        let id = format!("room{:02}", num);
        let kit = self.kit_names.first().cloned().unwrap_or_else(|| "office".into());
        let shape = if w == 1 || l == 1 { "corridor" } else { "room" };
        self.status = format!("Created {shape} '{id}' ({w}×{l})");
        self.layout.rooms.push(Room {
            id: id.clone(),
            name: format!("Room {num}"),
            kit,
            width: w,
            length: l,
            doorways: Vec::new(),
            furniture: Vec::new(),
            floor_clutter: Vec::new(),
            surface_clutter: Vec::new(),
            wall_decorations: Vec::new(),
            lights: Vec::new(),
            grid_x: Some(gx),
            grid_y: Some(gy),
        });
        self.placements.insert(id, (gx, gy));
        self.selected_room = Some(self.layout.rooms.len() - 1);
    }

    fn delete_room(&mut self, idx: usize) {
        let id = self.layout.rooms[idx].id.clone();
        self.placements.remove(&id);
        self.layout.rooms.remove(idx);
        self.selected_room = match self.selected_room {
            Some(s) if s == idx => None,
            Some(s) if s > idx => Some(s - 1),
            other => other,
        };
        self.status = format!("Deleted '{id}'");
    }

    fn load_toml(&mut self) {
        let path = std::path::Path::new(&self.load_path);
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => { self.status = format!("Load error: {e}"); return; }
        };
        let layout: Layout = match toml::from_str(&src) {
            Ok(l) => l,
            Err(e) => { self.status = format!("Parse error: {e}"); return; }
        };

        let need_bfs = layout.rooms.iter().any(|r| r.grid_x.is_none() || r.grid_y.is_none());
        let mut placements = HashMap::new();
        if need_bfs {
            let offsets = crate::plugin::compute_room_offsets(&layout);
            for room in &layout.rooms {
                let (ox, oy) = offsets.get(&room.id).copied().unwrap_or_default();
                let gx = (ox / PIECE_SIZE).round() as i32;
                let gy = (oy / PIECE_SIZE).round() as i32;
                placements.insert(room.id.clone(), (gx, gy));
            }
        } else {
            for room in &layout.rooms {
                placements.insert(room.id.clone(), (room.grid_x.unwrap(), room.grid_y.unwrap()));
            }
        }

        let next_room_num = layout.rooms.iter()
            .filter_map(|r| r.id.strip_prefix("room").and_then(|s| s.parse::<usize>().ok()))
            .max()
            .unwrap_or(0) + 1;

        self.selected_room = None;
        self.id_edited = !layout.id.is_empty();
        self.layout = layout;
        self.placements = placements;
        self.next_room_num = next_room_num;
        self.status = format!("Loaded '{}'", path.display());
    }

    fn export_toml(&mut self) {
        for room in &mut self.layout.rooms {
            if let Some(&(gx, gy)) = self.placements.get(&room.id) {
                room.grid_x = Some(gx);
                room.grid_y = Some(gy);
            }
        }
        let text = match toml::to_string(&self.layout) {
            Ok(s) => s,
            Err(e) => { self.status = format!("Serialize error: {e}"); return; }
        };
        let path = std::path::PathBuf::from(self.layout.editor_id()).with_extension("toml");
        match std::fs::write(&path, text) {
            Ok(()) => self.status = format!("Saved {}", path.display()),
            Err(e) => self.status = format!("Write error: {e}"),
        }
    }

    fn generate_esp(&mut self) {
        let errors = self.layout.validate();
        if !errors.is_empty() {
            self.status = errors.join("; ");
            return;
        }
        let plugin = Plugin::from_layout(&self.layout, 0, &self.content);
        let path = std::path::PathBuf::from(self.layout.editor_id()).with_extension("esp");
        match plugin.write_to_file(&path) {
            Ok(()) => self.status = format!("Wrote {}", path.display()),
            Err(e) => self.status = format!("ESP error: {e}"),
        }
    }
}

impl eframe::App for FnvibApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_sidebar(ctx);
        self.show_canvas(ctx);
    }
}

impl FnvibApp {
    fn show_sidebar(&mut self, ctx: &egui::Context) {
        SidePanel::left("sidebar").exact_width(270.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("FNVIB");

            ui.horizontal(|ui| {
                ui.label("Cell Name:");
                if ui.text_edit_singleline(&mut self.layout.name).changed() && !self.id_edited {
                    self.layout.id = self.layout.name.to_lowercase().replace(' ', "_");
                }
            });
            ui.horizontal(|ui| {
                ui.label("Cell ID:  ");
                let resp = ui.text_edit_singleline(&mut self.layout.id);
                if resp.changed() {
                    self.id_edited = !self.layout.id.is_empty();
                    // Sanitise: lowercase, spaces → underscores
                    self.layout.id = self.layout.id
                        .to_lowercase()
                        .chars()
                        .map(|c| if c == ' ' { '_' } else { c })
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                }
            });

            ui.separator();

            // Load
            ui.horizontal(|ui| {
                ui.label("Load:");
                ui.text_edit_singleline(&mut self.load_path)
                    .on_hover_text("Path to .toml layout file");
                if ui.button("Go").clicked() {
                    self.load_toml();
                }
            });

            ui.separator();

            // Tools
            ui.horizontal(|ui| {
                for (t, label) in [
                    (Tool::Select, "Select"),
                    (Tool::Draw, "Draw"),
                    (Tool::Erase, "Erase"),
                ] {
                    if ui.selectable_label(self.tool == t, label).clicked() {
                        self.tool = t;
                    }
                }
            });
            match self.tool {
                Tool::Select => { ui.label("Click to select. Scroll to pan."); }
                Tool::Draw   => { ui.label("Click or drag to place room."); }
                Tool::Erase  => { ui.label("Click to delete room."); }
            }

            ui.separator();

            // Properties
            egui::ScrollArea::vertical()
                .id_source("props_scroll")
                .max_height(ui.available_height() - 70.0)
                .show(ui, |ui| {
                    match self.selected_room {
                        Some(idx) if idx < self.layout.rooms.len() => {
                            self.show_room_props(ui, idx);
                        }
                        _ => { ui.label("Select a room to edit its properties."); }
                    }
                });

            // Bottom bar
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                if !self.status.is_empty() {
                    ui.colored_label(Color32::from_gray(160), &self.status);
                }
                ui.horizontal(|ui| {
                    if ui.button("Export TOML").clicked() { self.export_toml(); }
                    if ui.button("Generate ESP").clicked() { self.generate_esp(); }
                });
            });
        });
    }

    fn show_room_props(&mut self, ui: &mut egui::Ui, idx: usize) {
        let kit_names = self.kit_names.clone();
        let empty: Vec<String> = Vec::new();
        let furniture_lists     = self.lists_by_cat.get("furniture").unwrap_or(&empty).clone();
        let floor_clutter_lists = self.lists_by_cat.get("floor_clutter").unwrap_or(&empty).clone();
        let wall_deco_lists     = self.lists_by_cat.get("wall_decorations").unwrap_or(&empty).clone();
        let lights_lists        = self.lists_by_cat.get("lights").unwrap_or(&empty).clone();
        let all_ids: Vec<String> = self.layout.rooms.iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .map(|(_, r)| r.id.clone())
            .collect();

        {
            let room = &mut self.layout.rooms[idx];

            ui.heading("Properties");
            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(&mut room.id);
            });
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut room.name);
            });
            ui.label(format!("Size: {}×{} pieces", room.width, room.length));
            if room.width == 1 || room.length == 1 {
                ui.colored_label(Color32::from_rgb(90, 210, 170), "Type: Corridor");
            }

            ui.horizontal(|ui| {
                ui.label("Kit");
                ComboBox::from_id_source("room_kit")
                    .selected_text(&room.kit)
                    .show_ui(ui, |ui| {
                        for k in &kit_names {
                            ui.selectable_value(&mut room.kit, k.clone(), k);
                        }
                    });
            });

            ui.separator();
            show_list_section(ui, "Furniture", &mut room.furniture, &furniture_lists);
            show_list_section(ui, "Floor Clutter", &mut room.floor_clutter, &floor_clutter_lists);
            show_list_section(ui, "Wall Decorations", &mut room.wall_decorations, &wall_deco_lists);
            show_list_section(ui, "Lights", &mut room.lights, &lights_lists);

            ui.separator();
            ui.label("Doorways:");
            let mut remove_door: Option<usize> = None;
            for (di, door) in room.doorways.iter().enumerate() {
                ui.horizontal(|ui| {
                    let target = door.links_to.as_deref()
                        .map(|t| format!(" → {t}"))
                        .unwrap_or_default();
                    ui.label(format!("{:?} @{}{}", door.direction, door.offset, target));
                    if ui.small_button("✕").clicked() { remove_door = Some(di); }
                });
            }
            if let Some(di) = remove_door { room.doorways.remove(di); }
        }

        // Add-doorway form (uses self.new_door_* — must be after room borrow ends)
        ui.collapsing("+ Add Doorway", |ui| {
            let dirs = [Cardinal::North, Cardinal::South, Cardinal::East, Cardinal::West];
            let labels = ["North", "South", "East", "West"];
            let cur = dirs.iter().position(|&d| d == self.new_door_dir).unwrap_or(0);
            ui.horizontal(|ui| {
                ui.label("Direction:");
                ComboBox::from_id_source("new_door_dir")
                    .selected_text(labels[cur])
                    .show_ui(ui, |ui| {
                        for (&d, &l) in dirs.iter().zip(labels.iter()) {
                            ui.selectable_value(&mut self.new_door_dir, d, l);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Offset:");
                ui.add(DragValue::new(&mut self.new_door_offset).range(0..=15));
            });
            ui.horizontal(|ui| {
                ui.label("Links to:");
                ComboBox::from_id_source("new_door_link")
                    .selected_text(if self.new_door_links_to.is_empty() { "(none)" } else { &self.new_door_links_to })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.new_door_links_to.is_empty(), "(none)").clicked() {
                            self.new_door_links_to.clear();
                        }
                        for id in &all_ids {
                            ui.selectable_value(&mut self.new_door_links_to, id.clone(), id.as_str());
                        }
                    });
            });
            if ui.button("Add").clicked() {
                let links = if self.new_door_links_to.is_empty() {
                    None
                } else {
                    Some(self.new_door_links_to.clone())
                };
                self.layout.rooms[idx].doorways.push(Doorway {
                    direction: self.new_door_dir,
                    offset: self.new_door_offset,
                    links_to: links.clone(),
                });

                // Auto-add the reciprocal doorway to the linked room if it doesn't
                // already have one pointing back to this room.
                if let Some(ref target_id) = links {
                    let this_id = self.layout.rooms[idx].id.clone();
                    let recip_dir = self.new_door_dir.opposite();
                    let recip_off = reciprocal_offset(
                        &self.layout.rooms[idx],
                        self.new_door_dir,
                        self.new_door_offset,
                        self.layout.rooms.iter().find(|r| &r.id == target_id).unwrap(),
                        &self.placements,
                    ).unwrap_or(0);

                    if let Some(target) = self.layout.rooms.iter_mut()
                        .find(|r| &r.id == target_id)
                    {
                        let already_linked = target.doorways.iter()
                            .any(|d| d.links_to.as_deref() == Some(&this_id));
                        if !already_linked {
                            target.doorways.push(Doorway {
                                direction: recip_dir,
                                offset: recip_off,
                                links_to: Some(this_id),
                            });
                        }
                    }
                }
            }
        });
    }

    fn show_canvas(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (response, painter) = ui.allocate_painter(avail, egui::Sense::click_and_drag());
            let canvas = response.rect;

            // Scroll to pan
            let scroll = ctx.input(|i| i.smooth_scroll_delta);
            if response.hovered() {
                self.view_offset += scroll;
            }

            let origin = Pos2::new(
                canvas.center().x + self.view_offset.x,
                canvas.center().y + self.view_offset.y,
            );

            painter.rect_filled(canvas, Rounding::ZERO, C_BG);
            draw_grid(&painter, canvas, origin);

            // Current hover grid cell
            let hover_grid = ctx.input(|i| i.pointer.hover_pos())
                .filter(|p| canvas.contains(*p))
                .map(|p| screen_to_grid(origin, p));

            // Draw rooms
            let n = self.layout.rooms.len();
            for i in 0..n {
                let room = &self.layout.rooms[i];
                let &(gx, gy) = match self.placements.get(&room.id) {
                    Some(p) => p,
                    None => continue,
                };
                let (w, l) = (room.width, room.length);
                let rect = grid_to_screen_rect(origin, gx, gy, w, l);
                let selected = self.selected_room == Some(i);
                let hovered = self.tool == Tool::Select
                    && hover_grid.map(|(hx, hy)| {
                        hx >= gx && hx < gx + w as i32 && hy >= gy && hy < gy + l as i32
                    }).unwrap_or(false);
                let is_cor = w == 1 || l == 1;

                let fill = match (selected, hovered, is_cor) {
                    (true, _, false) => C_ROOM_SEL,
                    (true, _, true)  => C_ROOM_SEL_COR,
                    (_, true, _)     => C_ROOM_HOVER,
                    (_, _, false)    => C_ROOM,
                    (_, _, true)     => C_ROOM_COR,
                };
                painter.rect(rect, Rounding::same(2.0), fill, Stroke::new(1.5, C_BORDER));

                // Piece grid overlay
                for xi in 0..=w {
                    let x = rect.min.x + xi as f32 * CELL_PX;
                    painter.line_segment(
                        [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                        Stroke::new(0.5, C_PIECE_LINE),
                    );
                }
                for li in 0..=l {
                    let y = rect.max.y - li as f32 * CELL_PX;
                    painter.line_segment(
                        [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                        Stroke::new(0.5, C_PIECE_LINE),
                    );
                }

                // Label
                let label = format!("{}\n{}×{} {}", room.id, w, l, &room.kit);
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );

                // Doorway markers
                for door in &room.doorways {
                    if let Some(pt) = doorway_point(origin, gx, gy, w, l, door) {
                        painter.circle_filled(pt, 4.5, C_DOOR);
                    }
                }
            }

            // Draw tool preview
            if self.tool == Tool::Draw {
                let preview_start = self.drag_start.or(hover_grid);
                let preview_end = if self.drag_start.is_some() { hover_grid } else { hover_grid };
                if let (Some(s), Some(e)) = (preview_start, preview_end) {
                    let (px, py, pw, pl) = drag_rect(s, e);
                    let prect = grid_to_screen_rect(origin, px, py, pw, pl);
                    painter.rect(prect, Rounding::same(2.0), C_PREVIEW, Stroke::new(1.5, C_PREVIEW_BDR));
                    if self.drag_start.is_some() {
                        painter.text(
                            prect.center(),
                            Align2::CENTER_CENTER,
                            format!("{}×{}", pw, pl),
                            FontId::proportional(11.0),
                            Color32::WHITE,
                        );
                    }
                }
            }

            // Input handling
            let cur_grid = response.interact_pointer_pos()
                .map(|p| screen_to_grid(origin, p))
                .or(hover_grid);

            match self.tool {
                Tool::Draw => {
                    if response.drag_started() {
                        self.drag_start = cur_grid;
                    }
                    if self.drag_start.is_some() && !response.dragged() {
                        let start = self.drag_start.take().unwrap();
                        if let Some(end) = cur_grid {
                            let (gx, gy, w, l) = drag_rect(start, end);
                            self.create_room(gx, gy, w, l);
                        }
                    }
                    // Single click = 1×1 (no drag threshold crossed)
                    if response.clicked() {
                        if let Some(g) = cur_grid {
                            self.create_room(g.0, g.1, 1, 1);
                        }
                    }
                }
                Tool::Select => {
                    if response.clicked() {
                        self.selected_room = cur_grid
                            .and_then(|(gx, gy)| self.room_at_grid(gx, gy));
                    }
                }
                Tool::Erase => {
                    if response.clicked() {
                        if let Some((gx, gy)) = cur_grid {
                            if let Some(idx) = self.room_at_grid(gx, gy) {
                                self.delete_room(idx);
                            }
                        }
                    }
                }
            }
        });
    }
}

fn screen_to_grid(origin: Pos2, pos: Pos2) -> (i32, i32) {
    let dx = pos.x - origin.x;
    let dy = pos.y - origin.y;
    ((dx / CELL_PX).floor() as i32, (-dy / CELL_PX).floor() as i32)
}

fn grid_to_screen_rect(origin: Pos2, gx: i32, gy: i32, w: u32, l: u32) -> Rect {
    Rect::from_min_max(
        Pos2::new(
            origin.x + gx as f32 * CELL_PX,
            origin.y - (gy + l as i32) as f32 * CELL_PX,
        ),
        Pos2::new(
            origin.x + (gx + w as i32) as f32 * CELL_PX,
            origin.y - gy as f32 * CELL_PX,
        ),
    )
}

fn drag_rect(start: (i32, i32), end: (i32, i32)) -> (i32, i32, u32, u32) {
    let gx = start.0.min(end.0);
    let gy = start.1.min(end.1);
    let w = (start.0 - end.0).unsigned_abs() + 1;
    let l = (start.1 - end.1).unsigned_abs() + 1;
    (gx, gy, w, l)
}

fn draw_grid(painter: &egui::Painter, canvas: Rect, origin: Pos2) {
    let x0 = ((canvas.min.x - origin.x) / CELL_PX).floor() as i32 - 1;
    let x1 = ((canvas.max.x - origin.x) / CELL_PX).ceil() as i32 + 1;
    let y0 = ((origin.y - canvas.max.y) / CELL_PX).floor() as i32 - 1;
    let y1 = ((origin.y - canvas.min.y) / CELL_PX).ceil() as i32 + 1;

    for gx in x0..=x1 {
        let x = origin.x + gx as f32 * CELL_PX;
        let (w, c) = if gx == 0 { (1.0, C_AXIS) } else { (0.5, C_GRID) };
        painter.line_segment(
            [Pos2::new(x, canvas.min.y), Pos2::new(x, canvas.max.y)],
            Stroke::new(w, c),
        );
    }
    for gy in y0..=y1 {
        let y = origin.y - gy as f32 * CELL_PX;
        let (w, c) = if gy == 0 { (1.0, C_AXIS) } else { (0.5, C_GRID) };
        painter.line_segment(
            [Pos2::new(canvas.min.x, y), Pos2::new(canvas.max.x, y)],
            Stroke::new(w, c),
        );
    }
}

/// Screen position of a doorway marker (center of the relevant wall segment).
fn doorway_point(
    origin: Pos2,
    gx: i32,
    gy: i32,
    w: u32,
    l: u32,
    door: &Doorway,
) -> Option<Pos2> {
    let is_corridor = w == 1 || l == 1;
    let pi = if is_corridor { door.offset as f32 } else { door.offset as f32 + 1.0 };
    let rect = grid_to_screen_rect(origin, gx, gy, w, l);

    Some(match door.direction {
        Cardinal::North => Pos2::new(rect.min.x + (pi + 0.5) * CELL_PX, rect.min.y),
        Cardinal::South => Pos2::new(rect.min.x + (pi + 0.5) * CELL_PX, rect.max.y),
        Cardinal::East  => Pos2::new(rect.max.x, rect.max.y - (pi + 0.5) * CELL_PX),
        Cardinal::West  => Pos2::new(rect.min.x, rect.max.y - (pi + 0.5) * CELL_PX),
    })
}

/// Compute the doorway offset that room B needs on its reciprocal wall so it
/// aligns with the doorway room A placed on its side.
///
/// Both rooms must appear in `placements`. Returns `None` if the geometry
/// can't be computed (e.g. rooms not yet placed), defaulting to 0 at the call site.
fn reciprocal_offset(
    a: &crate::model::room::Room,
    a_dir: Cardinal,
    a_off: u32,
    b: &crate::model::room::Room,
    placements: &HashMap<String, (i32, i32)>,
) -> Option<u32> {
    let &(ax, ay) = placements.get(&a.id)?;
    let &(bx, by) = placements.get(&b.id)?;

    // doorway_piece_idx: corridor rooms use raw offset; others add 1 (skip corner)
    let corridor_a = a.width == 1 || a.length == 1;
    let corridor_b = b.width == 1 || b.length == 1;
    let a_piece = if corridor_a { a_off as i32 } else { a_off as i32 + 1 };

    // Absolute grid-piece column/row of the doorway
    let abs = match a_dir {
        Cardinal::North | Cardinal::South => ax + a_piece,
        Cardinal::East  | Cardinal::West  => ay + a_piece,
    };

    // Room B's origin along the same axis
    let b_origin = match a_dir {
        Cardinal::North | Cardinal::South => bx,
        Cardinal::East  | Cardinal::West  => by,
    };

    let b_piece = abs - b_origin;
    let b_off = if corridor_b { b_piece } else { b_piece - 1 };
    if b_off < 0 { return None; }
    Some(b_off as u32)
}

fn show_list_section(ui: &mut egui::Ui, label: &str, selected: &mut Vec<String>, available: &[String]) {
    ui.collapsing(label, |ui| {
        if available.is_empty() {
            ui.label("(no lists loaded)");
            return;
        }
        for name in available {
            let mut checked = selected.contains(name);
            if ui.checkbox(&mut checked, name.as_str()).changed() {
                if checked {
                    selected.push(name.clone());
                } else {
                    selected.retain(|s| s != name);
                }
            }
        }
    });
}
