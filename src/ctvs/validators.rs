use super::observer::SecondOrderTransitionCounts;
use super::observer::{StateEntry, StateId, TransitionCounts};
use super::report::ValidationStatus;
use std::collections::BTreeMap;

pub const LOGLOSS_EPSILON: f64 = 1e-15;

#[derive(Debug, Clone, PartialEq)]
pub struct LogLossResult {
    pub mean_logloss: f64,
    pub observations: usize,
    pub zero_probability_events: usize,
}

pub fn validate_state_ids(sequence: &[StateEntry]) -> Result<(), String> {
    for entry in sequence {
        match entry.state {
            StateId::S0 | StateId::S1 | StateId::S2 | StateId::S3 | StateId::S4 => {}
        }
    }
    Ok(())
}

pub fn validate_sequence_closure(sequence: &[StateEntry]) -> Result<(), String> {
    if sequence.len() < 2 {
        return Err("CTVS-003 FAIL: sequence must contain at least 2 entries".to_string());
    }
    for window in sequence.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        if curr.timestamp <= prev.timestamp {
            return Err(format!(
                "CTVS-003 FAIL: non-monotonic timestamp at {} -> {}",
                prev.timestamp, curr.timestamp
            ));
        }
    }
    Ok(())
}

pub fn validate_independent_counts(
    sequence: &[StateEntry],
    stored_counts: &TransitionCounts,
) -> Result<(), String> {
    let computed = TransitionCounts::from_sequence(sequence);
    if computed.counts != stored_counts.counts {
        return Err("CTVS-004 FAIL: stored counts do not match sequence".to_string());
    }
    Ok(())
}

pub fn validate_probabilities(counts: &TransitionCounts) -> Result<(), String> {
    for (state_id, row) in &counts.counts {
        let row_sum: u64 = row.values().sum();
        if row_sum > 0 {
            let prob_sum: f64 = row.values().map(|&c| c as f64 / row_sum as f64).sum();
            if (prob_sum - 1.0).abs() > 1e-9 {
                return Err(format!(
                    "CTVS-005 FAIL: row for {:?} sums to {}, expected 1.0",
                    state_id, prob_sum
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_zero_origin(counts: &TransitionCounts) -> Result<(), String> {
    for (state_id, row) in &counts.counts {
        let row_sum: u64 = row.values().sum();
        if row_sum == 0 {
            let probs = counts.to_probabilities();
            if let Some(prob_row) = probs.get(state_id) {
                if !prob_row.is_empty() {
                    return Err(format!(
                        "CTVS-006 FAIL: state {:?} with zero origin has probabilities",
                        state_id
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn validate_dwell_time(
    counts: &TransitionCounts,
    sequence: &[StateEntry],
    delta_minutes: i64,
) -> Result<(), String> {
    for state_id in counts.counts.keys() {
        let model_steps = counts.model_dwell_time_steps(*state_id);
        let empirical_steps = counts.empirical_dwell_time_steps(sequence, *state_id);
        if let Some(model) = model_steps {
            if model.is_finite() {
                let model_minutes = model * delta_minutes as f64;
                let empirical_minutes = empirical_steps * delta_minutes as f64;
                println!(
                    "State {:?}: model dwell = {:.2} steps ({:.1} min), empirical = {:.2} steps ({:.1} min)",
                    state_id, model, model_minutes, empirical_steps, empirical_minutes
                );
            }
        }
    }
    Ok(())
}

pub fn validate_stationary_distribution(counts: &TransitionCounts) -> Result<(), String> {
    if let Some(pi) = counts.stationary_distribution(1000, 1e-10) {
        let sum: f64 = pi.values().sum();
        if (sum - 1.0).abs() > 1e-4 {
            return Err(format!(
                "CTVS-008 FAIL: stationary distribution sums to {}, expected 1.0",
                sum
            ));
        }
        let probs = counts.to_probabilities();
        for (state, &pi_i) in &pi {
            let mut computed = 0.0;
            for (from_state, &pi_from) in &pi {
                if let Some(row) = probs.get(from_state) {
                    computed += pi_from * row.get(state).unwrap_or(&0.0);
                }
            }
            if (computed - pi_i).abs() > 1e-4 {
                return Err(format!(
                    "CTVS-008 FAIL: πP mismatch for state {:?}: computed {}, stored {}",
                    state, computed, pi_i
                ));
            }
        }
        Ok(())
    } else {
        Err("CTVS-008 FAIL: could not compute stationary distribution".to_string())
    }
}

pub fn validate_null_model_reproducibility(
    sequence: &[StateEntry],
    seed: u64,
    n_permutations: usize,
) -> Result<(), String> {
    let first_run =
        crate::ctvs::null_models::shuffled_transition_counts(sequence, seed, n_permutations);
    let second_run =
        crate::ctvs::null_models::shuffled_transition_counts(sequence, seed, n_permutations);
    for (a, b) in first_run.iter().zip(&second_run) {
        if a.counts != b.counts {
            return Err("CTVS-009 FAIL: null model not reproducible with same seed".to_string());
        }
    }
    Ok(())
}

pub fn validate_frozen_boundary(train: &[StateEntry], test: &[StateEntry]) -> Result<(), String> {
    if train.is_empty() || test.is_empty() {
        return Err("CTVS-010 FAIL: train or test is empty".to_string());
    }
    let train_last = train.last().unwrap().timestamp;
    let test_first = test.first().unwrap().timestamp;
    if train_last >= test_first {
        return Err(format!(
            "CTVS-010 FAIL: train/test overlap: train ends at {}, test starts at {}",
            train_last, test_first
        ));
    }
    Ok(())
}

pub fn validate_temporal_stability(
    sequence: &[StateEntry],
    block_size: usize,
    js_threshold: f64,
) -> Result<(), String> {
    if block_size == 0 {
        return Err("CTVS-012 FAIL: block_size must be > 0".to_string());
    }
    let blocks: Vec<&[StateEntry]> = sequence.chunks(block_size).collect();
    if blocks.len() < 2 {
        return Err(format!(
            "CTVS-012 FAIL: need at least 2 blocks, got {}",
            blocks.len()
        ));
    }
    let block_models: Vec<TransitionCounts> = blocks
        .iter()
        .map(|block| TransitionCounts::from_sequence(block))
        .collect();
    let mut max_js = 0.0f64;
    for i in 0..block_models.len() {
        for j in (i + 1)..block_models.len() {
            let js = crate::ctvs::observer::js_divergence(&block_models[i], &block_models[j]);
            println!("JS Divergence between block {} and {}: {:.4}", i, j, js);
            if js > max_js {
                max_js = js;
            }
            if js > js_threshold {
                return Err(format!(
                    "CTVS-012 FAIL: JS divergence between block {} and {} = {:.4} > {}",
                    i, j, js, js_threshold
                ));
            }
        }
    }
    println!(
        "Max JS Divergence: {:.4} (threshold: {:.4})",
        max_js, js_threshold
    );
    Ok(())
}

pub fn validate_null_significance(
    sequence: &[StateEntry],
    seed: u64,
    n_permutations: usize,
    alpha: f64,
) -> Result<(), String> {
    let observed = TransitionCounts::from_sequence(sequence);

    let observed_persistence: BTreeMap<StateId, f64> = observed
        .counts
        .iter()
        .map(|(state, row)| {
            let row_sum: u64 = row.values().sum();
            let p_ii = if row_sum > 0 {
                *row.get(state).unwrap_or(&0) as f64 / row_sum as f64
            } else {
                0.0
            };
            (*state, p_ii)
        })
        .collect();

    let null_counts =
        crate::ctvs::null_models::shuffled_transition_counts(sequence, seed, n_permutations);

    for (state, &obs_p) in &observed_persistence {
        let mut count_exceed = 0usize;
        for null in &null_counts {
            let row_sum: u64 = null
                .counts
                .get(state)
                .map(|r| r.values().sum())
                .unwrap_or(0);
            if row_sum > 0 {
                let null_p = *null.counts[state].get(state).unwrap_or(&0) as f64 / row_sum as f64;
                if null_p >= obs_p {
                    count_exceed += 1;
                }
            }
        }

        let p_value = (count_exceed + 1) as f64 / (n_permutations + 1) as f64;
        println!(
            "State {:?}: observed P_ii = {:.4}, p-value = {:.4}",
            state, obs_p, p_value
        );

        if p_value > alpha {
            return Err(format!(
                "CTVS-013 FAIL: state {:?} persistence not significant (p = {:.4}, alpha = {:.4})",
                state, p_value, alpha
            ));
        }
    }

    Ok(())
}

/// Вычисляет LogLoss для модели первого порядка
pub fn logloss_m1(
    sequence: &[StateEntry],
    start_idx: usize,
    end_idx: usize,
) -> Result<LogLossResult, String> {
    if sequence.len() < 2 {
        return Err("Sequence must contain at least two states".to_string());
    }
    if start_idx >= end_idx || end_idx >= sequence.len() {
        return Err("Invalid indices for logloss_m1".to_string());
    }
    let train = &sequence[..start_idx];
    let counts = TransitionCounts::from_sequence(train);
    let probs = counts.to_probabilities();
    let mut total_logloss = 0.0;
    let mut observations = 0usize;
    let mut zero_probability_events = 0usize;
    for t in start_idx..end_idx {
        let from_state = sequence[t].state;
        let to_state = sequence[t + 1].state;
        let p = probs
            .get(&from_state)
            .and_then(|row| row.get(&to_state))
            .copied()
            .unwrap_or(0.0);
        if p == 0.0 {
            zero_probability_events += 1;
        }
        let p_clipped = p.max(LOGLOSS_EPSILON);
        total_logloss -= p_clipped.ln();
        observations += 1;
    }
    if observations == 0 {
        return Err("No OOS transitions".to_string());
    }
    Ok(LogLossResult {
        mean_logloss: total_logloss / observations as f64,
        observations,
        zero_probability_events,
    })
}

/// CTVS-014: Сравниваем M1 и M2 на OOS данных
pub fn validate_markov_sufficiency(
    sequence: &[StateEntry],
    train_ratio: f64,
    epsilon: f64,
) -> Result<(), String> {
    let n = sequence.len();
    if n < 10 {
        return Err("CTVS-014 FAIL: sequence too short for M1/M2 comparison".to_string());
    }

    let train_end = (n as f64 * train_ratio) as usize;
    if train_end < 5 || train_end > n - 5 {
        return Err("CTVS-014 FAIL: invalid train/test split".to_string());
    }

    let m1_result = logloss_m1(sequence, train_end, n - 1)?;
    let ll_m1 = m1_result.mean_logloss;
    let m1_obs = m1_result.observations;
    let m1_zero = m1_result.zero_probability_events;
    let ll_m2 = logloss_m2(sequence, train_end, n - 1)?;

    println!(
        "LogLoss M1 (OOS): {:.4} (obs={}, zero_prob={})",
        ll_m1, m1_obs, m1_zero
    );
    println!("LogLoss M2 (OOS): {:.4}", ll_m2);
    println!("Difference (M2 - M1): {:.4}", ll_m2 - ll_m1);

    if ll_m2 < ll_m1 - epsilon {
        println!("M2 is significantly better (improvement > {:.4})", epsilon);
        Ok(())
    } else {
        println!("M1 is sufficient (no significant improvement from M2)");
        Ok(())
    }
}

pub fn logloss_m2(
    sequence: &[StateEntry],
    start_idx: usize,
    end_idx: usize,
) -> Result<f64, String> {
    if sequence.len() < 3 {
        return Err("Sequence must contain at least three states for M2".to_string());
    }
    if start_idx >= end_idx || end_idx >= sequence.len() {
        return Err("Invalid indices for logloss_m2".to_string());
    }
    let train = &sequence[..start_idx];
    let counts = SecondOrderTransitionCounts::from_sequence(train);
    let probs = counts.to_probabilities();
    let mut total_logloss = 0.0;
    let mut observations = 0usize;
    for t in start_idx..end_idx {
        if t < 1 {
            continue;
        }
        let s_prev = sequence[t - 1].state;
        let s_curr = sequence[t].state;
        let s_next = sequence[t + 1].state;
        let context = (s_prev, s_curr);
        let p = probs
            .get(&context)
            .and_then(|row| row.get(&s_next))
            .copied()
            .unwrap_or(0.0);
        let p_clipped = p.max(LOGLOSS_EPSILON);
        total_logloss -= p_clipped.ln();
        observations += 1;
    }
    if observations == 0 {
        return Err("No OOS transitions for M2".to_string());
    }
    Ok(total_logloss / observations as f64)
}

/// Статистики для одного состояния
#[derive(Debug, Clone)]
pub struct PredictiveStats {
    #[allow(dead_code)]
    pub state: StateId,
    pub n: usize,
    pub mean: f64,
    pub variance: f64,
}

/// Результат predictive validation
#[derive(Debug, Clone)]
pub struct PredictiveValidation {
    pub observations: usize,
    pub states_tested: usize,
    pub f_statistic: Option<f64>,
    pub p_value: Option<f64>,
    pub effect_size: Option<f64>,
    pub status: ValidationStatus,
}

/// CTVS-015A: Validator correctness — проверяет, что validator
/// обнаруживает заранее известное различие между состояниями.
pub fn validate_predictive_separation(
    sequence: &[StateEntry],
    min_obs_per_state: usize,
    alpha: f64,
) -> PredictiveValidation {
    // Группируем return_forward по состояниям
    let mut returns_by_state: BTreeMap<StateId, Vec<f64>> = BTreeMap::new();

    for entry in sequence {
        if let Some(ret) = entry.return_forward {
            returns_by_state.entry(entry.state).or_default().push(ret);
        }
    }

    // Проверяем минимальное число наблюдений
    let states_tested = returns_by_state.len();
    let total_obs: usize = returns_by_state.values().map(|v| v.len()).sum();

    if total_obs == 0 || states_tested < 2 {
        return PredictiveValidation {
            observations: total_obs,
            states_tested,
            f_statistic: None,
            p_value: None,
            effect_size: None,
            status: ValidationStatus::NotEstimable,
        };
    }

    // Проверяем, что каждое состояние имеет достаточно наблюдений
    for (state, rets) in &returns_by_state {
        if rets.len() < min_obs_per_state {
            println!(
                "State {:?} has {} observations (need {})",
                state,
                rets.len(),
                min_obs_per_state
            );
            return PredictiveValidation {
                observations: total_obs,
                states_tested,
                f_statistic: None,
                p_value: None,
                effect_size: None,
                status: ValidationStatus::NotEstimable,
            };
        }
    }

    // Вычисляем статистики
    let mut stats: Vec<PredictiveStats> = Vec::new();
    for (state, rets) in &returns_by_state {
        let n = rets.len();
        let mean = rets.iter().sum::<f64>() / n as f64;
        let variance = if n > 1 {
            rets.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        stats.push(PredictiveStats {
            state: *state,
            n,
            mean,
            variance,
        });
    }

    // Упрощённый ANOVA (one-way)
    let grand_mean = returns_by_state.values().flatten().sum::<f64>() / total_obs as f64;

    let ss_between: f64 = stats
        .iter()
        .map(|s| s.n as f64 * (s.mean - grand_mean).powi(2))
        .sum();
    let ss_within: f64 = stats.iter().map(|s| (s.n as f64 - 1.0) * s.variance).sum();

    let df_between = (states_tested - 1) as f64;
    let df_within = (total_obs - states_tested) as f64;

    if df_within <= 0.0 || ss_within == 0.0 {
        return PredictiveValidation {
            observations: total_obs,
            states_tested,
            f_statistic: None,
            p_value: None,
            effect_size: None,
            status: ValidationStatus::Inconclusive,
        };
    }

    let ms_between = ss_between / df_between;
    let ms_within = ss_within / df_within;
    let f_statistic = ms_between / ms_within;

    // Эффект размера: eta-squared
    let eta_squared = ss_between / (ss_between + ss_within);

    // Аппроксимация p-value через неполную бета-функцию
    let p_value = f_distribution_p_value(f_statistic, df_between, df_within);

    let status = if p_value < alpha {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    };

    PredictiveValidation {
        observations: total_obs,
        states_tested,
        f_statistic: Some(f_statistic),
        p_value: Some(p_value),
        effect_size: Some(eta_squared),
        status,
    }
}

/// Аппроксимация p-value для F-распределения
fn f_distribution_p_value(f: f64, df1: f64, df2: f64) -> f64 {
    // Abramowitz & Stegun 26.6.2
    let x = df2 / (df2 + df1 * f);
    let p = regularized_incomplete_beta(df2 / 2.0, df1 / 2.0, x);
    p
}

/// Регуляризованная неполная бета-функция (Continued fraction)
fn regularized_incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    let ln_beta = ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b);
    let front = (x.powf(a) * (1.0 - x).powf(b) / a).exp() * (ln_beta).exp();

    // Lentz's algorithm for continued fraction
    let max_iter = 500;
    let eps = 1e-15;
    let mut f: f64 = 1.0;
    let mut c: f64 = 1.0;
    let mut d: f64 = 0.0;

    for i in 0..max_iter {
        let m = (i / 2) as f64;
        let numerator = if i % 2 == 0 {
            m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m))
        } else {
            -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0))
        };

        d = 1.0 + numerator * d;
        if d.abs() < eps {
            d = eps;
        }
        d = 1.0 / d;

        c = 1.0 + numerator / c;
        if c.abs() < eps {
            c = eps;
        }

        f *= d * c;

        if (d * c - 1.0).abs() < eps {
            break;
        }
    }

    front * f
}

/// Логарифм гамма-функции (Lanczos approximation)
fn ln_gamma(z: f64) -> f64 {
    let g = 7.0;
    let coefficients = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];

    if z < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * z).sin()).ln() - ln_gamma(1.0 - z);
    }

    let z = z - 1.0;
    let mut x = coefficients[0];
    for i in 1..coefficients.len() {
        x += coefficients[i] / (z + i as f64);
    }
    let t = z + g + 0.5;
    let sqrt_2pi = (2.0 * std::f64::consts::PI).sqrt();
    return sqrt_2pi.ln() + (z + 0.5) * t.ln() - t + x.ln();
}

