#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;

use eframe::{App, egui};
use ids::Id;

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

mod ids {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Id(usize);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Connection {
    start: ConnectionPoint,
    end: ConnectionPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionPoint {
    Input(Id),
    Output(Id),
    MiddleSignal(Id),
    GateInput { gate: Id, input: Id },
    GateOutput { gate: Id, output: Id },
}

#[derive(Debug, Clone)]
enum LogicGate {
    Nand {
        inputs: [(Id, bool); 2],
        output: (Id, bool),
    },
    Custom(LogicGateMap),
}
impl LogicGate {
    pub fn step(&self) -> Self {
        match self {
            LogicGate::Nand {
                inputs: [(id1, v1), (id2, v2)],
                output: (idq, _),
            } => Self::Nand {
                inputs: [(*id1, *v1), (*id2, *v2)],
                output: (*idq, !(*v1 && *v2)),
            },
            LogicGate::Custom(map) => LogicGate::Custom(map.step()),
        }
    }

    pub fn get_input(&self, id: Id) -> Option<bool> {
        match self {
            LogicGate::Nand {
                inputs: [(id1, v1), (id2, v2)],
                ..
            } => {
                if id == *id1 {
                    Some(*v1)
                } else if id == *id2 {
                    Some(*v2)
                } else {
                    None
                }
            }
            LogicGate::Custom(logic_gate_map) => logic_gate_map.inputs.get(&id).copied(),
        }
    }

    pub fn get_output(&self, id: Id) -> Option<bool> {
        match self {
            LogicGate::Nand {
                output: (idq, q), ..
            } => (id == *idq).then_some(*q),
            LogicGate::Custom(logic_gate_map) => logic_gate_map.outputs.get(&id).copied(),
        }
    }

    pub fn set_input(&mut self, id: Id, new_value: bool) {
        match self {
            LogicGate::Nand {
                inputs: [(id1, v1), (id2, v2)],
                ..
            } => {
                if id == *id1 {
                    *v1 = new_value;
                } else if id == *id2 {
                    *v2 = new_value;
                }
            }
            LogicGate::Custom(logic_gate_map) => {
                logic_gate_map.inputs.insert(id, new_value);
            }
        }
    }

    pub fn set_output(&mut self, id: Id, new_value: bool) {
        match self {
            LogicGate::Nand {
                output: (idq, q), ..
            } => {
                if id == *idq {
                    *q = new_value
                }
            }
            LogicGate::Custom(logic_gate_map) => {
                logic_gate_map.outputs.insert(id, new_value);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct LogicGateMap {
    inputs: HashMap<Id, bool>,
    outputs: HashMap<Id, bool>,
    middle_signals: HashMap<Id, bool>,
    gates: HashMap<Id, LogicGate>,
    connections: HashMap<Id, Connection>,
}
impl LogicGateMap {
    pub fn empty() -> Self {
        Self {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            middle_signals: HashMap::new(),
            gates: HashMap::new(),
            connections: HashMap::new(),
        }
    }

    pub fn step(&self) -> Self {
        let mut new_map = self.clone();
        for (id, gate) in &mut new_map.gates {
            *gate = self.gates[id].step();
        }
        for Connection { start, end } in self.connections.values() {
            let input_value = match start {
                ConnectionPoint::GateInput { gate, input } => self.gates[gate]
                    .get_input(*input)
                    .expect("should be able to get input!"),
                ConnectionPoint::GateOutput { gate, output } => self.gates[gate]
                    .get_output(*output)
                    .expect("should be able to get output!"),
                ConnectionPoint::Input(id) => self.inputs[id],
                ConnectionPoint::Output(id) => self.outputs[id],
                ConnectionPoint::MiddleSignal(id) => self.middle_signals[id],
            };
            match *end {
                ConnectionPoint::Input(id) => {
                    new_map.inputs.insert(id, input_value);
                }
                // NOTE: maybe this ^^^ should be some sort of error??
                ConnectionPoint::Output(id) => {
                    new_map.outputs.insert(id, input_value);
                }
                ConnectionPoint::MiddleSignal(id) => {
                    new_map.middle_signals.insert(id, input_value);
                }
                ConnectionPoint::GateInput { gate, input } => new_map
                    .gates
                    .get_mut(&gate)
                    .expect("should be able to get gate by ID!")
                    .set_input(input, input_value),
                ConnectionPoint::GateOutput { gate, output } => new_map
                    .gates
                    .get_mut(&gate)
                    .expect("should be able to get gate by ID")
                    .set_output(output, input_value), // NOTE: maybe this ^^^ shuold be some sort of error??
            }
        }
        new_map
    }
}

struct LogicGateApp {
    map: LogicGateMap,
}
impl Default for LogicGateApp {
    fn default() -> Self {
        Self {
            map: LogicGateMap::empty(),
        }
    }
}
impl App for LogicGateApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.map = self.map.step();
    }
}
