#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::{HashMap, HashSet},
    fmt::write,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use eframe::{
    App,
    egui::{
        self, Color32, PointerButton, PointerState, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
    },
};
use ids::{Id, IdGenerator};

macro_rules! create_input {
    ($map: ident, $($input:ident)*) => {
        $(let $input = ConnectionPoint::Input($map.create_input());)+
    };
}
macro_rules! create_nand_gate {
    ($map: ident, $($gate:ident)*) => {
        $(let $gate = $map.create_nand_gate();)+
    }
}
macro_rules! create_custom_gate {
    ($map: ident, $($gate:ident = $gate_value:expr);*) => {
        $(
            let $gate = $map.create_custom_gate($gate_value);
        )*
    }
}
macro_rules! create_output {
    ($map: ident, $($output:ident)*) => {
        $(let $output = ConnectionPoint::Output($map.create_output());)+
    }
}
macro_rules! create_connection {
    ($map: ident, $($start:expr => $($end:expr),+);*) => {
        $(
            $(
                $map.create_connection(($start, $end));
            )+
        )*
    }
}
macro_rules! point {
    ($x:ident) => {
        x
    };
    ($index:literal => $gate:expr) => {
        $gate.input_connection($index)
    };
    ($gate:expr => $index:literal) => {
        $gate.output_connection($index)
    };
}
macro_rules! gate {
    (inputs $($input:ident)+, nands $($nand:ident)*, custom_gates $($gate:ident = $gate_value:expr),*; outputs $($output:ident)+, connections $($start:expr => $($end:expr),+);*) => {
{
        let mut result = LogicGateMap::empty();
        $(create_input!(result, $input);)+
        $(create_nand_gate!(result, $nand);)*
        create_custom_gate!(result, $($gate = $gate_value);*);
        $(create_output!(result, $output);)+
        create_connection!(result, $($start => $($end),+);*);
        result
        }
    }
}

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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Id(usize);

    #[derive(Debug, Clone)]
    pub struct IdGenerator {
        inner: usize,
    }
    impl IdGenerator {
        pub fn new() -> Self {
            Self { inner: 0 }
        }

        pub fn generate(&mut self) -> Id {
            self.inner += 1;
            Id(self.inner - 1)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Connection {
    start: ConnectionPoint,
    end: ConnectionPoint,
}
impl From<(ConnectionPoint, ConnectionPoint)> for Connection {
    fn from((start, end): (ConnectionPoint, ConnectionPoint)) -> Self {
        Connection { start, end }
    }
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

    pub fn get_input_index(&self, id: Id) -> usize {
        match self {
            LogicGate::Nand {
                inputs: [(id1, _), (_, _)],
                ..
            } => {
                if id == *id1 {
                    0
                } else {
                    1
                }
            }
            LogicGate::Custom(logic_gate_map) => {
                let mut ids = logic_gate_map.inputs.keys().collect::<Vec<_>>();
                ids.sort();
                ids.iter()
                    .enumerate()
                    .find(|(_, x)| ***x == id)
                    .map(|(i, _)| i)
                    .expect("should be able to find input with that ID!")
            }
        }
    }

    pub fn input_count(&self) -> usize {
        match self {
            LogicGate::Nand { .. } => 2,
            LogicGate::Custom(map) => map.inputs.len(),
        }
    }
    pub fn get_output_index(&self, id: Id) -> usize {
        match self {
            LogicGate::Nand { .. } => 0,
            LogicGate::Custom(logic_gate_map) => {
                let mut ids = logic_gate_map.outputs.keys().collect::<Vec<_>>();
                ids.sort();
                ids.iter()
                    .enumerate()
                    .find(|(_, x)| ***x == id)
                    .map(|(i, _)| i)
                    .expect("should be able to find input with that ID!")
            }
        }
    }

    pub fn output_count(&self) -> usize {
        match self {
            LogicGate::Nand { .. } => 1,
            LogicGate::Custom(map) => map.outputs.len(),
        }
    }

    fn inputs(&self) -> Vec<(Id, bool)> {
        let mut inputs = match self {
            LogicGate::Nand { inputs, .. } => {
                inputs.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>()
            }
            LogicGate::Custom(logic_gate_map) => logic_gate_map.inputs().collect::<Vec<_>>(),
        };
        inputs.sort_by_key(|(a, _)| *a);
        inputs
    }
    fn outputs(&self) -> Vec<(Id, bool)> {
        let mut outputs = match self {
            LogicGate::Nand { output, .. } => {
                vec![*output]
            }
            LogicGate::Custom(logic_gate_map) => logic_gate_map.outputs().collect::<Vec<_>>(),
        };
        outputs.sort_by_key(|(a, _)| *a);
        outputs
    }
}

#[derive(Debug, Clone)]
struct GateCreationInfo {
    gate_id: Id,
    inputs: Vec<Id>,
    outputs: Vec<Id>,
}
impl GateCreationInfo {
    pub fn new(gate_id: Id, mut inputs: Vec<Id>, mut outputs: Vec<Id>) -> Self {
        inputs.sort();
        outputs.sort();
        Self {
            gate_id,
            inputs,
            outputs,
        }
    }

    pub fn input_connections(&self) -> Vec<ConnectionPoint> {
        self.inputs
            .iter()
            .map(|id| ConnectionPoint::GateInput {
                gate: self.gate_id,
                input: *id,
            })
            .collect()
    }

    pub fn input_connection(&self, index: usize) -> ConnectionPoint {
        ConnectionPoint::GateInput {
            gate: self.gate_id,
            input: self.inputs[index],
        }
    }

    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn output_connections(&self) -> Vec<ConnectionPoint> {
        self.outputs
            .iter()
            .map(|id| ConnectionPoint::GateInput {
                gate: self.gate_id,
                input: *id,
            })
            .collect()
    }

    pub fn output_connection(&self, index: usize) -> ConnectionPoint {
        ConnectionPoint::GateOutput {
            gate: self.gate_id,
            output: self.outputs[index],
        }
    }

    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    pub fn gate_id(&self) -> Id {
        self.gate_id
    }
}

#[derive(Debug, Clone)]
enum LogicGateMapParseError {
    MissingVersionLine,
    InvalidVersionLine(String),
    NoCurrentGate(usize, String),
    InvalidCustomGate(usize, Vec<String>, String),
    UnrecognisedCommand(usize, String),
    InvalidConnectionPoint(usize, String, String),
    InvalidRenderLine(usize, String),
}

#[derive(Debug, Clone)]
struct LogicGateMap {
    inputs: HashMap<Id, bool>,
    outputs: HashMap<Id, bool>,
    middle_signals: HashMap<Id, bool>,
    gates: HashMap<Id, LogicGate>,
    connections: HashMap<Id, Connection>,
    id_generator: IdGenerator,
}

fn parse_text(
    value: &str,
) -> Result<Vec<(LogicGateMap, Option<MapRenderSavedState>)>, LogicGateMapParseError> {
    let mut lines = value.lines().filter(|l| !l.trim().is_empty());
    let Some(version_line) = lines.next() else {
        return Err(LogicGateMapParseError::MissingVersionLine);
    };
    let Some(version): Option<usize> = version_line
        .strip_prefix("version ")
        .and_then(|x| x.parse().ok())
    else {
        return Err(LogicGateMapParseError::InvalidVersionLine(
            version_line.to_string(),
        ));
    };

    match version {
        0 => LogicGateMap::parse_version_0(lines),
        _ => Err(LogicGateMapParseError::InvalidVersionLine(
            version_line.to_string(),
        )),
    }
}
impl LogicGateMap {
    fn parse_version_0<'a>(
        lines: impl Iterator<Item = &'a str>,
    ) -> Result<Vec<(Self, Option<MapRenderSavedState>)>, LogicGateMapParseError> {
        let mut results = HashMap::new();
        let mut renderers = HashMap::new();
        let mut current = None;

        let mut inputs = HashMap::new();
        let mut outputs = HashMap::new();
        let mut nands = HashMap::new();
        let mut custom_gates = HashMap::new();

        for (line_number, line) in lines.enumerate() {
            if let Some(name) = line.strip_prefix("define_gate ") {
                results.insert(name.to_string(), Self::empty());
                renderers.insert(name.to_string(), MapRenderSavedState::new());
                current = Some(name.to_string());
            } else if let Some(operands) = line.strip_prefix("inputs ") {
                let Some(current) = current.as_ref() else {
                    return Err(LogicGateMapParseError::NoCurrentGate(
                        line_number,
                        line.to_string(),
                    ));
                };
                for input_name in operands
                    .split_whitespace()
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                {
                    let id = results.get_mut(current).unwrap().create_input();
                    inputs.insert(input_name.to_string(), id);
                    renderers.get_mut(current).unwrap().add_input(id);
                }
            } else if let Some(operands) = line.strip_prefix("outputs ") {
                let Some(current) = current.as_ref() else {
                    return Err(LogicGateMapParseError::NoCurrentGate(
                        line_number,
                        line.to_string(),
                    ));
                };
                for output_name in operands
                    .split_whitespace()
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                {
                    let id = results.get_mut(current).unwrap().create_output();
                    outputs.insert(output_name.to_string(), id);
                    renderers.get_mut(current).unwrap().add_output(id);
                }
            } else if let Some(operands) = line.strip_prefix("nands ") {
                let Some(current) = current.as_ref() else {
                    return Err(LogicGateMapParseError::NoCurrentGate(
                        line_number,
                        line.to_string(),
                    ));
                };
                for nand_name in operands
                    .split_whitespace()
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                {
                    let id = results.get_mut(current).unwrap().create_nand_gate();
                    nands.insert(nand_name.to_string(), id);
                }
            } else if let Some(operands) = line.strip_prefix("custom_gates ") {
                let Some(current) = current.as_ref() else {
                    return Err(LogicGateMapParseError::NoCurrentGate(
                        line_number,
                        line.to_string(),
                    ));
                };
                for definition in operands
                    .split(", ")
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                {
                    let parts: Vec<_> = definition
                        .split("=")
                        .map(|x| x.trim())
                        .filter(|x| !x.is_empty())
                        .collect();
                    if parts.len() != 2 {
                        return Err(LogicGateMapParseError::InvalidCustomGate(
                            line_number,
                            parts.into_iter().map(|x| x.to_string()).collect(),
                            line.to_string(),
                        ));
                    };
                    let Some(chosen_custom_gate) = results
                        .iter()
                        .find(|(name, _)| name.as_str() == parts[1])
                        .map(|(_, map)| map.clone())
                    else {
                        return Err(LogicGateMapParseError::InvalidCustomGate(
                            line_number,
                            parts.into_iter().map(|x| x.to_string()).collect(),
                            line.to_string(),
                        ));
                    };
                    let current = results.get_mut(current).unwrap();
                    custom_gates.insert(
                        parts[0].to_string(),
                        current.create_custom_gate(chosen_custom_gate),
                    );
                }
            } else if let Some(operands) = line.strip_prefix("connections ") {
                for definition in operands
                    .split(", ")
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                {
                    let parts: Vec<_> = definition
                        .split("=>")
                        .map(|x| x.trim())
                        .filter(|x| !x.is_empty())
                        .collect();
                    if parts.len() != 2 {
                        return Err(LogicGateMapParseError::InvalidCustomGate(
                            line_number,
                            parts.into_iter().map(|x| x.to_string()).collect(),
                            line.to_string(),
                        ));
                    };

                    let start = Self::parse_version_0_connection_point(
                        line_number,
                        line,
                        parts[0],
                        &inputs,
                        &outputs,
                        &nands,
                        &custom_gates,
                    )?;
                    let end = Self::parse_version_0_connection_point(
                        line_number,
                        line,
                        parts[1],
                        &inputs,
                        &outputs,
                        &nands,
                        &custom_gates,
                    )?;

                    let Some(current) = current.as_ref() else {
                        return Err(LogicGateMapParseError::NoCurrentGate(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let current = results.get_mut(current).unwrap();
                    current.create_connection((start, end));
                }
            } else if let Some(operands) = line.strip_prefix("render_nand_gate ") {
                let parts: Vec<_> = operands
                    .split_whitespace()
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                    .collect();
                // render_nand_gate x y some name
                if parts.len() >= 4 {
                    let Some(id) = nands.get(parts[0]).map(|gate| gate.gate_id) else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let Ok(x): Result<usize, _> = parts[1].parse() else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let Ok(y): Result<usize, _> = parts[2].parse() else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let name = parts[3..].join(" ");
                    let Some(current) = current.as_ref() else {
                        return Err(LogicGateMapParseError::NoCurrentGate(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    renderers.get_mut(current).unwrap().add_gate(
                        id,
                        Pos2::new(x as f32, y as f32),
                        name.into(),
                    );
                }
            } else if let Some(operands) = line.strip_prefix("render_custom_gate ") {
                let parts: Vec<_> = operands
                    .split_whitespace()
                    .map(|x| x.trim())
                    .filter(|x| !x.is_empty())
                    .collect();
                // render_nand_gate x y some name
                if parts.len() >= 4 {
                    let Some(id) = custom_gates.get(parts[0]).map(|gate| gate.gate_id) else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let Ok(x): Result<usize, _> = parts[1].parse() else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let Ok(y): Result<usize, _> = parts[2].parse() else {
                        return Err(LogicGateMapParseError::InvalidRenderLine(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    let name = parts[2..].join(" ");
                    let Some(current) = current.as_ref() else {
                        return Err(LogicGateMapParseError::NoCurrentGate(
                            line_number,
                            line.to_string(),
                        ));
                    };
                    renderers.get_mut(current).unwrap().add_gate(
                        id,
                        Pos2::new(x as f32, y as f32),
                        name.into(),
                    );
                }
            } else {
                return Err(LogicGateMapParseError::UnrecognisedCommand(
                    line_number,
                    line.to_string(),
                ));
            }
        }
        Ok(results
            .into_iter()
            .map(|(name, map)| (map, renderers.remove(&name).unwrap()))
            .map(|(map, renderer)| {
                let renderer = map
                    .gates
                    .keys()
                    .all(|x| renderer.gates.contains_key(x))
                    .then_some(renderer);
                (map, renderer)
            })
            .collect())
    }

    fn parse_version_0_connection_point(
        line_number: usize,
        line: &str,
        text: &str,
        inputs: &HashMap<String, Id>,
        outputs: &HashMap<String, Id>,
        nands: &HashMap<String, GateCreationInfo>,
        custom_gates: &HashMap<String, GateCreationInfo>,
    ) -> Result<ConnectionPoint, LogicGateMapParseError> {
        let parts: Vec<_> = text
            .split_whitespace()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .collect();
        match parts.len() {
            1 => {
                if let Some(id) = inputs.get(parts[0]) {
                    Ok(ConnectionPoint::Input(*id))
                } else if let Some(id) = outputs.get(parts[0]) {
                    Ok(ConnectionPoint::Output(*id))
                } else {
                    Err(LogicGateMapParseError::InvalidConnectionPoint(
                        line_number,
                        line.to_string(),
                        text.to_string(),
                    ))
                }
            }
            3 => {
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                enum IO {
                    Input,
                    Output,
                }
                let input_or_output = match parts[1] {
                    "in" => IO::Input,
                    "out" => IO::Output,
                    _ => {
                        return Err(LogicGateMapParseError::InvalidConnectionPoint(
                            line_number,
                            line.to_string(),
                            text.to_string(),
                        ));
                    }
                };
                let Some(io_index): Option<usize> = parts[2].parse().ok() else {
                    return Err(LogicGateMapParseError::InvalidConnectionPoint(
                        line_number,
                        line.to_string(),
                        text.to_string(),
                    ));
                };
                let gate = if let Some(gate) = nands.get(parts[0]) {
                    gate
                } else if let Some(gate) = custom_gates.get(parts[0]) {
                    gate
                } else {
                    return Err(LogicGateMapParseError::InvalidConnectionPoint(
                        line_number,
                        line.to_string(),
                        text.to_string(),
                    ));
                };
                match input_or_output {
                    IO::Input => {
                        if gate.input_count() > io_index {
                            Ok(gate.input_connection(io_index))
                        } else {
                            Err(LogicGateMapParseError::InvalidConnectionPoint(
                                line_number,
                                line.to_string(),
                                text.to_string(),
                            ))
                        }
                    }
                    IO::Output => {
                        if gate.output_count() > io_index {
                            Ok(gate.output_connection(io_index))
                        } else {
                            Err(LogicGateMapParseError::InvalidConnectionPoint(
                                line_number,
                                line.to_string(),
                                text.to_string(),
                            ))
                        }
                    }
                }
            }
            _ => Err(LogicGateMapParseError::InvalidConnectionPoint(
                line_number,
                line.to_string(),
                text.to_string(),
            )),
        }
    }
}
impl LogicGateMap {
    pub fn empty() -> Self {
        Self {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            middle_signals: HashMap::new(),
            gates: HashMap::new(),
            connections: HashMap::new(),
            id_generator: IdGenerator::new(),
        }
    }

    pub fn step(&self) -> Self {
        let mut new_map = self.clone();
        for (id, gate) in &mut new_map.gates {
            *gate = self.gates[id].step();
        }
        for Connection { start, end } in self.connections.values() {
            let input_value = self.connection_point_value(start);
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

    fn connection_point_value(&self, connection_point: &ConnectionPoint) -> bool {
        match connection_point {
            ConnectionPoint::GateInput { gate, input } => self.gates[gate]
                .get_input(*input)
                .expect("should be able to get input!"),
            ConnectionPoint::GateOutput { gate, output } => self.gates[gate]
                .get_output(*output)
                .expect("should be able to get output!"),
            ConnectionPoint::Input(id) => self.inputs[id],
            ConnectionPoint::Output(id) => self.outputs[id],
            ConnectionPoint::MiddleSignal(id) => self.middle_signals[id],
        }
    }
}
impl LogicGateMap {
    pub fn inputs(&self) -> impl Iterator<Item = (Id, bool)> {
        self.inputs.iter().map(|(a, b)| (*a, *b))
    }
    pub fn inputs_mut(&mut self) -> impl Iterator<Item = (Id, &mut bool)> {
        self.inputs.iter_mut().map(|(a, b)| (*a, b))
    }

    pub fn input_by_id(&self, id: Id) -> bool {
        self.inputs[&id]
    }

    pub fn outputs(&self) -> impl Iterator<Item = (Id, bool)> {
        self.outputs.iter().map(|(a, b)| (*a, *b))
    }

    pub fn output_by_id(&self, id: Id) -> bool {
        self.outputs[&id]
    }

    fn middle_signals(&self) -> impl Iterator<Item = (Id, bool)> {
        self.middle_signals.iter().map(|(a, b)| (*a, *b))
    }

    pub fn middle_signal_by_id(&self, id: Id) -> bool {
        self.middle_signals[&id]
    }

    pub fn connections(&self) -> impl Iterator<Item = (Id, Connection)> {
        self.connections
            .iter()
            .map(|(id, connection)| (*id, *connection))
    }

    pub fn gate_by_id(&self, id: Id) -> &LogicGate {
        &self.gates[&id]
    }
}
impl LogicGateMap {
    pub fn and_gate() -> Self {
        gate! {
            inputs i1 i2,
            nands nand not,
            custom_gates;
            outputs q,
            connections
            i1 => point!(0 => nand);
            i2 => point!(1 => nand);
            point!(nand => 0) => point!(0 => not), point!(1 => not);
            point!(not => 0) => q
        }
    }

    pub fn not_gate() -> Self {
        gate! {
            inputs i,
            nands nand,
            custom_gates;
            outputs q,
            connections
            i => point!(0 => nand), point!(1 => nand);
            point!(nand => 0) => q
        }
    }

    pub fn or_gate() -> Self {
        gate! {
            inputs a b,
            nands nand,
            custom_gates
                not_a = dbg!(Self::not_gate()),
                not_b = Self::not_gate();
            outputs q,
            connections
                a => point!(0 => not_a);
                b => point!(0 => not_b);
                point!(not_a => 0) => point!(0 => nand);
                point!(not_b => 0) => point!(1 => nand);
                point!(nand => 0) => q
        }
    }

    pub fn nor_gate() -> Self {
        gate! {
            inputs a b,
            nands,
            custom_gates
                not_a = Self::not_gate(),
                not_b = Self::not_gate(),
                and = Self::and_gate();
            outputs q,
            connections
                a => point!(0 => not_a);
                b => point!(0 => not_b);
                point!(not_a => 0) => point!(0 => and);
                point!(not_b => 0) => point!(1 => and);
                point!(and => 0) => q
        }
    }

    pub fn sr_latch() -> Self {
        gate! {
            inputs set reset,
            nands,
            custom_gates
                nor_a = Self::nor_gate(),
                nor_b = Self::nor_gate();
            outputs q not_q,
            connections
                set => point!(0 => nor_a);
                reset => point!(1 => nor_b);
                point!(nor_a => 0) => point!(0 => nor_b);
                point!(nor_b => 0) => point!(1 => nor_a);
                point!(nor_a => 0) => q;
                point!(nor_b => 0) => not_q
        }
    }

    pub fn d_latch() -> Self {
        gate! {
            inputs data enable,
            nands,
            custom_gates
                reset_and = Self::and_gate(),
                set_and = Self::and_gate(),
                reset_not = Self::not_gate(),
                sr_latch = Self::sr_latch();
            outputs q,
            connections
                data => point!(0 => reset_not);
                point!(reset_not => 0) => point!(0 => reset_and);
                data => point!(0 => set_and);
                enable => point!(1 => reset_and), point!(1 => set_and);
                point!(reset_and => 0) => point!(0 => sr_latch);
                point!(set_and => 0) => point!(1 => sr_latch);
                point!(sr_latch => 0) => q
        }
    }

    pub fn create_input(&mut self) -> Id {
        let id = self.id_generator.generate();
        self.inputs.insert(id, false);
        id
    }

    pub fn create_output(&mut self) -> Id {
        let id = self.id_generator.generate();
        self.outputs.insert(id, false);
        id
    }

    pub fn create_nand_gate(&mut self) -> GateCreationInfo {
        let id = self.id_generator.generate();
        let (i1, i2, q) = (
            self.id_generator.generate(),
            self.id_generator.generate(),
            self.id_generator.generate(),
        );
        self.gates.insert(
            id,
            LogicGate::Nand {
                inputs: [(i1, false), (i2, false)],
                output: (q, true),
            },
        );
        GateCreationInfo::new(id, vec![i1, i2], vec![q])
    }

    pub fn create_custom_gate(&mut self, gate: LogicGateMap) -> GateCreationInfo {
        let id = self.id_generator.generate();
        let inputs = gate.inputs().map(|(id, _)| id).collect();
        let outputs = gate.outputs().map(|(id, _)| id).collect();
        self.gates.insert(id, LogicGate::Custom(gate));
        GateCreationInfo::new(id, inputs, outputs)
    }

    pub fn create_connection(&mut self, connection: impl Into<Connection>) -> Id {
        let id = self.id_generator.generate();
        self.connections.insert(id, connection.into());
        id
    }
}

/// the result of calculating the layout of items on the screen
/// we're using an immediate-mode GUI, so this is reconstructed every frame
/// and state is not saved
/// TODO: figure out how this works with inputs
#[derive(Debug, Clone, Default)]
struct MapRenderSavedState {
    inputs: Vec<Id>,
    outputs: Vec<Id>,
    middle_signals: HashMap<Id, SignalRenderSavedState>,
    gates: HashMap<Id, GateRenderSavedState>,
}
impl MapRenderSavedState {
    pub fn new() -> Self {
        Self::default()
    }

    fn input_position(&self, id: Id) -> Pos2 {
        Pos2::new(
            30.0,
            30.0 + 70.0
                * self
                    .inputs
                    .iter()
                    .enumerate()
                    .find_map(|(i, x)| (id == *x).then_some(i))
                    .unwrap() as f32,
        )
    }

    fn output_position(&self, id: Id, screen_width: f32) -> Pos2 {
        Pos2::new(
            screen_width - 30.0,
            30.0 + 70.0
                * self
                    .outputs
                    .iter()
                    .enumerate()
                    .find(|(_, x)| **x == id)
                    .map(|(i, _)| i)
                    .unwrap() as f32,
        )
    }

    fn gate_input_position(&self, map: &LogicGateMap, gate_id: Id, input_id: Id) -> Pos2 {
        let gate_position = self.gates[&gate_id].position;
        let x = gate_position.x - 50.0;
        let input_index = map.gate_by_id(gate_id).get_input_index(input_id);
        let input_count = map.gate_by_id(gate_id).input_count();

        let input_array_height = 20.0 * 2.0 * input_count as f32;
        let input_offset = 20.0 * 2.0 * input_index as f32 + 20.0;
        let y = input_offset - input_array_height / 2.0 + gate_position.y;
        Pos2::new(x, y)
    }
    fn gate_output_position(&self, map: &LogicGateMap, gate_id: Id, output_id: Id) -> Pos2 {
        let gate_position = self.gates[&gate_id].position;
        let x = gate_position.x + 50.0;
        let output_index = map.gate_by_id(gate_id).get_output_index(output_id);
        let output_count = map.gate_by_id(gate_id).output_count();

        let output_array_height = 20.0 * 2.0 * output_count as f32;
        let output_offset = 20.0 * 2.0 * output_index as f32 + 20.0;
        let y = output_offset - output_array_height / 2.0 + gate_position.y;
        Pos2::new(x, y)
    }
    // TODO: updating functions for adding things to the renderer
    // and functions for deleting things as well
    pub fn add_input(&mut self, id: Id) {
        self.inputs.push(id);
    }

    pub fn add_output(&mut self, id: Id) {
        self.outputs.push(id);
    }

    pub fn add_middle_signal(&mut self, id: Id, position: Pos2) {
        self.middle_signals
            .insert(id, SignalRenderSavedState { position });
    }

    pub fn add_gate(&mut self, id: Id, position: Pos2, name: String) {
        self.gates
            .insert(id, GateRenderSavedState { position, name });
    }

    /// This method uses the logic gate and the saved state
    /// to render to the screen
    /// If saved state is required for an element but isn't available
    /// this function for now just ignores that element
    pub fn process_input_and_render(
        &self,
        map: &mut LogicGateMap,
        click_position: Option<Pos2>,
        ui: &mut Ui,
    ) -> Result<(), ()> {
        let painter = ui.painter();

        // draw inputs
        for (i, (id, input)) in map.inputs_mut().enumerate() {
            let shape = CircleCollider::new(self.input_position(id), 20.0);
            if let Some(click_position) = click_position
                && shape.intersects_point(click_position)
            {
                *input = !*input;
            }
            painter.circle_filled(
                shape.position(),
                shape.radius(),
                if *input { ON_COLOUR } else { OFF_COLOUR },
            );
        }

        for (i, (id, output)) in map.outputs().enumerate() {
            painter.circle_filled(
                self.output_position(id, ui.available_width()),
                20.0,
                if output { ON_COLOUR } else { OFF_COLOUR },
            );
        }

        for (id, value) in map.middle_signals() {
            painter.circle_filled(
                self.middle_signals[&id].position,
                20.0,
                if value { ON_COLOUR } else { OFF_COLOUR },
            );
        }

        for (_, connection) in map.connections() {
            let start_position = self.connection_point_position(map, ui, connection.start);
            let end_position = self.connection_point_position(map, ui, connection.end);
            let value = map.connection_point_value(&connection.start);
            painter.line_segment(
                [start_position, end_position],
                Stroke::new(3.0, if value { ON_COLOUR } else { OFF_COLOUR }),
            );
        }

        for (id, gate) in &self.gates {
            let height = map.gates[id]
                .input_count()
                .max(map.gates[id].output_count()) as f32
                * 20.0
                * 2.0;
            // TODO: draw block
            painter.rect_stroke(
                Rect::from_center_size(gate.position, Vec2::new(100.0, height)),
                0.0,
                Stroke::new(3.0, Color32::LIGHT_GRAY),
                StrokeKind::Middle,
            );
            // TODO: draw input array
            for (input_id, value) in map.gates[id].inputs().into_iter() {
                let position = self.gate_input_position(map, *id, input_id);
                painter.circle_filled(position, 20.0, if value { ON_COLOUR } else { OFF_COLOUR });
            }
            // TODO: draw output array
            for (output_id, value) in map.gates[id].outputs().into_iter() {
                let position = self.gate_output_position(map, *id, output_id);
                painter.circle_filled(position, 20.0, if value { ON_COLOUR } else { OFF_COLOUR });
            }
        }

        Ok(())
    }

    fn connection_point_position(
        &self,
        map: &LogicGateMap,
        ui: &Ui,
        connection_point: ConnectionPoint,
    ) -> Pos2 {
        match connection_point {
            ConnectionPoint::Input(id) => self.input_position(id),
            ConnectionPoint::Output(id) => self.output_position(id, ui.available_width()),
            ConnectionPoint::MiddleSignal(id) => self.middle_signals[&id].position,
            ConnectionPoint::GateInput { gate, input } => {
                self.gate_input_position(map, gate, input)
            }
            ConnectionPoint::GateOutput { gate, output } => {
                self.gate_output_position(map, gate, output)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SignalRenderSavedState {
    position: Pos2,
}

#[derive(Debug, Clone)]
struct GateRenderSavedState {
    position: Pos2,
    name: String,
}

#[derive(Debug, Clone)]
struct CircleCollider {
    position: Pos2,
    radius: f32,
}
impl CircleCollider {
    pub fn new(position: Pos2, radius: f32) -> Self {
        Self { position, radius }
    }

    pub fn intersects_point(&self, point: Pos2) -> bool {
        (point - self.position).length_sq() <= self.radius * self.radius
    }

    pub fn position(&self) -> Pos2 {
        self.position
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }
}

const ON_COLOUR: Color32 = Color32::GREEN;
const OFF_COLOUR: Color32 = Color32::RED;

struct LogicGateApp {
    map: Arc<RwLock<LogicGateMap>>,
    closed: Arc<AtomicBool>,
    render_data: MapRenderSavedState,
}
impl Default for LogicGateApp {
    fn default() -> Self {
        let data = std::fs::read_to_string("gates.dat").unwrap();
        let maps = parse_text(data.as_str()).unwrap();
        dbg!(&maps);

        let (map, render_data) = maps
            .iter()
            .rev()
            .find(|(_, b)| b.is_some())
            .map(|(a, b)| (a.clone(), b.as_ref().unwrap().clone()))
            .clone()
            .expect("should be able to find a renderable map!");

        let map = Arc::new(RwLock::new(map));
        let update_map_clone = Arc::clone(&map);
        let closed = Arc::new(AtomicBool::new(false));
        let update_closed_clone = Arc::clone(&closed);
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
