use std::collections::HashMap;

use eframe::egui::Pos2;

use crate::{
    id::Id,
    logic_gate::{ConnectionPoint, GateCreationInfo},
    logic_gate_map::LogicGateMap,
    render::MapRenderSavedState,
};

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum LogicGateMapParseError {
    MissingVersionLine,
    InvalidVersionLine(String),
    NoCurrentGate(usize, String),
    InvalidCustomGate(usize, Vec<String>, String),
    UnrecognisedCommand(usize, String),
    InvalidConnectionPoint(usize, String, String),
    InvalidRenderLine(usize, String),
}

pub fn parse_text(
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
        0 => parse_version_0(lines),
        _ => Err(LogicGateMapParseError::InvalidVersionLine(
            version_line.to_string(),
        )),
    }
}
fn parse_version_0<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> Result<Vec<(LogicGateMap, Option<MapRenderSavedState>)>, LogicGateMapParseError> {
    let mut results = HashMap::new();
    let mut renderers = HashMap::new();
    let mut current = None;

    let mut inputs = HashMap::new();
    let mut outputs = HashMap::new();
    let mut nands = HashMap::new();
    let mut custom_gates = HashMap::new();

    for (line_number, line) in lines.enumerate() {
        if let Some(name) = line.strip_prefix("define_gate ") {
            results.insert(name.to_string(), LogicGateMap::empty());
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

                let start = parse_version_0_connection_point(
                    line_number,
                    line,
                    parts[0],
                    &inputs,
                    &outputs,
                    &nands,
                    &custom_gates,
                )?;
                let end = parse_version_0_connection_point(
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
                let Some(id) = nands.get(parts[0]).map(|gate| gate.gate_id()) else {
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
                    name,
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
                let Some(id) = custom_gates.get(parts[0]).map(|gate| gate.gate_id()) else {
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
                    name,
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
                .gates()
                .all(|x| renderer.has_gate(x))
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
