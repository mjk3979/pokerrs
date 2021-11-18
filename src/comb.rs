#[derive(Clone)]
pub struct Combinations<T, It> {
    p: usize,
    stack: Vec<It>,
    combo: Vec<T>,
}

pub fn combinations<T: Clone, I, It: Iterator<Item=T> + Clone>(src: I, p: usize) -> Combinations<T, It>
    where I: IntoIterator<Item = T, IntoIter=It>
{
    assert!(p > 0);
    let stack: Vec<_> = vec![src.into_iter()];
    let combo = Vec::new();
    Combinations {p, stack, combo}
}

impl<T: Clone, It: Iterator<Item=T> + Clone> Iterator for Combinations<T, It> {
    type Item = Vec<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.combo.len() == self.p {
            self.combo.pop();
        }
        while let Some(mut iter) = self.stack.pop() {
            if let Some(v) = iter.next() {
                self.combo.push(v);
                if self.combo.len() == self.p {
                    self.stack.push(iter);
                    return Some(self.combo.clone());
                } else {
                    self.stack.push(iter.clone());
                    self.stack.push(iter);
                }
            } else {
                if !self.combo.is_empty() {
                    self.combo.pop();
                }
            }
        }
        None
    }
}

mod test {

use crate::frozen::FrozenSet;
use crate::comb::combinations;

#[test]
fn test_combinations_empty() {
    let empty: Vec<i64> = Vec::new();
    for p in 1..5 {
        let result: Vec<_> = combinations(&empty, p).collect();
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
