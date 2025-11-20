#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod id;
mod logic_gate;
mod logic_gate_map;
mod parse;
mod render;

use logic_gate_map::LogicGateMap;
use parse::parse_text;
use render::MapRenderSavedState;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use eframe::{
    App,
    egui::{self, PointerButton},
};

fn main() -> eframe::Result {
    env_logger::init();
    eframe::run_native(
        "Logic Gate Simulator",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 600.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::<LogicGateApp>::default())),
    )
}

struct LogicGateApp {
    map: Arc<RwLock<LogicGateMap>>,
    closed: Arc<AtomicBool>,
    render_data: MapRenderSavedState,
}
impl Default for LogicGateApp {
    fn default() -> Self {
        let mut maps: Vec<(LogicGateMap, Option<MapRenderSavedState>)> = vec![];
        for filename in std::env::args().skip(1) {
            let data = std::fs::read_to_string(filename).unwrap();
            let new_maps = parse_text(data.as_str()).unwrap();
            maps.extend_from_slice(new_maps.as_slice());
        }

        let (map, render_data) = maps
            .iter()
            .rev()
            .find(|(_, b)| b.is_some())
            .map(|(a, b)| (a.clone(), b.as_ref().unwrap().clone()))
            .clone()
            .expect("should be able to find a renderable map!");

        let map = Arc::new(RwLock::new(map));
        let _update_map_clone = Arc::clone(&map);
        let closed = Arc::new(AtomicBool::new(false));
        let _update_closed_clone = Arc::clone(&closed);
        /* let _ = std::thread::spawn(move || {
            while !update_closed_clone.load(Ordering::Relaxed) {
                let mut map;
                {
                    map = update_map_clone
                        .read()
                        .expect("Should be able to update map!")
                        .clone();
                    for _ in 0..10 {
                        let start = std::time::Instant::now();
                        map = map.step();
                        let end = std::time::Instant::now();
                        let duration = end - start;
                        dbg!(duration.as_millis());
                    }
                }
                *update_map_clone
                    .write()
                    .expect("Should be able to update map!") = map;

                std::thread::sleep(Duration::from_millis(0));
            }
        }); */

        Self {
            map,
            closed,
            render_data,
        }
    }
}
impl App for LogicGateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let click_position = ctx
            .input(|i| {
                i.pointer
                    .button_pressed(PointerButton::Primary)
                    .then_some(i.pointer.interact_pos())
            })
            .flatten();
        egui::CentralPanel::default().show(ctx, |ui| {
            // NOTE: there's a lot of allocation and deallocation here
            // if we had some sort of double-buffer, then there would only
            // be efficiency losses from basically memcpy-ing all the stuffs

            {
                let mut writeable = self.map.write().expect("should be able to render map!");
                for _ in 0..10 {
                    *writeable = writeable.step();
                }
                self.render_data
                    .process_input_and_render(&mut writeable, click_position, ui)
                    .expect("should be able to update and render!");
            }

            ctx.request_repaint();

            if ui.should_close() {
                self.closed.store(true, Ordering::Relaxed);
            }
        });
    }
}
