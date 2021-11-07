
pub fn combinations<'a, T>(src: &'a [T], p: usize) -> Vec<Vec<&'a T>> {
    assert!(src.len() < 63);
    assert!(p > 0);
    let mut retval = Vec::new();
    for mask in 0usize..(1 << src.len()) {
        let mut attempt = Vec::new();
        for (idx, e) in src.iter().enumerate() {
            if (mask & (1 << idx)) != 0 {
                attempt.push(e);
            }
        }
        if attempt.len() == p {
            retval.push(attempt);
        }
    }
    retval
}

mod test {

use crate::frozen::FrozenSet;
use crate::comb::combinations;

#[test]
fn test_combinations_empty() {
    let empty: Vec<i64> = Vec::new();
    for p in 1..5 {
        let result = combinations(&empty, p);
        assert!(result.is_empty(), format!("Combination of empty set not empty: {:?}", result));
    }
}

#[test]
fn test_combinations_small() {
    let lst: Vec<u64> = vec![2, 3, 5, 7, 11];
    let combos: FrozenSet<FrozenSet<u64>>  = combinations(&lst, 2).into_iter().map(|v| v.into_iter().copied().collect()).collect();
    let expected: FrozenSet<FrozenSet<u64>> = vec![
        vec![2, 3],
        vec![2, 5],
        vec![2, 7],
        vec![2, 11],
        vec![3, 5],
        vec![3, 7],
        vec![3, 11],
        vec![5, 7],
        vec![5, 11],
        vec![7,11]
    ].into_iter().map(|v| v.into_iter().collect()).collect();
    assert!(combos == expected);
}

}
