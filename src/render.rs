use std::collections::HashMap;

use eframe::egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::{id::Id, logic_gate::ConnectionPoint, logic_gate_map::LogicGateMap};

/// the result of calculating the layout of items on the screen
/// we're using an immediate-mode GUI, so this is reconstructed every frame
/// and state is not saved
#[derive(Debug, Clone, Default)]
pub struct MapRenderSavedState {
    inputs: Vec<Id>,
    outputs: Vec<Id>,
    middle_signals: HashMap<Id, SignalRenderSavedState>,
    gates: HashMap<Id, GateRenderSavedState>,
}
impl MapRenderSavedState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_gate(&self, gate_id: Id) -> bool {
        self.gates.contains_key(&gate_id)
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
        for (id, input) in map.inputs_mut() {
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

        for (id, output) in map.outputs() {
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
            let height = map
                .gate_by_id(*id)
                .input_count()
                .max(map.gate_by_id(*id).output_count()) as f32
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
            for (input_id, value) in map.gate_by_id(*id).inputs().into_iter() {
                let position = self.gate_input_position(map, *id, input_id);
                painter.circle_filled(position, 20.0, if value { ON_COLOUR } else { OFF_COLOUR });
            }
            // TODO: draw output array
            for (output_id, value) in map.gate_by_id(*id).outputs().into_iter() {
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
#[allow(unused)]
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
