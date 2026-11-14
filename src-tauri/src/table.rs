use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub number: u32,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub name: String,
    pub phases: Vec<Phase>,
}

static TABLE: OnceLock<Vec<Area>> = OnceLock::new();

pub(crate) fn table() -> &'static [Area] {
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/entwicklungstabelle.json"))
            .expect("entwicklungstabelle.json is not valid")
    })
}

#[tauri::command]
pub fn get_areas() -> Vec<String> {
    table().iter().map(|a| a.name.clone()).collect()
}

#[tauri::command]
pub fn get_phase(area: usize, phase: u32) -> Result<Phase, String> {
    let a = table()
        .get(area)
        .ok_or_else(|| format!("No area with index {area}"))?;
    a.phases
        .iter()
        .find(|p| p.number == phase)
        .cloned()
        .ok_or_else(|| format!("{} has no phase {phase}", a.name))
}

#[tauri::command]
pub fn get_table() -> Vec<Area> {
    table().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_areas_and_phases() {
        let t = table();
        assert_eq!(t.len(), 8);
        for area in t {
            assert_eq!(area.phases.len(), 18);
        }
    }
}
