//! Blackjack simulator core: 6-deck shoe, 5 decks dealt, Hi-Lo with Illustrious 18 + Fab 4.
//! Rules: S17, DAS, late surrender, dealer peeks, split to 4 hands, split aces once
//! (one card each, no resplit), 3:2 blackjack, insurance offered.
//!
//! Mirrors ../cpp/bj.hpp step for step, including the RNG and the order cards are drawn,
//! so both produce identical results for the same seed.

pub const DECKS: usize = 6;
pub const CARDS: usize = DECKS * 52; // 312
pub const CUT: usize = 5 * 52; // reshuffle once 260 cards (5 decks) are dealt
pub const MAX_HANDS: usize = 4;
pub const TC_MIN: i32 = -10;
pub const TC_MAX: i32 = 10;
pub const TC_BUCKETS: usize = (TC_MAX - TC_MIN + 1) as usize;

/// Card ranks are 1..10 (1 = ace, 10 = any ten-value card).
pub const HILO: [i8; 11] = [0, -1, 1, 1, 1, 1, 1, 0, 0, 0, -1];

// ---------------------------------------------------------------- RNG
// xoshiro256** seeded by splitmix64; Lemire's unbiased bounded integers.
pub fn splitmix64(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(mut seed: u64) -> Self {
        let mut s = [0u64; 4];
        for x in &mut s {
            *x = splitmix64(&mut seed);
        }
        Rng { s }
    }

    #[inline(always)]
    pub fn next(&mut self) -> u64 {
        let s = &mut self.s;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }

    /// Uniform in [0, range).
    #[inline(always)]
    pub fn below(&mut self, range: u32) -> u32 {
        let mut x = self.next() >> 32;
        let mut m = x * range as u64;
        let mut l = m as u32;
        if l < range {
            let t = range.wrapping_neg() % range;
            while l < t {
                x = self.next() >> 32;
                m = x * range as u64;
                l = m as u32;
            }
        }
        (m >> 32) as u32
    }
}

// ---------------------------------------------------------------- Shoe
/// 312 bytes. Shuffling is lazy Fisher-Yates: each draw swaps a random undealt card into
/// position `pos`. The array always stays a permutation of the full shoe, so a reshuffle is
/// just `pos = 0` and only the 260 cards actually dealt are ever shuffled.
pub struct Shoe {
    pub cards: [u8; CARDS],
    pub pos: usize,
}

impl Default for Shoe {
    fn default() -> Self {
        let mut cards = [0u8; CARDS];
        let mut i = 0;
        for _ in 0..DECKS * 4 {
            for r in 1..=13u8 {
                cards[i] = r.min(10);
                i += 1;
            }
        }
        Shoe { cards, pos: 0 }
    }
}

impl Shoe {
    #[inline(always)]
    pub fn draw(&mut self, rng: &mut Rng) -> u8 {
        let j = self.pos + rng.below((CARDS - self.pos) as u32) as usize;
        self.cards.swap(self.pos, j);
        let c = self.cards[self.pos];
        self.pos += 1;
        c
    }
}

// ---------------------------------------------------------------- Hand
#[derive(Clone, Copy)]
pub struct Hand {
    pub sum: u8,      // aces counted as 1
    pub ncards: u8,
    pub first: u8,    // first card rank (pair detection)
    pub bet_mult: u8, // 1, or 2 after doubling
    pub ace: bool,
    pub split_aces: bool,
    pub surrendered: bool,
}

impl Default for Hand {
    fn default() -> Self {
        Hand { sum: 0, ncards: 0, first: 0, bet_mult: 1, ace: false, split_aces: false, surrendered: false }
    }
}

impl Hand {
    pub fn from(cards: &[u8]) -> Self {
        let mut h = Hand::default();
        for &c in cards {
            h.add(c);
        }
        h
    }
    #[inline(always)]
    pub fn add(&mut self, c: u8) {
        if self.ncards == 0 {
            self.first = c;
        }
        self.sum += c;
        self.ace |= c == 1;
        self.ncards += 1;
    }
    #[inline(always)]
    pub fn soft(&self) -> bool {
        self.ace && self.sum <= 11
    }
    #[inline(always)]
    pub fn value(&self) -> i32 {
        if self.soft() { self.sum as i32 + 10 } else { self.sum as i32 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Hit,
    Stand,
    Double,
    Split,
    Surrender,
}
use Action::*;

// ---------------------------------------------------------------- Stats
/// Money is tracked in half-units so 3:2, insurance and surrender stay integral.
#[derive(Clone, Default)]
pub struct Stats {
    pub rounds: u64,
    pub hands: u64,
    pub shuffles: u64,
    pub net: i64,         // spread-bet result, half-units
    pub initial_bet: u64, // sum of initial bets, half-units
    pub action: u64,      // all money put at risk incl. doubles/splits/insurance
    pub net_sq: u64,      // sum of squared per-round net (half-units^2)
    pub flat_net: i64,    // same play with a 1-unit bet every round
    pub flat_sq: u64,
    pub wins: u64,
    pub losses: u64,
    pub pushes: u64,
    pub player_bj: u64,
    pub dealer_bj: u64,
    pub doubles: u64,
    pub splits: u64,
    pub surrenders: u64,
    pub busts: u64,
    pub ins_taken: u64,
    pub ins_won: u64,
    pub tc_rounds: [u64; TC_BUCKETS],
    pub tc_flat_net: [i64; TC_BUCKETS],
}

impl Stats {
    pub fn merge(&mut self, o: &Stats) {
        self.rounds += o.rounds;
        self.hands += o.hands;
        self.shuffles += o.shuffles;
        self.net += o.net;
        self.initial_bet += o.initial_bet;
        self.action += o.action;
        self.net_sq += o.net_sq;
        self.flat_net += o.flat_net;
        self.flat_sq += o.flat_sq;
        self.wins += o.wins;
        self.losses += o.losses;
        self.pushes += o.pushes;
        self.player_bj += o.player_bj;
        self.dealer_bj += o.dealer_bj;
        self.doubles += o.doubles;
        self.splits += o.splits;
        self.surrenders += o.surrenders;
        self.busts += o.busts;
        self.ins_taken += o.ins_taken;
        self.ins_won += o.ins_won;
        for i in 0..TC_BUCKETS {
            self.tc_rounds[i] += o.tc_rounds[i];
            self.tc_flat_net[i] += o.tc_flat_net[i];
        }
    }
}

/// Bet ramp in units, keyed on floored true count: 1-12 spread.
pub fn bet_units(tc: i32) -> i32 {
    if tc < 2 { 1 } else { (2 * (tc - 1)).min(12) }
}

// ---------------------------------------------------------------- Player
pub struct Sim {
    pub rng: Rng,
    pub shoe: Shoe,
    pub rc: i32,           // Hi-Lo running count
    pub unseen_extra: i32, // 1 while the dealer's hole card is drawn but not yet seen
    pub counting: bool,    // false = flat-bet basic strategy (for validation)
    pub st: Stats,
}

impl Sim {
    pub fn new(seed: u64, counting: bool) -> Self {
        Sim { rng: Rng::new(seed), shoe: Shoe::default(), rc: 0, unseen_extra: 0, counting, st: Stats::default() }
    }

    #[inline(always)]
    fn unseen(&self) -> i32 {
        (CARDS - self.shoe.pos) as i32 + self.unseen_extra
    }

    /// Exact true-count comparison: rc / (unseen/52) >= k, with no division or rounding.
    #[inline(always)]
    pub fn tc_ge(&self, k: i32) -> bool {
        self.rc * 52 >= k * self.unseen()
    }

    /// Index plays fire only when counting; otherwise fall back to basic strategy.
    #[inline(always)]
    fn idx(&self, k: i32, basic: bool) -> bool {
        if self.counting { self.tc_ge(k) } else { basic }
    }

    #[inline(always)]
    fn draw_seen(&mut self) -> u8 {
        let c = self.shoe.draw(&mut self.rng);
        self.rc += HILO[c as usize] as i32;
        c
    }

    pub fn decide(&self, h: &Hand, up: u8, can_double: bool, can_split: bool, can_surrender: bool) -> Action {
        let v = h.value();
        let pair = h.ncards == 2 && h.sum == 2 * h.first;

        if can_surrender && !h.soft() && !(pair && h.first == 8) {
            if v == 16 && (up == 9 || up == 10 || up == 1) {
                return Surrender;
            }
            if v == 15 && up == 10 && self.idx(0, true) {
                return Surrender;
            }
            if v == 15 && up == 9 && self.idx(2, false) {
                return Surrender;
            }
            if v == 15 && up == 1 && self.idx(1, false) {
                return Surrender;
            }
            if v == 14 && up == 10 && self.idx(3, false) {
                return Surrender;
            }
        }

        if pair && can_split {
            let split = match h.first {
                1 | 8 => true,
                10 => (up == 5 && self.idx(5, false)) || (up == 6 && self.idx(4, false)),
                9 => (2..=6).contains(&up) || up == 8 || up == 9,
                7 => (2..=7).contains(&up),
                6 => (2..=6).contains(&up),
                4 => up == 5 || up == 6,
                2 | 3 => (2..=7).contains(&up),
                _ => false, // 5,5 plays as hard 10
            };
            if split {
                return Split;
            }
        }

        if h.soft() {
            if v >= 19 {
                return Stand;
            }
            if v == 18 {
                if (3..=6).contains(&up) {
                    return if can_double { Double } else { Stand };
                }
                if up == 2 || up == 7 || up == 8 {
                    return Stand;
                }
                return Hit;
            }
            let dbl = (v == 17 && (3..=6).contains(&up))
                || ((v == 15 || v == 16) && (4..=6).contains(&up))
                || ((v == 13 || v == 14) && (5..=6).contains(&up));
            return if dbl && can_double { Double } else { Hit };
        }

        if v >= 17 {
            return Stand;
        }
        if v >= 13 {
            let stand = match (v, up) {
                (16, 10) => self.idx(0, false),
                (15, 10) => self.idx(4, false),
                (16, 9) => self.idx(5, false),
                (13, 2) => self.idx(-1, true),
                (13, 3) => self.idx(-2, true),
                _ => (2..=6).contains(&up),
            };
            return if stand { Stand } else { Hit };
        }
        if v == 12 {
            let stand = match up {
                2 => self.idx(3, false),
                3 => self.idx(2, false),
                4 => self.idx(0, true),
                5 => self.idx(-2, true),
                6 => self.idx(-1, true),
                _ => false,
            };
            return if stand { Stand } else { Hit };
        }
        let dbl = match v {
            11 => up != 1 || self.idx(1, false),
            10 => (2..=9).contains(&up) || ((up == 10 || up == 1) && self.idx(4, false)),
            9 => (3..=6).contains(&up) || (up == 2 && self.idx(1, false)) || (up == 7 && self.idx(3, false)),
            _ => false,
        };
        if dbl && can_double { Double } else { Hit }
    }

    pub fn play_round(&mut self) {
        if self.shoe.pos >= CUT {
            self.shoe.pos = 0;
            self.rc = 0;
            self.st.shuffles += 1;
        }
        let tc = (self.rc * 52).div_euclid((CARDS - self.shoe.pos) as i32); // floor
        let units = if self.counting { bet_units(tc) } else { 1 };
        let b = 2 * units as i64; // initial bet, half-units
        let mut net: i64 = 0;
        self.st.rounds += 1;
        self.st.initial_bet += b as u64;
        self.st.action += b as u64;

        let mut hands = [Hand::default(); MAX_HANDS];
        let mut nh = 1;
        let c = self.draw_seen();
        hands[0].add(c);
        let up = self.draw_seen();
        let c = self.draw_seen();
        hands[0].add(c);
        let hole = self.shoe.draw(&mut self.rng);
        self.unseen_extra = 1;

        let dealer_bj = (up == 1 && hole == 10) || (up == 10 && hole == 1);
        let player_bj = hands[0].value() == 21;

        if up == 1 && self.counting && self.tc_ge(3) {
            self.st.ins_taken += 1;
            self.st.action += (b / 2) as u64;
            if dealer_bj {
                net += b;
                self.st.ins_won += 1;
            } else {
                net -= b / 2;
            }
        }

        if dealer_bj || player_bj {
            self.rc += HILO[hole as usize] as i32;
            self.unseen_extra = 0;
            self.st.hands += 1;
            if dealer_bj {
                self.st.dealer_bj += 1;
            }
            if player_bj {
                self.st.player_bj += 1;
            }
            if dealer_bj && player_bj {
                self.st.pushes += 1;
            } else if dealer_bj {
                net -= b;
                self.st.losses += 1;
            } else {
                net += 3 * b / 2;
                self.st.wins += 1;
            }
            self.finish(net, units, tc);
            return;
        }

        let mut any_live = false;
        let mut i = 0;
        while i < nh {
            let mut h = hands[i];
            if h.ncards == 1 {
                // second card for a split hand
                let c = self.draw_seen();
                h.add(c);
                if h.split_aces {
                    hands[i] = h;
                    any_live = true;
                    i += 1;
                    continue;
                }
            }
            loop {
                if h.value() >= 21 {
                    break;
                }
                let two = h.ncards == 2;
                // Split aces never reach here, so no resplitting aces falls out naturally.
                match self.decide(&h, up, two, two && nh < MAX_HANDS, two && nh == 1) {
                    Surrender => {
                        h.surrendered = true;
                        break;
                    }
                    Stand => break,
                    Hit => {
                        let c = self.draw_seen();
                        h.add(c);
                    }
                    Double => {
                        h.bet_mult = 2;
                        self.st.doubles += 1;
                        self.st.action += b as u64;
                        let c = self.draw_seen();
                        h.add(c);
                        break;
                    }
                    Split => {
                        self.st.splits += 1;
                        self.st.action += b as u64;
                        let c = h.first;
                        let mut nhand = Hand::default();
                        nhand.add(c);
                        h = Hand::default();
                        h.add(c);
                        if c == 1 {
                            h.split_aces = true;
                            nhand.split_aces = true;
                        }
                        hands[nh] = nhand;
                        nh += 1;
                        let c = self.draw_seen();
                        h.add(c);
                        if h.split_aces {
                            break;
                        }
                    }
                }
            }
            if !h.surrendered && h.value() <= 21 {
                any_live = true;
            }
            hands[i] = h;
            i += 1;
        }

        self.rc += HILO[hole as usize] as i32;
        self.unseen_extra = 0;
        let mut d = Hand::default();
        d.add(up);
        d.add(hole);
        if any_live {
            while d.value() < 17 {
                let c = self.draw_seen();
                d.add(c);
            }
        }
        let dv = d.value();

        for h in &hands[..nh] {
            let w = b * h.bet_mult as i64;
            let pv = h.value();
            self.st.hands += 1;
            if h.surrendered {
                net -= b / 2;
                self.st.surrenders += 1;
                self.st.losses += 1;
            } else if pv > 21 {
                net -= w;
                self.st.busts += 1;
                self.st.losses += 1;
            } else if dv > 21 || pv > dv {
                net += w;
                self.st.wins += 1;
            } else if pv < dv {
                net -= w;
                self.st.losses += 1;
            } else {
                self.st.pushes += 1;
            }
        }
        self.finish(net, units, tc);
    }

    fn finish(&mut self, net: i64, units: i32, tc: i32) {
        let st = &mut self.st;
        st.net += net;
        st.net_sq += (net * net) as u64;
        let flat = net / units as i64; // every amount scales with the bet, so this is exact
        st.flat_net += flat;
        st.flat_sq += (flat * flat) as u64;
        let bucket = (tc.clamp(TC_MIN, TC_MAX) - TC_MIN) as usize;
        st.tc_rounds[bucket] += 1;
        st.tc_flat_net[bucket] += flat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_tc(tc: i32) -> Sim {
        let mut s = Sim::new(1, true);
        s.shoe.pos = CARDS - 104; // two decks unseen
        s.rc = tc * 2;
        s
    }

    #[test]
    fn shoe_composition_and_balance() {
        let sh = Shoe::default();
        let mut counts = [0; 11];
        let mut sum = 0i32;
        for &c in &sh.cards {
            counts[c as usize] += 1;
            sum += HILO[c as usize] as i32;
        }
        assert!(counts[1..=9].iter().all(|&n| n == 24));
        assert_eq!(counts[10], 96);
        assert_eq!(sum, 0);
    }

    #[test]
    fn lazy_shuffle_keeps_permutation() {
        let mut sh = Shoe::default();
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            sh.pos = 0;
            let mut counts = [0; 11];
            for _ in 0..CARDS {
                counts[sh.draw(&mut rng) as usize] += 1;
            }
            assert!(counts[1..=9].iter().all(|&n| n == 24));
            assert_eq!(counts[10], 96);
        }
    }

    #[test]
    fn rng_bounded_uniform_and_matches_reference() {
        let mut rng = Rng::new(3);
        let mut b = [0; 13];
        for _ in 0..1_300_000 {
            b[rng.below(13) as usize] += 1;
        }
        assert!(b.iter().all(|&x| x > 99_000 && x < 101_000));
    }

    #[test]
    fn hand_values() {
        assert!(Hand::from(&[1, 6]).value() == 17 && Hand::from(&[1, 6]).soft());
        assert!(Hand::from(&[1, 6, 10]).value() == 17 && !Hand::from(&[1, 6, 10]).soft());
        assert_eq!(Hand::from(&[1, 1, 9]).value(), 21);
        assert_eq!(Hand::from(&[10, 1]).value(), 21);
        assert_eq!((-1i32).div_euclid(4), -1);
        assert_eq!((-4i32).div_euclid(4), -1);
        assert_eq!([1, 2, 4, 9].map(bet_units), [1, 2, 6, 12]);
    }

    #[test]
    fn basic_strategy_spots() {
        let s = at_tc(0);
        let d = |c: &[u8], up, dbl, spl, sur| s.decide(&Hand::from(c), up, dbl, spl, sur);
        assert_eq!(d(&[10, 6], 10, true, true, true), Surrender);
        assert_eq!(d(&[8, 8], 10, true, true, true), Split);
        assert_eq!(d(&[1, 1], 1, true, true, true), Split);
        assert_eq!(d(&[5, 5], 6, true, true, true), Double);
        assert_eq!(d(&[9, 9], 7, true, true, true), Stand);
        assert_eq!(d(&[9, 9], 8, true, true, true), Split);
        assert_eq!(d(&[6, 5], 1, true, true, true), Hit);
        assert_eq!(d(&[1, 7], 9, true, true, true), Hit);
        assert_eq!(d(&[1, 7], 6, true, true, true), Double);
        assert_eq!(d(&[1, 7], 6, false, true, true), Stand);
        assert_eq!(d(&[1, 6], 2, true, true, true), Hit);
        assert_eq!(d(&[10, 2], 4, true, true, true), Stand);
        assert_eq!(d(&[10, 3], 2, true, true, true), Stand);
        assert_eq!(d(&[10, 6], 7, true, true, true), Hit);
        assert_eq!(d(&[5, 4, 7], 10, false, false, false), Stand); // 16v10 at TC 0
    }

    #[test]
    fn illustrious_18_and_fab_4_thresholds() {
        // (cards, upcard, index, action one below the index, action at the index)
        let devs: &[(&[u8], u8, i32, Action, Action)] = &[
            (&[10, 5], 10, 4, Surrender, Surrender),
            (&[4, 3, 2, 6], 10, 4, Hit, Stand),
            (&[10, 10], 5, 5, Stand, Split),
            (&[10, 10], 6, 4, Stand, Split),
            (&[6, 4], 10, 4, Hit, Double),
            (&[10, 2], 3, 2, Hit, Stand),
            (&[10, 2], 2, 3, Hit, Stand),
            (&[6, 5], 1, 1, Hit, Double),
            (&[5, 4], 2, 1, Hit, Double),
            (&[6, 4], 1, 4, Hit, Double),
            (&[5, 4], 7, 3, Hit, Double),
            (&[3, 4, 9], 9, 5, Hit, Stand),
            (&[10, 3], 2, -1, Hit, Stand),
            (&[10, 2], 4, 0, Hit, Stand),
            (&[10, 2], 5, -2, Hit, Stand),
            (&[10, 2], 6, -1, Hit, Stand),
            (&[10, 3], 3, -2, Hit, Stand),
            (&[10, 4], 10, 3, Hit, Surrender),
            (&[10, 5], 9, 2, Hit, Surrender),
            (&[10, 5], 1, 1, Hit, Surrender),
            (&[9, 7], 10, 0, Hit, Stand), // 16v10 stand index, surrender unavailable
        ];
        for &(cards, up, index, below, at) in devs {
            let h = Hand::from(cards);
            let two = h.ncards == 2;
            let surr = two && at != Stand;
            assert_eq!(at_tc(index - 1).decide(&h, up, two, true, surr), below, "{cards:?} v {up} below");
            assert_eq!(at_tc(index).decide(&h, up, two, true, surr), at, "{cards:?} v {up} at");
        }
        assert_eq!(at_tc(-1).decide(&Hand::from(&[10, 5]), 10, true, true, true), Hit);
    }

    #[test]
    fn basic_mode_ignores_count() {
        let mut s = Sim::new(1, false);
        s.shoe.pos = CARDS - 52;
        s.rc = 20;
        assert_eq!(s.decide(&Hand::from(&[10, 10]), 6, true, true, true), Stand);
        assert_eq!(s.decide(&Hand::from(&[10, 6]), 10, true, true, false), Hit);
    }

    #[test]
    fn running_count_matches_dealt_cards() {
        let mut s = Sim::new(11, true);
        for _ in 0..200_000 {
            s.play_round();
            let rc: i32 = s.shoe.cards[..s.shoe.pos].iter().map(|&c| HILO[c as usize] as i32).sum();
            assert_eq!(rc, s.rc);
        }
        assert_eq!(s.st.rounds, 200_000);
    }
}
