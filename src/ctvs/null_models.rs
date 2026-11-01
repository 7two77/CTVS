use super::observer::{StateEntry, TransitionCounts};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

pub fn iid_shuffle(sequence: &[StateEntry], seed: u64) -> Vec<StateEntry> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut shuffled = sequence.to_vec();
    shuffled.shuffle(&mut rng);
    shuffled
}

pub fn shuffled_transition_counts(
    original: &[StateEntry],
    seed: u64,
    n_permutations: usize,
) -> Vec<TransitionCounts> {
    let mut results = Vec::with_capacity(n_permutations);
    for i in 0..n_permutations {
        let shuffled = iid_shuffle(original, seed + i as u64);
        results.push(TransitionCounts::from_sequence(&shuffled));
    }
    results
}

