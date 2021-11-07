use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct FrozenSet<T>
where T: Hash + Eq
{
    underlying: HashSet<T>,
    hsh: u64
}

impl<T> FromIterator<T> for FrozenSet<T>
where T: Hash + Eq
{
    fn from_iter<I:IntoIterator<Item=T>> (iter: I) -> FrozenSet<T> {
        let underlying: HashSet<T> = iter.into_iter().collect();
        let mut hashes: Vec<u64> = underlying.iter().map(|ele| {
            let mut hasher = DefaultHasher::new();
            ele.hash(&mut hasher);
            hasher.finish()
        }).collect();
        hashes.sort();

        let mut hasher = DefaultHasher::new();
        for hsh in hashes {
            hsh.hash(&mut hasher);
        }
        FrozenSet {
            underlying,
            hsh: hasher.finish()
        }
    }
}

impl<T> Hash for FrozenSet<T>
where T: Hash + Eq {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hsh.hash(h);
    }
}
