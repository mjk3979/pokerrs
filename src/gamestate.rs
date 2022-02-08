use crate::card::*;
use crate::special_card::*;
use crate::game::*;
use crate::table::*;
use crate::comb::*;
use crate::viewstate::*;
use crate::fold_channel;
use crate::bitcard::*;

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

#[derive(Clone)]
pub enum RoundState {
    Ante,
    Bet {
        player: PlayerRole,
        last_bet: Option<(Option<PlayerRole>, Chips)>,
        all_bets: HashMap<PlayerRole, Chips>
    },
    DrawToHand {
        facing: Vec<Facing>
    },
    DrawToCommunity {
        quant: usize
    },
    Replace {
        max_replace_fun: fn (&PlayerState) -> usize,
    },
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
            Round::DrawToCommunity{quant} => RoundState::DrawToCommunity{quant: *quant},
            Round::Replace{max_replace_fun, ..} => RoundState::Replace {
                max_replace_fun: *max_replace_fun
            },
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

pub struct BetState {
    pub player: PlayerRole,
    pub last_bet: Option<(Option<PlayerRole>, Chips)>,
    pub all_bets: HashMap<PlayerRole, Chips>,
}

pub struct HandState {
    pub deck: Mutex<Box<dyn Deck + Send>>,
    pub rounds: Vec<Round>,
    pub cur_round: Option<RoundState>,
    pub players: HashMap<PlayerRole, PlayerState>,
    pub community_cards: CardTuple,
    pub pending_bet: Option<(BetState, Vec<PokerGlobalViewDiff<PlayerRole>>)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[derive(TS)]
pub struct HandStrength {
    pub kind: Kind,
    pub kickers: RankTuple,
}

impl HandStrength {
    fn new(cards: CardTuple, hand_size: usize) -> Self {
        // Reorganize to make calculating easy
        let mut ranks: RankTuple = cards.into();
        ranks.sort();
        let is_straight = ranks.len() >= hand_size && (1..hand_size).all(|i| ranks.get(0) + i == ranks.get(i));
        let is_flush = cards.len() >= hand_size && (1..hand_size).all(|i| cards.get(0).suit == cards.get(i).suit);

        let mut by_rank: [u8; NUM_RANKS+1] = [0; NUM_RANKS+1];
        for rank in ranks.iter() {
            by_rank[rank] += 1;
        }
        let mut by_amount: [RankTuple; NUM_SUITS] = [RankTuple::new(); NUM_SUITS];
        for (rank, &amount) in by_rank.iter().enumerate() {
            let amount = std::cmp::min(amount, NUM_SUITS as u8);
            if amount > 0 {
                by_amount[(amount-1) as usize].push(rank);
            }
        }
        //println!("{:?}", ranks);
        //println!("{:?}", by_rank);
        //println!("{:?}", by_amount);
        
        // Go through cases best-to-worst
        use Kind::*;
        if is_straight && is_flush {
            return HandStrength {
                kind: StraightFlush(ranks.last().unwrap()),
                kickers: RankTuple::new(),
            };
        }
        if let Some(four_kind_rank) = by_amount[3].first() {
            return HandStrength {
                kind: FourKind(four_kind_rank),
                kickers: by_amount[0]
            };
        }
        if let Some(children) = by_amount[2].first() {
            if let Some(parents) = by_amount[1].first() {
                return HandStrength {
                    kind: FullHouse{
                        high: children,
                        low: parents,
                    },
                    kickers: RankTuple::new()
                };
            }
        }
        if is_flush {
            return HandStrength {
                kind: Flush(ranks.iter().rev().collect()),
                kickers: RankTuple::new()
            };
        }
        if is_straight {
            return HandStrength {
                kind: Straight(ranks.last().unwrap()),
                kickers: RankTuple::new()
            };
        }
        let kickers = by_amount[0].iter().rev().collect();
        if let Some(three_kind_rank) = by_amount[2].first() {
            return HandStrength {
                kind: ThreeKind(three_kind_rank),
                kickers
            }
        }
        if let Some(low_pair) = by_amount[1].first() {
            if let Some(high_pair) = by_amount[1].iter().nth(1) {
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
            kind: HighCard(ranks.last().unwrap()),
            kickers: ranks.iter().take(hand_size-1).rev().collect()
        };
    }
}

impl std::fmt::Display for HandStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

fn explode_aces(cards: CardTuple) -> CardTuple {
    let mut retval = cards;
    for card in cards.iter() {
        if card.rank == 0 {
            retval.push(Card {
                rank: NUM_RANKS,
                suit: card.suit
            });
        }
    }
    retval
}

pub fn best_hand(hand: CardTuple, community: CardTuple, hand_size: usize, rules: &SpecialRules) -> HandStrength {
    for rule in rules {
        if rule.wtype == SpecialCardType::WinsItAll {
            if let Some(_) = hand.iter().find(|cs| *cs == rule.card) {
                return HandStrength {
                    kind: Kind::WinsItAll,
                    kickers: RankTuple::new(),
                };
            }
        }
    }

    let mut all_cards = hand;
    for card in community.iter() {
        all_cards.push(card);
    }

    let comb_size = std::cmp::min(all_cards.len(), hand_size);
    combinations(all_cards.iter(), comb_size).map(|all_cards| {
        let mut unwild = CardTuple::new();
        let mut num_wild = 0;
    'outer: for &card in all_cards.iter() {
            for rule in rules {
                if SpecialCardType::Wild == rule.wtype && rule.card == card {
                    num_wild += 1;
                    continue 'outer;
                }
            }
            unwild.push(card);
        }

        wild_combinations(unwild, num_wild).into_iter().filter_map(|wild_hand| {
            aces_combos(wild_hand).into_iter().filter_map(|v|{
                Some(HandStrength::new(v, hand_size))
            }).max()
        }).max().unwrap()
    }).max().unwrap()
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct Subpot<P> {
    pub chips: Chips,
    pub players: Vec<P>
}

fn calc_subpots(state: &HandState) -> Vec<Subpot<PlayerRole>> {
    let mut bets: BTreeMap<Chips, (i32, Vec<PlayerRole>)> = BTreeMap::new();
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

pub fn best_hand_use_from_hand(use_from_hand: usize, hand: CardTuple, community: CardTuple, hand_size: usize, rules: &SpecialRules) -> HandStrength {
    let use_from_hand = std::cmp::min(hand.len(), use_from_hand);
    combinations(hand.iter(), use_from_hand).into_iter().map(|combo| {
        best_hand(combo.into_iter().collect(), community, hand_size, rules)
    }).max().unwrap()
}

fn calc_winners(variant: &PokerVariant, state: &HandState, rules: &SpecialRules) -> Winners<PlayerRole> {
    // calculate best hands for each player
    let best_hands: HashMap<PlayerRole, HandStrength> = state.players.iter().filter_map(|(&role, player)| {
        if player.folded {
            return None;
        }

        Some((role, {
            combinations(&player.hand, variant.use_from_hand).into_iter().map(|combo| {
                let community = state.community_cards.clone();
                best_hand(combo.iter().map(|cs| cs.card).collect(), community, 5, rules)
            }).max().unwrap()
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

pub fn next_player(current_role: PlayerRole, num_players: usize) -> PlayerRole {
    let mut new_player = current_role+1;
    if new_player >= num_players {
        return 0;
    }
    return new_player;
}

fn collect_ante_from_players(rule: &AnteRule, players: &mut HashMap<PlayerRole, PlayerState>) -> (Option<BetState>, Vec<PokerGlobalViewDiff<PlayerRole>>) {
    use AnteRule::*;
    let mut viewdiffs: Vec<PokerGlobalViewDiff<PlayerRole>> = Vec::new();
    let bet = match rule {
        Ante(ante) => {
            for (role, mut player) in players.iter_mut() {
                let chips = player.chips;
                let to_collect = std::cmp::min(chips, *ante);
                player.total_bet += to_collect;
                viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::from_blind_name(to_collect, *role, "ante".to_string())));
            }
            None
        }
        Blinds(blinds) => {
            let mut blind_starting_player = 0;
            if players.values().filter(|p| p.chips > 0).count() > 2 {
                blind_starting_player = 1;
            }
            let mut blind_role = blind_starting_player;
            let mut last = None;
            let mut all_bets = HashMap::new();
            for &Blind{amount} in blinds {
                let player = players.get_mut(&blind_role).unwrap();
                let chips = player.chips;
                let to_collect = std::cmp::min(chips, amount);
                //player.total_bet += to_collect;
                last = Some((blind_role, amount));
                all_bets.insert(blind_role, to_collect);
                viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::from_blind_name(to_collect, blind_role, "blind".to_string())));
                blind_role = next_player(blind_role, players.len());
            }
            last.map(|(last_blind_role, amount)| BetState {
                player: blind_role,
                last_bet: Some((None, amount)),
                all_bets
            })
        }
    };
    (bet, viewdiffs)
}

fn collect_bets(players: &mut PlayersState, bets: &HashMap<PlayerRole, Chips>) {
    for (role, &bet) in bets {
        players.get_mut(role).unwrap().total_bet += bet;
    }
}

fn update_players<'a, 'b, 'c, 'd, 'e>(players: &'b HashMap<PlayerRole, LivePlayer>, ids: &'e HashMap<PlayerRole, PlayerId>, spectator_channel: &'c Option<fold_channel::Sender<Vec<PokerGlobalViewDiff<PlayerId>>, Vec<PokerGlobalViewDiff<PlayerId>>>>, state: &'b HandState, role_viewdiffs: &'b [PokerGlobalViewDiff<PlayerRole>], rules: &SpecialRules, variant: &PokerVariant, round: usize) {
    if role_viewdiffs.is_empty() {
        return;
    }
    let viewdiffs: Vec<PokerGlobalViewDiff<PlayerId>> = role_viewdiffs.iter().map(|l| l.convert(ids)).collect();
    for (&role, player) in players.iter() {
        player.input.update(PokerViewUpdate {
            viewstate: PokerViewState::from_handstate_and_player(&state, &variant, &rules, role),
            diff: vec![PokerLogUpdate {
                round,
                log: viewdiffs.iter().map(|viewdiff| TableViewDiff::GameDiff(viewdiff.player_diff(Some(&player.player_id)))).collect()
            }],
        });
    }
    let diffs: Vec<PokerGlobalViewDiff<PlayerId>> = viewdiffs.iter().cloned().collect();
    //println!("Sending log {}", diffs.len());
    if let Some(channel) = spectator_channel {
        channel.send(diffs);
    }
}

pub fn show_cards(variant: &PokerVariant,
    players: &mut HashMap<PlayerRole, PlayerState>,
    viewdiffs: &mut Vec<PokerGlobalViewDiff<PlayerRole>>,
    last_bet: Option<PlayerRole>,
    community_cards: CardTuple,
    rules: &SpecialRules,
    ){
    if players.values().filter(|p| !p.folded).count() < 2 {
        return;
    }
    // last best and then to the left
    let starting = last_bet.unwrap_or(0); // todo who actually goes first?
    let num_players = players.len();
    let mut role = starting;
    loop {
        let player = players.get_mut(&role).unwrap();
        if !player.folded {
            let mut shown = Vec::new();
            for (idx, cs) in player.hand.iter_mut().enumerate() {
                if Facing::FaceDown == cs.facing {
                    cs.facing = Facing::FaceUp;
                    shown.push((idx, CardViewState::Visible(cs.clone())));
                }
            }
            if !shown.is_empty() {
                let strength = combinations(&player.hand, variant.use_from_hand).into_iter().map(|combo| {
                    best_hand(combo.iter().map(|cs| cs.card).collect(), community_cards.clone(), 5, rules)
                }).max().unwrap();
                viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::ShowCards {
                    player: role,
                    shown,
                    strength
                }));
            }
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
    rules: SpecialRules,
    round: usize
    ) ->
    Result<HashMap<PlayerRole, Chips>, PokerRoundError> {
    let mut state = HandState {
        deck: starting_deck,
        rounds: variant.rules.iter().cloned().rev().collect(),
        cur_round: None,
        players: players.iter().map(|(&e, p)| (e, PlayerState{chips: p.chips, hand: Vec::new(), folded: false, total_bet: 0})).collect(),
        community_cards: CardTuple::new(),
        pending_bet: None,
    };

    let num_players = players.len();
    let mut viewdiffs: Vec<PokerGlobalViewDiff<PlayerRole>> = Vec::new();
    let mut hand_last_bet: Option<PlayerRole> = None;
    let ids = players.iter().map(|(role, p)| (*role, p.player_id.clone())).collect();

    loop {
        update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, &rules, &variant, round);
        viewdiffs.clear();
        println!("Round");
        match state.cur_round {
            None => {
                let end_game = state.players.iter().filter(|(role, player)| {
                    !player.folded
                }).count() <= 1;
                if end_game {
                    let (winner_role, winner) = state.players.iter().find(|(role, player)| {
                        !player.folded
                    }).unwrap();
                    let subpots = calc_subpots(&state);
                    let winners = Winners {
                        winners_by_pot: subpots.into_iter().map(|s| (s, vec![*winner_role])).collect()
                    };
                    let mut retval = winners.totals();
                    for (&role, player) in &state.players {
                        *retval.entry(role).or_insert(0) -= player.total_bet;
                    }
                    viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::Winners(winners)));
                    update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, &rules, &variant, round);
                    viewdiffs.clear();
                    return Ok(retval);
                } else if let Some(next_round) = state.rounds.pop() {
                    let mut new_round = RoundState::new(&next_round);
                    if let RoundState::Bet{..} = new_round {
                        if let Some((BetState{player, last_bet, all_bets}, pending_viewdiffs)) = state.pending_bet.take() {
                            new_round = RoundState::Bet{player, last_bet, all_bets};
                            for viewdiff in pending_viewdiffs {
                                viewdiffs.push(viewdiff);
                            }
                        }
                    }

                    state.cur_round = Some(new_round);
                    continue;
                } else {
                    show_cards(&variant, &mut state.players, &mut viewdiffs, hand_last_bet, state.community_cards, &rules);
                    update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, &rules, &variant, round);
                    viewdiffs.clear();
                    let winners = calc_winners(&variant, &state, &rules);
                    let mut retval = winners.totals();
                    for (&role, player) in &state.players {
                        *retval.entry(role).or_insert(0) -= player.total_bet;
                    }
                    viewdiffs.push(PokerGlobalViewDiff::Common(PokerViewDiff::Winners(winners)));
                    update_players(&players, &ids, &spectator_channel, &state, &viewdiffs, &rules, &variant, round);
                    viewdiffs.clear();
                    return Ok(retval);
                }
            },
            Some(round_state) => {
                use RoundState::*;
                match round_state {
                    Ante => {
                        let (bet, vds) = collect_ante_from_players(&table_rules.ante, &mut state.players);
                        if let Some(bet) = bet {
                            state.pending_bet = Some((bet, vds));
                        } else {
                            for vd in vds {
                                viewdiffs.push(vd);
                            }
                        }
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
                            if last_bet_player == Some(bet_role) {
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
                        if !player.folded && player.chips > player.total_bet + *all_bets.get(&bet_role).unwrap_or(&0) {
                            println!("Waiting on {}", bet_role);
                            let f = players.get(&bet_role).unwrap().input.bet(last_bet_amount, min_bet);
                            match f.await {
                                BetResp::Bet(num_chips) => {
                                    //assert!(num_chips == 0 || num_chips == last_bet_amount || num_chips >= min_bet);
                                    if last_bet.is_none() || last_bet.unwrap().0.is_none() || num_chips > last_bet_amount {
                                        this_bet = Some((Some(bet_role), num_chips));
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
                    },
                    Replace{max_replace_fun} => {
                        state.cur_round = None;
                        let dealer = 0;
                        let mut role = dealer;
                        loop {
                            role = next_player(role, num_players);
                            let player = state.players.get(&role).unwrap();
                            if !player.folded {
                                let resp = players.get(&role).unwrap().input.replace(max_replace_fun(player)).await;
                                let mut player = state.players.get_mut(&role).unwrap();
                                let mut discard = Vec::new();
                                let mut drawn = Vec::new();
                                for idx in resp {
                                    let old_card = player.hand[idx].clone();
                                    discard.push(old_card);
                                    player.hand[idx].card = state.deck.lock().unwrap().draw()?;
                                    drawn.push(player.hand[idx].clone());
                                }
                                viewdiffs.push(PokerGlobalViewDiff::Replace {
                                    player: role,
                                    discard,
                                    drawn
                                });
                            }
                            if role == dealer {
                                break;
                            }
                        }
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
        let cards: CardTuple = vec![1, 2, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let result = explode_aces(cards);
        assert!(result == cards, "{:?} != {:?}", result, cards);
    }

    #[test]
    fn test_explode_aces_single_ace() {
        let cards: CardTuple = vec![1, 0, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_explode_aces_single_ace_first() {
        let cards: CardTuple = vec![0, 1, 3, NUM_RANKS-1, 9].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_explode_aces_single_ace_last() {
        let cards: CardTuple = vec![2, 1, 3, NUM_RANKS-1, 0].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_explode_aces_two_aces() {
        let cards: CardTuple = vec![0, 1, 0, NUM_RANKS-1, 7].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_explode_aces_two_aces_adjacent() {
        let cards: CardTuple = vec![0, 0, 2, NUM_RANKS-1, 7].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_explode_aces_four_aces() {
        let cards: CardTuple = vec![0, 0, 0, NUM_RANKS-1, 0].into_iter().map(|rank| Card{rank, suit: Suit(0)}).collect();
        let mut expected = cards.clone();
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        expected.push(Card {
            rank:NUM_RANKS,
            suit: Suit(0)
        });
        let result = explode_aces(cards);
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_high_card() {
        let cards: CardTuple = vec![4, 2, 7, 5, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::HighCard(9),
            kickers: vec![7, 5, 4, 2].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }
    
    #[test]
    fn test_best_hand_high_card_ace() {
        let cards: CardTuple = vec![4, 2, 0, 5, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::HighCard(NUM_RANKS),
            kickers: vec![9, 5, 4, 2].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_pair() {
        let cards: CardTuple = vec![4, 2, 1, 4, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::Pair(4),
            kickers: vec![9, 2, 1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_pair_ace_not_in_pair() {
        let cards: CardTuple = vec![4, 0, 1, 4, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::Pair(4),
            kickers: vec![NUM_RANKS, 9, 1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_pair_ace_in_pair() {
        let cards: CardTuple = vec![0, 2, 1, 0, 4].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::Pair(NUM_RANKS),
            kickers: vec![4, 2, 1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_two_pair() {
        let cards: CardTuple = vec![4, 2, 1, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: 4, low: 2},
            kickers: vec![1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_two_pair_ace_not_in_two_pair() {
        let cards: CardTuple = vec![4, 2, 0, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: 4, low: 2},
            kickers: vec![NUM_RANKS].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_two_pair_ace_in_two_pair() {
        let cards: CardTuple = vec![4, 0, 1, 4, 0].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::TwoPair{high: NUM_RANKS, low: 4},
            kickers: vec![1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_three_kind() {
        let cards: CardTuple = vec![4, 4, 1, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::ThreeKind(4),
            kickers: vec![2, 1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_three_kind_ace_not_in_three_kind() {
        let cards: CardTuple = vec![4, 4, 0, 4, 2].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::ThreeKind(4),
            kickers: vec![NUM_RANKS, 2].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_three_kind_ace_in_three_kind() {
        let cards: CardTuple = vec![0, 0, 1, 0, 4].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &vec![]);
        let expected = HandStrength{
            kind: Kind::ThreeKind(NUM_RANKS),
            kickers: vec![4, 1].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_best_hand_wilds() {
        let cards: CardTuple = vec![1, 0, 1, 0, 4].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        let result = best_hand(cards, CardTuple::new(), 5, &((0..4).map(|s| SpecialCard{wtype: SpecialCardType::Wild, card: Card{rank: 1, suit: Suit(s)}}).collect()));
        let expected = HandStrength{
            kind: Kind::FourKind(NUM_RANKS),
            kickers: vec![4].into()
        };
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    fn make_test_calc_winners_state(players: HashMap<PlayerRole, PlayerState>) -> HandState {
        HandState {
            deck: Mutex::new(Box::new(standard_deck().clone())),
            rounds: Vec::new(),
            cur_round: None,
            players,
            community_cards: CardTuple::new(),
            pending_bet: None,
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
        assert!(result == expected, "{:?} != {:?}", result, expected);
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
        assert!(result == expected, "{:?} != {:?}", result, expected);
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
        assert!(result == expected, "{:?} != {:?}", result, expected);
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
        assert!(result == expected, "{:?} != {:?}", result, expected);
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
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_split_pot() {
        let players = vec![0, 2, 3];
        let pot = 33;
        let result = split_pot(&players, pot);
        let expected = vec![(0, 11), (2, 11), (3, 11)];
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_split_pot_uneven() {
        let players = vec![0, 2, 3];
        let pot = 35;
        let result = split_pot(&players, pot);
        let expected = vec![(0, 12), (2, 12), (3, 11)];
        assert!(result == expected, "{:?} != {:?}", result, expected);
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
        let result = calc_winners(&five_card_stud(), &state, &vec![]).totals();
        let expected: HashMap<PlayerRole, Chips> = vec![(2, 44), (3, 10)].into_iter().collect();
        assert!(result == expected, "{:?} != {:?}", result, expected);

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

        players.get_mut(&0).unwrap().hand = make_cards(vec![(0, 0), (0, 1), (0, 2), (0, 3), (1, 0)]);
        let mut state = make_test_calc_winners_state(players);
        let result = calc_winners(&five_card_stud(), &state, &vec![]).totals();
        let expected: HashMap<PlayerRole, Chips> = vec![(2, 44), (0, 10)].into_iter().collect();
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_calc_winners_omaha() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: make_cards(vec![(0, 0), (0,1), (1, 2), (9, 2),]),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: make_cards(vec![(3, 2), (1,2), (5, 0), (9, 3)]),
            folded: true,
            total_bet: 8
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: make_cards(vec![(8,3), (7, 3), (6, 3), (5, 3)]),
            folded: false,
            total_bet: 12
        });
        players.insert(3, PlayerState {
            chips: 0,
            hand: make_cards(vec![(12, 0), (12, 1), (9, 1), (12, 3)]),
            folded: false,
            total_bet: 17
        });
        let community = make_cards(vec![(1, 1), (0, 2), (3, 3), (7, 0)]);

        players.get_mut(&0).unwrap().hand = make_cards(vec![(0, 3), (9, 1), (0, 2), (9, 3), (1, 0)]);
        let mut state = make_test_calc_winners_state(players);
        state.community_cards = community.into_iter().map(|cs| cs.card).collect();
        let result = calc_winners(&omaha_hold_em(), &state, &vec![]).totals();
        let expected: HashMap<PlayerRole, Chips> = vec![(0, 54)].into_iter().collect();
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }

    #[test]
    fn test_calc_winners_omaha_2() {
        let mut players = HashMap::new();
        players.insert(0, PlayerState {
            chips: 0,
            hand: make_cards(vec![(2, 2), (NUM_RANKS-1,1), (NUM_RANKS-2, 2), (0, 2),]),
            folded: false,
            total_bet: 17
        });
        players.insert(1, PlayerState {
            chips: 0,
            hand: make_cards(vec![(3, 2), (1,2), (5, 0), (9, 3)]),
            folded: true,
            total_bet: 8
        });
        players.insert(2, PlayerState {
            chips: 0,
            hand: make_cards(vec![(8,3), (7, 3), (6, 3), (5, 3)]),
            folded: false,
            total_bet: 12
        });
        players.insert(3, PlayerState {
            chips: 0,
            hand: make_cards(vec![(12, 0), (12, 1), (9, 1), (12, 3)]),
            folded: false,
            total_bet: 17
        });
        let community = make_cards(vec![(0, 1), (4, 2), (10, 3), (9, 0), (10, 0)]);
        let community_cards: CardTuple = community.iter().map(|cs| cs.card).collect();

        //players.get_mut(&0).unwrap().hand = make_cards(vec![(0, 3), (9, 1), (0, 2), (9, 3), (1, 0)]);
        let strength = combinations(&players.get(&0).unwrap().hand, 2).into_iter().map(|combo| {
            best_hand(combo.iter().map(|cs| cs.card).collect(), community_cards.clone(), 5, &vec![])
        }).max().unwrap();
        assert!(strength.kind == Kind::Straight(NUM_RANKS), "{:?}", strength.kind);
        let mut state = make_test_calc_winners_state(players);
        state.community_cards = community.into_iter().map(|cs| cs.card).collect();
        let result = calc_winners(&omaha_hold_em(), &state, &vec![]).totals();
        let expected: HashMap<PlayerRole, Chips> = vec![(0, 54)].into_iter().collect();
        assert!(result == expected, "{:?} != {:?}", result, expected);
    }
}
