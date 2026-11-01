use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StateId {
    S0,
    S1,
    S2,
    S3,
    S4,
}

impl StateId {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "S0" => Some(StateId::S0),
            "S1" => Some(StateId::S1),
            "S2" => Some(StateId::S2),
            "S3" => Some(StateId::S3),
            "S4" => Some(StateId::S4),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    pub timestamp: i64,
    pub state: StateId,
    pub return_forward: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionCounts {
    pub counts: BTreeMap<StateId, BTreeMap<StateId, u64>>,
    pub total_transitions: u64,
}

impl TransitionCounts {
    pub fn from_sequence(sequence: &[StateEntry]) -> Self {
        let all_states = [
            StateId::S0,
            StateId::S1,
            StateId::S2,
            StateId::S3,
            StateId::S4,
        ];
        let mut counts: BTreeMap<StateId, BTreeMap<StateId, u64>> = BTreeMap::new();
        for from_state in all_states.iter() {
            let mut row = BTreeMap::new();
            for to_state in all_states.iter() {
                row.insert(*to_state, 0);
            }
            counts.insert(*from_state, row);
        }
        let mut total = 0u64;
        for window in sequence.windows(2) {
            let from = &window[0];
            let to = &window[1];
            if let Some(row) = counts.get_mut(&from.state) {
                if let Some(count) = row.get_mut(&to.state) {
                    *count += 1;
                    total += 1;
                }
            }
        }
        TransitionCounts {
            counts,
            total_transitions: total,
        }
    }

    pub fn to_probabilities(&self) -> BTreeMap<StateId, BTreeMap<StateId, f64>> {
        let mut probs = BTreeMap::new();
        for (from_state, row) in &self.counts {
            let row_sum: u64 = row.values().sum();
            let mut prob_row = BTreeMap::new();
            if row_sum > 0 {
                for (to_state, &count) in row {
                    prob_row.insert(*to_state, count as f64 / row_sum as f64);
                }
            }
            probs.insert(*from_state, prob_row);
        }
        probs
    }

    pub fn model_dwell_time_steps(&self, state: StateId) -> Option<f64> {
        let probs = self.to_probabilities();
        let p_ii = probs.get(&state)?.get(&state)?;
        if *p_ii >= 1.0 {
            Some(f64::INFINITY)
        } else {
            Some(1.0 / (1.0 - p_ii))
        }
    }

    pub fn empirical_dwell_time_steps(&self, sequence: &[StateEntry], state: StateId) -> f64 {
        let mut durations = Vec::new();
        let mut current = 0u32;
        for entry in sequence {
            if entry.state == state {
                current += 1;
            } else {
                if current > 0 {
                    durations.push(current);
                    current = 0;
                }
            }
        }
        if current > 0 {
            durations.push(current);
        }
        if durations.is_empty() {
            0.0
        } else {
            durations.iter().sum::<u32>() as f64 / durations.len() as f64
        }
    }

    pub fn stationary_distribution(
        &self,
        max_iter: usize,
        tol: f64,
    ) -> Option<BTreeMap<StateId, f64>> {
        let probs = self.to_probabilities();
        let states: Vec<StateId> = self.counts.keys().cloned().collect();
        if states.is_empty() {
            return None;
        }
        let n = states.len();
        let mut pi: Vec<f64> = vec![1.0 / n as f64; n];
        for _ in 0..max_iter {
            let mut new_pi = vec![0.0; n];
            for (i, from_state) in states.iter().enumerate() {
                if let Some(row) = probs.get(from_state) {
                    for (j, to_state) in states.iter().enumerate() {
                        new_pi[j] += pi[i] * row.get(to_state).unwrap_or(&0.0);
                    }
                }
            }
            let diff: f64 = new_pi.iter().zip(&pi).map(|(a, b)| (a - b).abs()).sum();
            pi = new_pi;
            if diff < tol {
                break;
            }
        }
        Some(states.iter().cloned().zip(pi).collect())
    }
}

/// Jensen-Shannon Divergence между двумя transition models
pub fn js_divergence(model1: &TransitionCounts, model2: &TransitionCounts) -> f64 {
    let p1 = model1.to_probabilities();
    let p2 = model2.to_probabilities();
    let states: Vec<StateId> = model1.counts.keys().cloned().collect();
    let mut js_total = 0.0;

    for from_state in &states {
        let row1 = p1.get(from_state);
        let row2 = p2.get(from_state);
        if let (Some(r1), Some(r2)) = (row1, row2) {
            for to_state in &states {
                let p = *r1.get(to_state).unwrap_or(&0.0);
                let q = *r2.get(to_state).unwrap_or(&0.0);
                let m = (p + q) / 2.0;
                if m > 0.0 {
                    let kl1 = if p > 0.0 { p * (p / m).ln() } else { 0.0 };
                    let kl2 = if q > 0.0 { q * (q / m).ln() } else { 0.0 };
                    js_total += 0.5 * kl1 + 0.5 * kl2;
                }
            }
        }
    }
    js_total / states.len() as f64
}

/// Модель второго порядка: P(S_{t+1} | S_t, S_{t-1})
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondOrderTransitionCounts {
    pub counts: BTreeMap<(StateId, StateId), BTreeMap<StateId, u64>>,
    pub total_transitions: u64,
}

impl SecondOrderTransitionCounts {
    pub fn from_sequence(sequence: &[StateEntry]) -> Self {
        let all_states = [
            StateId::S0,
            StateId::S1,
            StateId::S2,
            StateId::S3,
            StateId::S4,
        ];
        let mut counts: BTreeMap<(StateId, StateId), BTreeMap<StateId, u64>> = BTreeMap::new();

        // Инициализируем все комбинации
        for s1 in all_states.iter() {
            for s2 in all_states.iter() {
                let mut row = BTreeMap::new();
                for s3 in all_states.iter() {
                    row.insert(*s3, 0);
                }
                counts.insert((*s1, *s2), row);
            }
        }

        let mut total = 0u64;
        for window in sequence.windows(3) {
            let s_prev = window[0].state;
            let s_curr = window[1].state;
            let s_next = window[2].state;
            if let Some(row) = counts.get_mut(&(s_prev, s_curr)) {
                if let Some(count) = row.get_mut(&s_next) {
                    *count += 1;
                    total += 1;
                }
            }
        }

        SecondOrderTransitionCounts {
            counts,
            total_transitions: total,
        }
    }

    pub fn to_probabilities(&self) -> BTreeMap<(StateId, StateId), BTreeMap<StateId, f64>> {
        let mut probs = BTreeMap::new();
        for (context, row) in &self.counts {
            let row_sum: u64 = row.values().sum();
            let mut prob_row = BTreeMap::new();
            if row_sum > 0 {
                for (to_state, &count) in row {
                    prob_row.insert(*to_state, count as f64 / row_sum as f64);
                }
            }
            probs.insert(*context, prob_row);
        }
        probs
    }
}

