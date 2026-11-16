use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::State;

use crate::table::table;

const PHASE_START_MONTHS: [u32; 18] = [
    0, 3, 6, 9, 12, 18, 24, 30, 36, 42, 48, 54, 60, 66, 72, 84, 96, 108,
];

pub fn expected_phase(age_months: u32) -> u32 {
    PHASE_START_MONTHS
        .iter()
        .rposition(|&start| age_months >= start)
        .map(|i| i as u32 + 1)
        .unwrap_or(1)
}

pub fn start_phase(age_months: u32) -> u32 {
    expected_phase(age_months).saturating_sub(1).max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Ja,
    Nein,
    Teilweise,
    WeissNicht,
}

impl Answer {
    fn points(self) -> Option<f64> {
        match self {
            Answer::Ja => Some(1.0),
            Answer::Teilweise => Some(0.5),
            Answer::Nein => Some(0.0),
            Answer::WeissNicht => None,
        }
    }
}

pub fn phase_value(answers: &[Answer]) -> Option<f64> {
    let counted: Vec<f64> = answers.iter().filter_map(|a| a.points()).collect();
    if counted.is_empty() {
        None
    } else {
        Some(counted.iter().sum::<f64>() / counted.len() as f64)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseResult {
    pub phase: u32,
    pub value: Option<f64>,
    pub answers: Vec<Answer>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AnswerTable {
    pub ja: Vec<String>,
    pub teilweise: Vec<String>,
    pub nein: Vec<String>,
    pub weiss_nicht: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AreaResult {
    pub area: String,
    pub floor: u32,
    pub ceiling: u32,
    pub total: f64,
    pub phases: Vec<PhaseResult>,
    pub table: AnswerTable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Question {
        area_index: usize,
        area_count: usize,
        area: String,
        phase: u32,
        condition_index: usize,
        condition_count: usize,
        title: String,
        example: String,
        answered: usize,
    },
    Finished {
        results: Vec<AreaResult>,
        median: f64,
    },
}

struct AreaRun {
    area: usize,
    current: Option<u32>,
    condition: usize,
    answers: BTreeMap<u32, Vec<Answer>>,
}

pub struct Assessment {
    age_months: u32,
    start: u32,
    run: Option<AreaRun>,
    results: Vec<AreaResult>,
    history: Vec<Answer>,
}

impl Default for Assessment {
    fn default() -> Self {
        Self::new(0)
    }
}

fn phase_count(area: usize) -> u32 {
    table()[area].phases.len() as u32
}

fn condition_count(area: usize, phase: u32) -> usize {
    table()[area]
        .phases
        .iter()
        .find(|p| p.number == phase)
        .map(|p| p.conditions.len())
        .unwrap_or(0)
}

fn condition_number(area: usize, phase: u32, index: usize) -> u32 {
    table()[area]
        .phases
        .iter()
        .find(|p| p.number == phase)
        .and_then(|p| p.conditions.get(index))
        .map(|c| c.number)
        .unwrap_or(index as u32 + 1)
}

pub fn evaluate(values: &BTreeMap<u32, Option<f64>>, start: u32, max: u32) -> Result<(u32, u32), u32> {
    let get = |p: u32| values.get(&p).copied();

    let mut top = max + 1;
    let mut p = start;
    while p <= max {
        match get(p) {
            None => return Err(p),
            Some(Some(v)) if v < 1.0 => {
                top = p;
                break;
            }
            _ => {}
        }
        p += 1;
    }

    let mut floor = 0;
    let mut p = top - 1;
    while p >= 1 {
        match get(p) {
            None => return Err(p),
            Some(Some(v)) if v >= 1.0 => {
                floor = p;
                break;
            }
            _ => {}
        }
        p -= 1;
    }

    let mut ceiling = max;
    let mut p = floor + 1;
    while p <= max {
        match get(p) {
            None => return Err(p),
            Some(Some(v)) if v <= 0.0 => {
                ceiling = p;
                break;
            }
            _ => {}
        }
        p += 1;
    }

    Ok((floor, ceiling))
}

pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

impl Assessment {
    pub fn new(age_months: u32) -> Self {
        Self {
            age_months,
            start: start_phase(age_months),
            run: None,
            results: Vec::new(),
            history: Vec::new(),
        }
    }

    fn finish_area(&mut self, floor: u32, ceiling: u32) {
        let Some(run) = self.run.take() else { return };

        let mut table_out = AnswerTable::default();
        let mut phases = Vec::new();
        let mut total = floor as f64;

        for (&phase, answers) in &run.answers {
            let value = phase_value(answers);
            if phase > floor && phase <= ceiling {
                total += value.unwrap_or(0.0);
            }
            for (i, a) in answers.iter().enumerate() {
                let label = format!("{}.{}", phase, condition_number(run.area, phase, i));
                match a {
                    Answer::Ja => table_out.ja.push(label),
                    Answer::Teilweise => table_out.teilweise.push(label),
                    Answer::Nein => table_out.nein.push(label),
                    Answer::WeissNicht => table_out.weiss_nicht.push(label),
                }
            }
            phases.push(PhaseResult {
                phase,
                value,
                answers: answers.clone(),
            });
        }

        self.results.push(AreaResult {
            area: table()[run.area].name.clone(),
            floor,
            ceiling,
            total,
            phases,
            table: table_out,
        });
    }

    pub fn advance(&mut self) -> Step {
        loop {
            if self.run.is_none() {
                let area = self.results.len();
                if area >= table().len() {
                    let totals: Vec<f64> = self.results.iter().map(|r| r.total).collect();
                    return Step::Finished {
                        results: self.results.clone(),
                        median: median(&totals),
                    };
                }
                self.run = Some(AreaRun {
                    area,
                    current: None,
                    condition: 0,
                    answers: BTreeMap::new(),
                });
            }

            let start = self.start;
            let run = self.run.as_mut().unwrap();
            let max = phase_count(run.area);

            if let Some(phase) = run.current {
                let total = condition_count(run.area, phase);
                if run.condition < total {
                    let area = &table()[run.area];
                    let condition = &area
                        .phases
                        .iter()
                        .find(|p| p.number == phase)
                        .unwrap()
                        .conditions[run.condition];
                    return Step::Question {
                        area_index: run.area,
                        area_count: table().len(),
                        area: area.name.clone(),
                        phase,
                        condition_index: run.condition,
                        condition_count: total,
                        title: condition.title.clone(),
                        example: condition.example.clone(),
                        answered: self.history.len(),
                    };
                }
                run.answers.entry(phase).or_default();
                run.current = None;
            }

            let values: BTreeMap<u32, Option<f64>> = run
                .answers
                .iter()
                .map(|(p, a)| (*p, phase_value(a)))
                .collect();

            match evaluate(&values, start.min(max).max(1), max) {
                Err(phase) => {
                    run.current = Some(phase);
                    run.condition = 0;
                }
                Ok((floor, ceiling)) => self.finish_area(floor, ceiling),
            }
        }
    }

    pub fn answer(&mut self, answer: Answer) -> Result<Step, String> {
        let run = self.run.as_mut().ok_or("No question is open")?;
        let phase = run.current.ok_or("No question is open")?;
        run.answers.entry(phase).or_default().push(answer);
        run.condition += 1;
        self.history.push(answer);
        Ok(self.advance())
    }

    pub fn age_months(&self) -> u32 {
        self.age_months
    }

    pub fn finished(&self) -> Option<(Vec<AreaResult>, f64)> {
        if self.run.is_some() || self.results.is_empty() || self.results.len() < table().len() {
            return None;
        }
        let totals: Vec<f64> = self.results.iter().map(|r| r.total).collect();
        Some((self.results.clone(), median(&totals)))
    }

    pub fn back(&mut self) -> Step {
        let mut history = std::mem::take(&mut self.history);
        history.pop();
        let mut fresh = Assessment::new(self.age_months);
        let mut step = fresh.advance();
        for answer in history {
            match fresh.answer(answer) {
                Ok(next) => step = next,
                Err(_) => break,
            }
        }
        *self = fresh;
        step
    }
}

#[tauri::command]
pub fn start_assessment(age_months: u32, state: State<'_, Mutex<Assessment>>) -> Result<Step, String> {
    let mut assessment = state.lock().map_err(|e| e.to_string())?;
    *assessment = Assessment::new(age_months);
    Ok(assessment.advance())
}

#[tauri::command]
pub fn answer(answer: Answer, state: State<'_, Mutex<Assessment>>) -> Result<Step, String> {
    state.lock().map_err(|e| e.to_string())?.answer(answer)
}

#[tauri::command]
pub fn go_back(state: State<'_, Mutex<Assessment>>) -> Result<Step, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.back())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(age_months: u32, pick: impl Fn(u32) -> Answer) -> (Vec<AreaResult>, f64) {
        let mut a = Assessment::new(age_months);
        let mut step = a.advance();
        loop {
            match step {
                Step::Question { phase, .. } => step = a.answer(pick(phase)).unwrap(),
                Step::Finished { results, median } => return (results, median),
            }
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn start_is_one_below_expected() {
        assert_eq!(expected_phase(0), 1);
        assert_eq!(expected_phase(17), 5);
        assert_eq!(expected_phase(18), 6);
        assert_eq!(expected_phase(71), 14);
        assert_eq!(expected_phase(72), 15);
        assert_eq!(expected_phase(108), 18);
        assert_eq!(expected_phase(200), 18);
        assert_eq!(start_phase(0), 1);
        assert_eq!(start_phase(4), 1);
        assert_eq!(start_phase(19), 5);
    }

    #[test]
    fn phase_value_excludes_weiss_nicht() {
        use Answer::*;
        assert_eq!(phase_value(&[Ja, Ja, Teilweise, Nein]), Some(0.625));
        assert_eq!(phase_value(&[Ja, WeissNicht]), Some(1.0));
        assert_eq!(phase_value(&[WeissNicht, WeissNicht]), None);
    }

    #[test]
    fn all_ja() {
        let (results, median) = run(19, |_| Answer::Ja);
        for r in results {
            assert_eq!((r.floor, r.ceiling), (18, 18));
            assert!(close(r.total, 18.0));
        }
        assert!(close(median, 18.0));
    }

    #[test]
    fn all_nein() {
        for r in run(19, |_| Answer::Nein).0 {
            assert_eq!((r.floor, r.ceiling), (0, 1));
            assert!(close(r.total, 0.0));
        }
    }

    #[test]
    fn never_fully_nein_keeps_ceiling_at_18() {
        for r in run(19, |_| Answer::Teilweise).0 {
            assert_eq!((r.floor, r.ceiling), (0, 18));
            assert!(close(r.total, 9.0));
        }
    }

    #[test]
    fn floor_ceiling_and_total_from_any_start() {
        let pick = |p: u32| match p {
            1..=5 => Answer::Ja,
            6 | 7 => Answer::Teilweise,
            _ => Answer::Nein,
        };
        for age in [0, 19, 40, 96] {
            for r in run(age, pick).0 {
                assert_eq!((r.floor, r.ceiling), (5, 8));
                assert!(close(r.total, 6.0));
            }
        }
    }

    #[test]
    fn all_weiss_nicht_phase_is_skipped() {
        let pick = |p: u32| match p {
            6 => Answer::WeissNicht,
            1..=7 => Answer::Ja,
            _ => Answer::Nein,
        };
        for r in run(19, pick).0 {
            assert_eq!((r.floor, r.ceiling), (7, 8));
            assert!(close(r.total, 7.0));
        }
    }

    #[test]
    fn table_holds_every_answer_once() {
        let pick = |p: u32| match p {
            1..=5 => Answer::Ja,
            6 => Answer::Teilweise,
            7 => Answer::WeissNicht,
            _ => Answer::Nein,
        };
        for (i, r) in run(40, pick).0.iter().enumerate() {
            let asked: usize = r.phases.iter().map(|p| p.answers.len()).sum();
            let t = &r.table;
            assert_eq!(asked, t.ja.len() + t.teilweise.len() + t.nein.len() + t.weiss_nicht.len());
            for p in &r.phases {
                assert_eq!(p.answers.len(), condition_count(i, p.phase));
            }
            assert!(t.teilweise.iter().all(|l| l.starts_with("6.")));
        }
    }

    fn question_key(step: &Step) -> Option<(usize, u32, usize, usize)> {
        match step {
            Step::Question { area_index, phase, condition_index, answered, .. } => {
                Some((*area_index, *phase, *condition_index, *answered))
            }
            Step::Finished { .. } => None,
        }
    }

    #[test]
    fn back_returns_to_previous_question() {
        let mut a = Assessment::new(40);
        let first = a.advance();
        let second = a.answer(Answer::Ja).unwrap();
        let third = a.answer(Answer::Nein).unwrap();
        assert_ne!(question_key(&second), question_key(&third));
        assert_eq!(question_key(&a.back()), question_key(&second));
        assert_eq!(question_key(&a.back()), question_key(&first));
        assert_eq!(question_key(&a.back()), question_key(&first));
        assert_eq!(question_key(&first).unwrap().3, 0);
    }

    #[test]
    fn back_can_undo_a_wrong_answer_anywhere() {
        let pick = |p: u32| match p {
            1..=5 => Answer::Ja,
            6 | 7 => Answer::Teilweise,
            _ => Answer::Nein,
        };
        let mut a = Assessment::new(40);
        let mut step = a.advance();
        let mut count = 0;
        while let Step::Question { phase, .. } = step {
            count += 1;
            step = if count % 7 == 0 {
                a.answer(Answer::WeissNicht).unwrap();
                let back = a.back();
                a.answer(pick(phase)).unwrap_or(back)
            } else {
                a.answer(pick(phase)).unwrap()
            };
        }
        let Step::Finished { results, .. } = step else { unreachable!() };
        let back = a.back();
        assert!(question_key(&back).is_some());
        for r in results {
            assert_eq!((r.floor, r.ceiling), (5, 8));
            assert!(close(r.total, 6.0));
        }
    }

    #[test]
    fn median_of_totals() {
        assert!(close(median(&[3.0, 1.0, 2.0]), 2.0));
        assert!(close(median(&[4.0, 1.0, 3.0, 2.0]), 2.5));
    }
}
