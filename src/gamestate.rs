use crate::card::*;
use crate::game::*;
use crate::table::*;
use crate::comb::*;
use crate::viewstate::*;
use crate::fold_channel;

use tokio::sync::broadcast;
use tokio::sync::oneshot;

use ts_rs::{TS, export};

use async_trait::async_trait;

use std::collections::{HashMap, BTreeMap, HashSet};
use std::convert::TryInto;
use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use core::future::Future;
use core::pin::Pin;
use std::hash::Hash;

pub enum GameState {
    Start,
    Playing(HandState),
    End(Winners<PlayerId>)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RoundState {
    Ante,
    Bet {
        player: PlayerRole,
        last_bet: Option<(PlayerRole, Chips)>,
        all_bets: HashMap<PlayerRole, Chips>
    },
    DrawToHand {
        facing: Vec<Facing>
    },
    DrawToCommunity {
        quant: usize
    }
}

impl RoundState {
    pub fn new(round: &Round) -> Self {
        match round {
            Round::Ante => RoundState::Ante,
            Round::Bet{starting_player} => RoundState::Bet {
                player: *starting_player,
                last_bet: None,
                all_bets: HashMap::new()
            },
            Round::DrawToHand{facing} => RoundState::DrawToHand{facing: facing.clone()},
            Round::DrawToCommunity{quant} => RoundState::DrawToCommunity{quant: *quant}
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct CardState {
    pub card: Card,
    pub facing: Facing
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerState {
    pub chips: Chips,
    pub hand: Vec<CardState>,
    pub folded: bool,
    pub total_bet: Chips
}

type PlayersState = HashMap<PlayerRole, PlayerState>;

pub struct HandState {
    pub deck: Mutex<Box<dyn Deck + Send>>,
    pub rounds: Vec<Round>,
    pub cur_round: Option<RoundState>,
    pub players: HashMap<PlayerRole, PlayerState>,
    pub community_cards: Vec<Card>
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[derive(TS)]
pub struct HandStrength {
    pub kind: Kind,
    pub kickers: Vec<StrengthRank>
}

impl HandStrength {
    fn new(cards: [Card; 5]) -> Self {
        // Reorganize to make calculating easy
        let mut ranks: Vec<usize> = cards.iter().map(|c| c.rank).collect();
        ranks.sort();
        let is_straight = (1..5).all(|i| ranks[0] + i == ranks[i]);
        let is_flush = (1..5).all(|i| cards[0].suit == cards[i].suit);

        let mut by_rank: Vec<usize> = vec![0; NUM_RANKS+1];
        for &rank in ranks.iter() {
            by_rank[rank] += 1;
        }
        let mut by_amount: Vec<Vec<usize>> = vec![Vec::new(); NUM_SUITS];
        for (rank, &amount) in by_rank.iter().enumerate() {
            if amount > 0 {
                by_amount[amount-1].push(rank);
            }
        }
        
        // Go through cases best-to-worst
        use Kind::*;
        if is_straight && is_flush {
            return HandStrength {
                kind: StraightFlush(*ranks.last().unwrap()),
                kickers: Vec::new()
            };
        }
        if let Some(&four_kind_rank) = by_amount[3].first() {
            return HandStrength {
                kind: FourKind(four_kind_rank),
                kickers: by_amount[0].clone()
            };
        }
        if let Some(&children) = by_amount[2].first() {
            if let Some(&parents) = by_amount[1].first() {
                return HandStrength {
                    kind: FullHouse{
                        high: children,
                        low: parents,
                    },
                    kickers: Vec::new()
                };
            }
        }
        if is_flush {
            return HandStrength {
                kind: Flush(ranks.into_iter().rev().collect::<Vec<_>>().try_into().unwrap()),
                kickers: Vec::new()
            };
        }
        if is_straight {
            return HandStrength {
                kind: Straight(*ranks.last().unwrap()),
                kickers: Vec::new()
            };
        }
        let kickers = by_amount[0].iter().copied().rev().collect();
        if let Some(&three_kind_rank) = by_amount[2].first() {
            return HandStrength {
                kind: ThreeKind(three_kind_rank),
                kickers
            }
        }
        if let Some(&low_pair) = by_amount[1].first() {
            if let Some(&high_pair) = by_amount[1].iter().nth(1) {
                return HandStrength {
                    kind: TwoPair{
                        high: high_pair,
                        low: low_pair
                    },
                    kickers
                };
            } else {
                return HandStrength {
                    kind: Pair(low_pair),
                    kickers
                };
            }
        }
        return HandStrength {
            kind: HighCard(*ranks.last().unwrap()),
            kickers: (&ranks[0..4]).iter().rev().copied().collect()
        };
    }
}

impl std::fmt::Display for HandStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

fn explode_aces(cards: &[Card]) -> Vec<Card> {
    let mut retval = Vec::new();
    for &card in cards {
        retval.push(card);
        if card.rank == 0 {
            retval.push(Card {
                rank: NUM_RANKS,
                suit: card.suit
            });
        }
    }
    retval
}

pub fn best_hand(cards: &[Card]) -> HandStrength {
    let all_cards = explode_aces(cards);
    let real_ace_count = all_cards.len() - cards.len();
    combinations(&all_cards, 5).into_iter().filter_map(|v|{
        let combo = v.into_iter().copied().collect::<Vec<_>>();
        let mut ace_count = 0;
        for &card in &combo {
            if card.rank == 0 || card.rank == NUM_RANKS {
                ace_count += 1;
            }
        }
        if ace_count > real_ace_count {
            None
        } else {
            Some(HandStrength::new(combo.try_into().unwrap()))
        }
    }).max().unwrap()
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct Subpot<P> {
    pub chips: Chips,
    pub players: Vec<P>
}

fn calc_subpots(state: &HandState) -> Vec<Subpot<PlayerRole>> {
    let mut bets: BTreeMap<Chips, (i64, Vec<PlayerRole>)> = BTreeMap::new();
    let mut players_involved = HashSet::new();
    let mut players_bet = 0;
    for (&role, player) in &state.players {
        let entry = bets.entry(player.total_bet).or_insert_with(|| (0, Vec::new()));
        entry.0 += 1;
        players_bet += 1;
        if !player.folded {
            entry.1.push(role);
            players_involved.insert(role);
        }
    }
    let mut pot_acc = 0;
    let mut last = 0;
    let mut retval = Vec::new();
    for (bet, (num_bet, players)) in bets {
        pot_acc += (bet - last) * players_bet;
        last = bet;
        players_bet -= num_bet;
        if !players.is_empty() {
            let mut in_pot: Vec<PlayerRole> = players_involved.iter().copied().collect();
            in_pot.sort();
            retval.push(Subpot {
                chips: pot_acc,
                players: in_pot
            });
            pot_acc = 0;
            for player in players {
                players_involved.remove(&player);
            }
        }
    }
    retval
}

fn split_pot<P: Clone>(players: &[P], pot: Chips) -> Vec<(P, Chips)> {
    let num_players = players.len() as Chips;
    let even = pot / num_players;
    let mut left = pot % num_players;
    let mut retval = Vec::new();
    for player in players {
        let mut cut = even;
        if left > 0 {
            cut += 1;
            left -= 1;
        }
        retval.push((player.clone(), cut));
    }
    retval
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct Winners<P> {
    pub winners_by_pot: Vec<(Subpot<P>, Vec<P>)>
}

impl<P: Clone + Eq + Hash> Winners<P> {
    pub fn totals(&self) -> HashMap<P, Chips> {
        let mut retval = HashMap::new();
        for (Subpot{chips: pot, ..}, winners) in &self.winners_by_pot {
            for (player, cut) in split_pot(&winners, *pot) {
                *retval.entry(player).or_insert(0) += cut;
            }
        }
        retval
    }

    pub fn convert<T: Clone>(&self, mapping: &HashMap<P, T>) -> Winners<T> {
        let winners_by_pot = self.winners_by_pot.iter().map(|(p, l)| (p.convert(mapping), l.iter().map(|p| mapping.get(p).cloned().unwrap()).collect())).collect();
        Winners {
            winners_by_pot
        }
    }
}

impl<P: Clone + Eq + Hash> Subpot<P> {
    pub fn convert<T: Clone>(&self, mapping: &HashMap<P, T>) -> Subpot<T> {
        Subpot {
            chips: self.chips,
            players: self.players.iter().map(|p| mapping.get(p).cloned().unwrap()).collect(),
        }
    }
}

fn calc_winners(state: &HandState) -> Winners<PlayerRole> {
    // calculate best hands for each player
    let best_hands: HashMap<PlayerRole, HandStrength> = state.players.iter().filter_map(|(&role, player)| {
        if player.folded {
            return None;
        }
        Some((role, {
            let mut all_cards = state.community_cards.clone();
            for &cs in player.hand.iter() {
                all_cards.push(cs.card);
            }
            best_hand(&all_cards)
        }))

    }).collect();
    // figure out subpots
    let subpots = calc_subpots(state);
    // foreach subpot, split amongst top hands for players in the subpot
    let mut retval = Vec::new();
    for Subpot{chips: pot, players} in subpots {
        let mut piter = players.iter().copied();
        let mut winners = vec![piter.next().unwrap()];
        let mut best = best_hands.get(winners.first().unwrap()).unwrap();
        for player in piter {
            let hand = best_hands.get(&player).unwrap();
            if hand > best {
                winners = vec![player];
                best = hand;
            } else if hand == best {
                winners.push(player);
            }
        }
        retval.push((Subpot{chips: pot, players}, winners));
    }
    Winners {
        winners_by_pot: retval
    }
}

fn next_player(current_role: PlayerRole, num_players: usize) -> PlayerRole {
    let mut new_player = current_role+1;
    if new_player >= num_players {
        return 0;
    }
    return new_player;
}

fn collect_ante_from_players(rule: &AnteRule, players: &mut HashMap<PlayerRole, PlayerState>, viewdiffs: &mut Vec<PokerGlobalViewDiff<PlayerRole>>) {
    use AnteRule::*;
    match rule {
        Ante(ante) => {
            for (role, mut player) in players.iter_mut() {
                let chips = player.chips;
                let to_collect = std::cmp::min(chips, *ante);
                player.total_bet += to_collect;
                viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::from_blind_name(to_collect, *role, "ante".to_string())));
            }
        }
        Blinds(blinds) => {
            for &Blind{player: blind_role, amount} in blinds {
                let player = players.get_mut(&blind_role).unwrap();
                let chips = player.chips;
                let to_collect = std::cmp::min(chips, amount);
                player.total_bet += to_collect;
                viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::from_blind_name(to_collect, blind_role, "blind".to_string())));
            }
        }
    }
}

fn collect_bets(players: &mut PlayersState, bets: &HashMap<PlayerRole, Chips>) {
    for (role, &bet) in bets {
        players.get_mut(role).unwrap().total_bet += bet;
    }
}

fn update_players<'a, 'b, 'c, 'd, 'e>(players: &'b HashMap<PlayerRole, LivePlayer>, ids: &'e HashMap<PlayerRole, PlayerId>, spectator_channel: &'c Option<fold_channel::Sender<Vec<PokerGlobalViewDiff<PlayerId>>, Vec<PokerGlobalViewDiff<PlayerId>>>>, state: &'b HandState, role_viewdiffs: &'b [PokerGlobalViewDiff<PlayerRole>], round: usize) {
    if role_viewdiffs.is_empty() {
        return;
    }
    let viewdiffs: Vec<PokerGlobalViewDiff<PlayerId>> = role_viewdiffs.iter().map(|l| l.convert(ids)).collect();
    for (&role, player) in players.iter() {
        player.input.lock().unwrap().update(PokerViewUpdate {
            viewstate: PokerViewState::from_handstate_and_player(&state, role),
            diff: vec![PokerLogUpdate {
                round,
                log: viewdiffs.iter().map(|viewdiff| viewdiff.player_diff(Some(&player.player_id))).collect()
            }],
        });
    }
    let diffs: Vec<PokerGlobalViewDiff<PlayerId>> = viewdiffs.iter().cloned().collect();
    //println!("Sending log {}", diffs.len());
    if let Some(channel) = spectator_channel {
        channel.send(diffs);
    }
}

pub fn show_cards(players: &mut HashMap<PlayerRole, PlayerState>, viewdiffs: &mut Vec<PokerGlobalViewDiff<PlayerRole>>, last_bet: Option<PlayerRole>, community_cards: &[Card]) {
    // last best and then to the left
    let starting = last_bet.unwrap_or(0); // todo who actually goes first?
    let num_players = players.len();
    let mut role = starting;
    loop {
        let player = players.get_mut(&role).unwrap();
        let mut shown = Vec::new();
        for (idx, cs) in player.hand.iter_mut().enumerate() {
            if Facing::FaceDown == cs.facing {
                cs.facing = Facing::FaceUp;
                shown.push((idx, CardViewState::Visible(cs.clone())));
            }
        }
        if !shown.is_empty() {
            let mut all_cards: Vec<_> = community_cards.iter().cloned().collect();
            for &cs in player.hand.iter() {
                all_cards.push(cs.card);
            }
            let strength = best_hand(&all_cards);
            viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::ShowCards {
                player: role,
                shown,
                strength
            }));
        }
        // end
        role = next_player(role, num_players);
        if role == starting {
            break;
        }
    }
}

pub async fn play_poker<'a>(variant: PokerVariant,
    starting_deck: Mutex<Box<dyn Deck + Send>>,
    players: HashMap<PlayerRole, LivePlayer>,
    spectator_channel: Option<fold_channel::Sender<Vec<PokerGlobalViewDiff<PlayerId>>, Vec<PokerGlobalViewDiff<PlayerId>>>>,
    table_rules: TableRules,
    round: usize
    ) ->
    Result<HashMap<PlayerRole, Chips>, PokerRoundError> {
    let mut state = HandState {
        deck: starting_deck,
        rounds: variant.iter().cloned().rev().collect(),
        cur_round: None,
        players: players.iter().map(|(&e, p)| (e, PlayerState{chips: p.chips, hand: Vec::new(), folded: false, total_bet: 0})).collect(),
        community_cards: Vec::new()
    };

    let num_players = players.len();
    let mut viewdiffs: Vec<PokerGlobalViewDiff<PlayerRole>> = Vec::new();
    let mut hand_last_bet: Option<PlayerRole> = None;
    let ids = players.iter().map(|(role, p)| (*role, p.player_id.clone())).collect();

    loop {
        //println!("{:#?}", state);
        update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, round);
        viewdiffs.clear();
        match state.cur_round {
            None => {
                if let Some(next_round) = state.rounds.pop() {
                    state.cur_round = Some(RoundState::new(&next_round));
                } else {
                    show_cards(&mut state.players, &mut viewdiffs, hand_last_bet, &state.community_cards);
                    update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, round);
                    viewdiffs.clear();
                    let winners = calc_winners(&state);
                    let mut retval = winners.totals();
                    for (&role, player) in &state.players {
                        *retval.entry(role).or_insert(0) -= player.total_bet;
                    }
                    viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::Winners(winners)));
                    update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, round);
                    viewdiffs.clear();
                    return Ok(retval);
                }
            },
            Some(round_state) => {
                use RoundState::*;
                match round_state {
                    Ante => {
                        collect_ante_from_players(&table_rules.ante, &mut state.players, &mut viewdiffs);
                        state.cur_round = None;
                    },
                    DrawToHand{facing} => {
                        for facing in facing {
                            for (role, player) in state.players.iter_mut() {
                                if !player.folded {
                                    let newcard = CardState {
                                        card: state.deck.lock().unwrap().draw()?,
                                        facing
                                    };
                                    viewdiffs.push(PokerGlobalViewDiff::Draw{
                                        player: *role,
                                        drawn: vec![newcard.clone()]
                                    });
                                    player.hand.push(newcard);
                                }
                            }
                        }
                        state.cur_round = None;
                    },
                    DrawToCommunity{quant} => {
                        let mut newcards: Vec<Card> = Vec::new();
                        for _ in 0..quant {
                            newcards.push(state.deck.lock().unwrap().draw()?);
                        }
                        for &card in &newcards {
                            state.community_cards.push(card);
                        }
                        viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::CommunityDraw {
                            drawn: newcards.into_iter().map(|c| CardViewState::Visible(CardState {
                                card: c,
                                facing: Facing::FaceUp
                            })).collect()
                        }));
                        state.cur_round = None;
                    }
                    Bet{player: bet_role, last_bet, mut all_bets} => {
                        let (last_bet_amount, min_bet) = if let Some((last_bet_player, last_bet_amount)) = last_bet {
                            if last_bet_player == bet_role {
                                collect_bets(&mut state.players, &all_bets);
                                state.cur_round = None;
                                continue;
                            }
                            if last_bet_amount == 0 {
                                assert!(table_rules.min_bet > 0);
                                (0, table_rules.min_bet)
                            } else {
                                (last_bet_amount, last_bet_amount * 2)
                            }
                        } else {
                            assert!(table_rules.min_bet > 0);
                            (0, table_rules.min_bet)
                        };
                        assert!(min_bet > 0);
                        println!("last_bet: {}", last_bet_amount);
                        if state.players.iter().filter(|(role, player)| {
                            player.chips - player.total_bet != *all_bets.get(&bet_role).unwrap_or(&0) && !player.folded
                        }).count() <= 1 {
                            collect_bets(&mut state.players, &all_bets);
                            state.cur_round = None;
                            continue;
                        }
                        let player = state.players.get_mut(&bet_role).unwrap();
                        let mut this_bet = None;
                        if !player.folded && player.chips > *all_bets.get(&bet_role).unwrap_or(&0) {
                            //println!("Waiting on {}", bet_role);
                            let (tx, rx) = oneshot::channel();
                            players.get(&bet_role).unwrap().input.lock().unwrap().bet(last_bet_amount, min_bet, tx);
                            match rx.await.unwrap() {
                                BetResp::Bet(num_chips) => {
                                    //assert!(num_chips == 0 || num_chips == last_bet_amount || num_chips >= min_bet);
                                    if last_bet.is_none() || num_chips > last_bet_amount {
                                        this_bet = Some((bet_role, num_chips));
                                    }
                                    viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::from_player_bet_min_bet(bet_role, num_chips, *all_bets.get(&bet_role).unwrap_or(&0), last_bet_amount)));
                                    *all_bets.entry(bet_role).or_insert(0) = num_chips;
                                },
                                BetResp::Fold => {
                                    player.folded = true;
                                    viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::Fold {
                                        player: bet_role
                                    }));
                                }
                            }
                        }
                        state.cur_round = Some(Bet{
                            player: next_player(bet_role, num_players),
                            last_bet: this_bet.or(last_bet),
                            all_bets
                        });
                    }
                }
            }
        }
    }
}

mod test {
    use crate::gamestate::*;
    use std::hash::Hash;
    use std::fmt::Display;

    #[test]
    fn test_explode_aces_do_nothing() {
        let cards: Vec<Card> = vec![1, 2, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let result = explode_aces(&cards);
        assert!(result == cards, format!("{:?} != {:?}", result, cards));
    }

    #[test]
    fn test_explode_aces_single_ace() {
        let cards: Vec<Card> = vec![1, 0, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(2, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_explode_aces_single_ace_first() {
        let cards: Vec<Card> = vec![0, 1, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(1, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_explode_aces_single_ace_last() {
        let cards: Vec<Card> = vec![2, 1, 3, NUM_RANKS-1, 0].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(5, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_explode_aces_two_aces() {
        let cards: Vec<Card> = vec![0, 1, 0, NUM_RANKS-1, 7].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(3, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.insert(1, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_explode_aces_two_aces_adjacent() {
        let cards: Vec<Card> = vec![0, 0, 2, NUM_RANKS-1, 7].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(2, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.insert(1, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_explode_aces_four_aces() {
        let cards: Vec<Card> = vec![0, 0, 0, NUM_RANKS-1, 0].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.insert(5, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.insert(3, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.insert(2, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.insert(1, Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(&cards);
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_high_card() {
        let cards: Vec<Card> = vec![4, 2, 7, 5, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::HighCard(9),
            kickers: vec![7, 5, 4, 2]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }
    
    #[test]
    fn test_best_hand_high_card_ace() {
        let cards: Vec<Card> = vec![4, 2, 0, 5, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::HighCard(NUM_RANKS),
            kickers: vec![9, 5, 4, 2]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_pair() {
        let cards: Vec<Card> = vec![4, 2, 1, 4, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::Pair(4),
            kickers: vec![9, 2, 1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_pair_ace_not_in_pair() {
        let cards: Vec<Card> = vec![4, 0, 1, 4, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::Pair(4),
            kickers: vec![NUM_RANKS, 9, 1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_pair_ace_in_pair() {
        let cards: Vec<Card> = vec![0, 2, 1, 4, 0].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::Pair(NUM_RANKS),
            kickers: vec![4, 2, 1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_two_pair() {
        let cards: Vec<Card> = vec![4, 2, 1, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: 4, low: 2},
            kickers: vec![1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_two_pair_ace_not_in_two_pair() {
        let cards: Vec<Card> = vec![4, 2, 0, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: 4, low: 2},
            kickers: vec![NUM_RANKS]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_two_pair_ace_in_two_pair() {
        let cards: Vec<Card> = vec![4, 0, 1, 4, 0].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: NUM_RANKS, low: 4},
            kickers: vec![1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_three_kind() {
        let cards: Vec<Card> = vec![4, 4, 1, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::ThreeKind(4),
            kickers: vec![2, 1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_three_kind_ace_not_in_three_kind() {
        let cards: Vec<Card> = vec![4, 4, 0, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::ThreeKind(4),
            kickers: vec![NUM_RANKS, 2]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_best_hand_three_kind_ace_in_three_kind() {
        let cards: Vec<Card> = vec![0, 0, 1, 4, 0].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(&cards);
        let expected = HandStrength{
            kind: Kind::ThreeKind(NUM_RANKS),
            kickers: vec![4, 1]
        };
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    fn make_test_calc_winners_state(players: HashMap<PlayerRole, PlayerState>) -> HandState {
        HandState {
            deck: Mutex::new(Box::new(standard_deck())),
            rounds: Vec::new(),
            cur_round: None,
            players,
            community_cards: Vec::new()
        }
    }

    #[test]
    fn test_calc_subpots_simple() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });

        let state = make_test_calc_winners_state(players);
        let result = calc_subpots(&state);
        let expected = vec![Subpot {
            chips: 34,
            players: vec![0, 1]
        }];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_calc_subpots_simple_fold() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: true,
            total_bet: 17
        });

        let state = make_test_calc_winners_state(players);
        let result = calc_subpots(&state);
        let expected = vec![Subpot {
            chips: 34,
            players: vec![0]
        }];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_calc_subpots_all_in() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 5
        });

        let state = make_test_calc_winners_state(players);
        let result = calc_subpots(&state);
        let expected = vec![Subpot {
            chips: 15,
            players: vec![0, 1, 2]
        }, Subpot {
            chips: 24,
            players: vec![0, 1]
        }];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_calc_subpots_fold() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: true,
            total_bet: 8
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });

        let state = make_test_calc_winners_state(players);
        let result = calc_subpots(&state);
        let expected = vec![Subpot {
            chips: 42,
            players: vec![1, 2]
        }];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_calc_subpots_fold_all_in() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: true,
            total_bet: 8
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 12
        });
        players.insert(3, PlayerState {
            chips: 0,
            hand: Vec::new(),
            folded: false,
            total_bet: 17
        });

        let state = make_test_calc_winners_state(players);
        let result = calc_subpots(&state);
        let expected = vec![Subpot {
            chips: 44,
            players: vec![0, 2, 3]
        }, Subpot {
            chips: 10,
            players: vec![0, 3]
        }];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_split_pot() {
        let players = vec![0, 2, 3];
        let pot = 33;
        let result = split_pot(&players, pot);
        let expected = vec![(0, 11), (2, 11), (3, 11)];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    #[test]
    fn test_split_pot_uneven() {
        let players = vec![0, 2, 3];
        let pot = 35;
        let result = split_pot(&players, pot);
        let expected = vec![(0, 12), (2, 12), (3, 11)];
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }

    fn make_cards(ts: Vec<(usize, usize)>) -> Vec<CardState> {
        ts.into_iter().map(|t| CardState {
            card: t.into(),
            facing: Facing::FaceDown
        }).collect()
    }

    #[test]
    fn test_calc_winners() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: make_cards(vec![(3, 0), (3,1), (0, 0), (9, 2), (1, 0)]),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: make_cards(vec![(3, 2), (1,2), (5, 0), (9, 3), (7, 2)]),
            folded: true,
            total_bet: 8
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: make_cards(vec![(8,3), (7, 3), (6, 3), (5, 3), (4, 3)]),
            folded: false,
            total_bet: 12
        });
        players.insert(3, PlayerState {
            chips: 0,
            hand: make_cards(vec![(12, 0), (12, 1), (9, 1), (12, 3), (12, 2)]),
            folded: false,
            total_bet: 17
        });

        let mut state = make_test_calc_winners_state(players);
        let result = calc_winners(&state).totals();
        let expected: HashMap<PlayerRole, Chips> = vec![(2, 44), (3, 10)].into_iter().collect();
        assert!(result == expected, format!("{:?} != {:?}", result, expected));
    }
}
