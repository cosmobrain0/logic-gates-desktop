use std::collections::HashMap;

use crate::{
    create_connection, create_custom_gate, create_input, create_nand_gate, create_output, gate,
    id::{Id, IdGenerator},
    logic_gate::{Connection, ConnectionPoint, GateCreationInfo, LogicGate},
    point,
};

#[derive(Debug, Clone)]
pub struct LogicGateMap {
    inputs: HashMap<Id, bool>,
    outputs: HashMap<Id, bool>,
    middle_signals: HashMap<Id, bool>,
    gates: HashMap<Id, LogicGate>,
    connections: HashMap<Id, Connection>,
    id_generator: IdGenerator,
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

    pub fn connection_point_value(&self, connection_point: &ConnectionPoint) -> bool {
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

    pub fn set_input(&mut self, id: Id, initial_value: bool) {
        self.inputs.insert(id, initial_value);
    }

    pub fn outputs(&self) -> impl Iterator<Item = (Id, bool)> {
        self.outputs.iter().map(|(a, b)| (*a, *b))
    }

    pub fn output_by_id(&self, id: Id) -> bool {
        self.outputs[&id]
    }

    pub fn set_output(&mut self, id: Id, initial_value: bool) {
        self.outputs.insert(id, initial_value);
    }

    pub fn middle_signals(&self) -> impl Iterator<Item = (Id, bool)> {
        self.middle_signals.iter().map(|(a, b)| (*a, *b))
    }

    pub fn connections(&self) -> impl Iterator<Item = (Id, Connection)> {
        self.connections
            .iter()
            .map(|(id, connection)| (*id, *connection))
    }

    pub fn gate_by_id(&self, id: Id) -> &LogicGate {
        &self.gates[&id]
    }

    pub fn gates(&self) -> impl Iterator<Item = Id> {
        self.gates.keys().copied()
    }
}
#[allow(unused)]
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
