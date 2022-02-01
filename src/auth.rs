use crate::game::*;

use std::collections::HashMap;
use rand::prelude::*;

pub trait AuthMap<K, V> {
    fn insert(&mut self, v: V) -> K;
    fn get(&self, key: &K) -> Option<&V>;
    fn get_mut(&mut self, key: &K) -> Option<&mut V>;
}

pub type RandomToken = Vec<u8>;

pub struct RandomTokenAuthMap<V> {
    key_len: usize,
    map: HashMap<RandomToken, V>,
}

impl<V> RandomTokenAuthMap<V> {
    pub fn new(key_len: usize) -> RandomTokenAuthMap<V> {
        assert!(key_len % 8 == 0);
        RandomTokenAuthMap {
            key_len,
            map: HashMap::new(),
        }
    }

    fn generate_token<R: Rng+CryptoRng>(&self, rng: &mut R) -> RandomToken {
        (0..(self.key_len / 8)).map(|_| rng.gen()).collect()
    }

    fn new_token(&self) -> RandomToken {
        let mut rng = rand::thread_rng();
        self.generate_token(&mut rng)
    }
}

impl<V> AuthMap<RandomToken, V> for RandomTokenAuthMap<V> {
    fn insert(&mut self, v: V) -> RandomToken {
        let token = self.new_token();
        let res = self.map.insert(token.clone(), v);
        assert!(res.is_none(), "Duplicate token generated {:?}: {:?}", token, self.map.keys());
        token
    }

    fn get(&self, key: &RandomToken) -> Option<&V> {
        self.map.get(key)
    }

    fn get_mut(&mut self, key: &RandomToken) -> Option<&mut V> {
        self.map.get_mut(key)
    }
}

mod test {
    use crate::auth::*;

    #[test]
    fn test_random_token_map() {
        let mut map = RandomTokenAuthMap::new(512);
        let player1 = map.insert(Vec::new());
        let player2 = map.insert(Vec::new());
        map.get_mut(&player1).unwrap().push("player1");
        map.get_mut(&player2).unwrap().push("player2");
        map.get_mut(&player1).unwrap().push("bob");
        map.get_mut(&player2).unwrap().push("alice");
        assert!(map.get(&player1).unwrap() == &vec!["player1", "bob"]);
        assert!(map.get(&player2).unwrap() == &vec!["player2", "alice"]);
    }

    #[test]
    #[ignore]
    fn test_random_token_map_large() {
        let mut map = RandomTokenAuthMap::new(512);
        let mut players = Vec::new();
        const NUM_PLAYERS: usize = 1_000_000;
        for pidx in 0..NUM_PLAYERS {
            let new_player = map.insert(vec![format!("player{}", pidx)]);
            players.push(new_player);
        }
        for (pidx, token) in players.into_iter().enumerate() {
            assert!(map.get(&token).unwrap() == &vec![format!("player{}", pidx)]);
        }
    }
}
