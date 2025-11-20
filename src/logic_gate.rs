use crate::{id::*, logic_gate_map::LogicGateMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connection {
    pub start: ConnectionPoint,
    pub end: ConnectionPoint,
}
impl From<(ConnectionPoint, ConnectionPoint)> for Connection {
    fn from((start, end): (ConnectionPoint, ConnectionPoint)) -> Self {
        Connection { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum ConnectionPoint {
    Input(Id),
    Output(Id),
    MiddleSignal(Id),
    GateInput { gate: Id, input: Id },
    GateOutput { gate: Id, output: Id },
}

#[derive(Debug, Clone)]
pub enum LogicGate {
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
            LogicGate::Custom(logic_gate_map) => Some(logic_gate_map.input_by_id(id)),
        }
    }

    pub fn get_output(&self, id: Id) -> Option<bool> {
        match self {
            LogicGate::Nand {
                output: (idq, q), ..
            } => (id == *idq).then_some(*q),
            LogicGate::Custom(logic_gate_map) => Some(logic_gate_map.output_by_id(id)),
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
                logic_gate_map.set_input(id, new_value);
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
                logic_gate_map.set_output(id, new_value);
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
                let mut ids = logic_gate_map.inputs().map(|x| x.0).collect::<Vec<_>>();
                ids.sort();
                ids.iter()
                    .enumerate()
                    .find(|(_, x)| **x == id)
                    .map(|(i, _)| i)
                    .expect("should be able to find input with that ID!")
            }
        }
    }

    pub fn input_count(&self) -> usize {
        match self {
            LogicGate::Nand { .. } => 2,
            LogicGate::Custom(map) => map.inputs().count(),
        }
    }
    pub fn get_output_index(&self, id: Id) -> usize {
        match self {
            LogicGate::Nand { .. } => 0,
            LogicGate::Custom(logic_gate_map) => {
                let mut ids = logic_gate_map.outputs().map(|x| x.0).collect::<Vec<_>>();
                ids.sort();
                ids.iter()
                    .enumerate()
                    .find(|(_, x)| **x == id)
                    .map(|(i, _)| i)
                    .expect("should be able to find input with that ID!")
            }
        }
    }

    pub fn output_count(&self) -> usize {
        match self {
            LogicGate::Nand { .. } => 1,
            LogicGate::Custom(map) => map.outputs().count(),
        }
    }

    pub fn inputs(&self) -> Vec<(Id, bool)> {
        let mut inputs = match self {
            LogicGate::Nand { inputs, .. } => {
                inputs.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>()
            }
            LogicGate::Custom(logic_gate_map) => logic_gate_map.inputs().collect::<Vec<_>>(),
        };
        inputs.sort_by_key(|(a, _)| *a);
        inputs
    }
    pub fn outputs(&self) -> Vec<(Id, bool)> {
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
pub struct GateCreationInfo {
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

    pub fn gate_id(&self) -> Id {
        self.gate_id
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

    pub fn output_connection(&self, index: usize) -> ConnectionPoint {
        ConnectionPoint::GateOutput {
            gate: self.gate_id,
            output: self.outputs[index],
        }
    }

    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }
}

#[macro_export]
macro_rules! create_input {
    ($map: ident, $($input:ident)*) => {
        $(let $input = ConnectionPoint::Input($map.create_input());)+
    };
}
#[macro_export]
macro_rules! create_nand_gate {
    ($map: ident, $($gate:ident)*) => {
        $(let $gate = $map.create_nand_gate();)+
    }
}
#[macro_export]
macro_rules! create_custom_gate {
    ($map: ident, $($gate:ident = $gate_value:expr);*) => {
        $(
            let $gate = $map.create_custom_gate($gate_value);
        )*
    }
}
#[macro_export]
macro_rules! create_output {
    ($map: ident, $($output:ident)*) => {
        $(let $output = ConnectionPoint::Output($map.create_output());)+
    }
}
#[macro_export]
macro_rules! create_connection {
    ($map: ident, $($start:expr => $($end:expr),+);*) => {
        $(
            $(
                $map.create_connection(($start, $end));
            )+
        )*
    }
}
#[macro_export]
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
#[macro_export]
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
