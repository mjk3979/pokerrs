use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use crate::card::*;
use crate::game::*;

use rand::prelude::*;

pub trait Crypto {
    fn encrypt(&self, plaintext: &[u8]) -> Vec<u8>;
    fn decrypt(&self, ciphertext: &[u8]) -> Vec<u8>;
}

#[derive(Clone, Debug)]
pub struct Encrypted {
    ciphertext: Vec<u8>
}

#[derive(Debug)]
pub struct Decrypted {
    plaintext: Vec<u8>
}

impl Decrypted {
    fn as_t<'a, T: Deserialize<'a>>(&'a self) -> T {
        serde_json::from_slice::<T>(&self.plaintext).unwrap()
    }
}

impl Encrypted {
    fn encrypt<C: Crypto, T: Serialize>(plaintext: &T, c: &C) -> Encrypted {
        let json = serde_json::to_vec(plaintext).unwrap();
        let ciphertext = c.encrypt(&json);
        Encrypted {
            ciphertext
        }
    }

    fn decrypt<C: Crypto>(&self, c: &C) -> Decrypted {
        Decrypted {
            plaintext: c.decrypt(&self.ciphertext)
        }
    }
}

pub trait DeckDealer {
    fn request_deck(&self) -> Vec<Encrypted>;
    fn decrypt_card(&mut self, input: &Encrypted, idx: usize) -> Decrypted;
}

pub struct VerifiedDeckDealer<C: Crypto> {
    cards: Vec<Encrypted>,
    crypto: C
}

impl<C: Crypto> DeckDealer for VerifiedDeckDealer<C> {
    fn request_deck(&self) -> Vec<Encrypted> {
        self.cards.clone()
    }

    fn decrypt_card(&mut self, input: &Encrypted, idx: usize) -> Decrypted {
        self.cards.remove(idx);
        input.decrypt(&self.crypto)
    }
}

pub struct VerifiedDeckClient<'a, R: Rng, C: Crypto> {
    dealer: Mutex<Box<dyn DeckDealer>>,
    crypto: C,
    rng: &'a mut R
}

impl<'a, R: Rng, C: Crypto> Deck for VerifiedDeckClient<'a, R, C> {
    fn draw(&mut self) -> Result<Card, PokerRoundError> {
        let mut d = self.dealer.lock().unwrap();
        let deck = d.request_deck();
        let (idx, picked) = deck.into_iter().enumerate().choose(self.rng).unwrap();
        let enc_picked = Encrypted::encrypt(&picked.ciphertext, &self.crypto);
        let dec_picked: Vec<u8> = d.decrypt_card(&enc_picked, idx).as_t();
        Ok(serde_json::from_slice(&self.crypto.decrypt(&dec_picked)).unwrap())
    }
}
