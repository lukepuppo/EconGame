//! Usage: bj [rounds=100000000] [threads=all] [seed=1] [hilo|basic] [vec|ref]
//! Games run in lanes (independent games with their own shoes), GROUPS x 16 lanes per set. `vec`
//! runs them on the AVX-512 lockstep engine, `ref` on the scalar reference engine; results are
//! identical, and the same for any thread count.
use bj::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

fn report(s: &Stats) {
    let n = s.rounds as f64;
    let (net, init, act) = (s.net as f64 / 2.0, s.initial_bet as f64 / 2.0, s.action as f64 / 2.0);
    let mean = net / n;
    let sd = (s.net_sq as f64 / 4.0 / n - mean * mean).sqrt();
    let fmean = s.flat_net as f64 / 2.0 / n;
    let fsd = (s.flat_sq as f64 / 4.0 / n - fmean * fmean).sqrt();
    let avg_bet = init / n;
    let h = s.hands as f64;
    println!("rounds              {}", s.rounds);
    println!("hands               {}", s.hands);
    println!("shoes               {}", s.shuffles);
    println!("avg initial bet     {:.4} units", avg_bet);
    println!("total action        {:.1} units", act);
    println!("net result          {:+.1} units", net);
    println!("EV per round        {:+.5} units (SD {:.4})", mean, sd);
    println!(
        "edge vs initial bet {:+.4}% +/- {:.4}% (95% CI)",
        100.0 * mean / avg_bet,
        100.0 * 1.96 * sd / n.sqrt() / avg_bet
    );
    println!("edge vs total action{:+.4}%", 100.0 * net / act);
    println!(
        "flat-bet edge       {:+.4}% +/- {:.4}% (play only, 1 unit every round)",
        100.0 * fmean,
        100.0 * 1.96 * fsd / n.sqrt()
    );
    println!(
        "win/loss/push       {:.4} / {:.4} / {:.4} per hand",
        s.wins as f64 / h,
        s.losses as f64 / h,
        s.pushes as f64 / h
    );
    println!(
        "blackjacks          player {:.4}%  dealer {:.4}%",
        100.0 * s.player_bj as f64 / n,
        100.0 * s.dealer_bj as f64 / n
    );
    println!("doubles/splits/surr {} / {} / {}", s.doubles, s.splits, s.surrenders);
    println!("insurance           taken {}  won {}", s.ins_taken, s.ins_won);
    println!("true count  %rounds   flat EV%");
    for i in 0..TC_BUCKETS {
        if s.tc_rounds[i] == 0 {
            continue;
        }
        let tc = i as i32 + TC_MIN;
        let pre = if tc == TC_MIN { "<=" } else if tc == TC_MAX { ">=" } else { "  " };
        println!(
            "{}{:+3}    {:8.4}  {:+8.3}",
            pre,
            tc,
            100.0 * s.tc_rounds[i] as f64 / n,
            100.0 * s.tc_flat_net[i] as f64 / 2.0 / s.tc_rounds[i] as f64
        );
    }
}

const GROUPS: usize = 3; // 16-lane groups interleaved per set (3 measured fastest)
const NLANES: usize = GROUPS * 16;
const SETS: u64 = 128; // independent lane sets: the unit of work handed to threads

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rounds: u64 = args.get(1).map_or(100_000_000, |a| a.parse().expect("rounds"));
    let threads: usize = args
        .get(2)
        .map_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()), |a| a.parse().expect("threads"))
        .max(1);
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("seed"));
    let counting = args.get(4).map_or(true, |m| m != "basic");
    let reference = args.get(5).is_some_and(|m| m == "ref") || !cfg!(target_feature = "avx512f");

    // The work is split into SETS independent lane sets with fixed seeds and round counts, handed
    // out to threads dynamically, so a core slowed by other load doesn't hold up the run. Each
    // set's result doesn't depend on which thread runs it, so output is the same for any thread
    // count.
    let next = AtomicU64::new(0);
    let t0 = Instant::now();
    let total = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut total = Stats::default();
                    loop {
                        let set = next.fetch_add(1, Ordering::Relaxed);
                        if set >= SETS {
                            break;
                        }
                        let n = rounds / SETS + u64::from(set < rounds % SETS);
                        let mut seeder = set_seed(seed, set);
                        let mut lanes: Vec<Sim> = (0..NLANES).map(|_| Sim::new(splitmix64(&mut seeder), counting)).collect();
                        let (per, extra) = (n / NLANES as u64, (n % NLANES as u64) as usize);
                        if reference {
                            for (l, sim) in lanes.iter_mut().enumerate() {
                                for _ in 0..per + u64::from(l < extra) {
                                    sim.play_round();
                                }
                            }
                        } else {
                            #[cfg(target_feature = "avx512f")]
                            bj::engine::Engine::<GROUPS>::new(&mut lanes, counting).run(per);
                            for sim in &mut lanes[..extra] {
                                sim.play_round();
                            }
                        }
                        for sim in &lanes {
                            total.merge(&sim.st);
                        }
                    }
                    total
                })
            })
            .collect();
        let mut total = Stats::default();
        for h in handles {
            total.merge(&h.join().unwrap());
        }
        total
    });
    let secs = t0.elapsed().as_secs_f64();

    println!(
        "mode                {}",
        if counting { "hi-lo (I18 + Fab 4, 1-12 spread)" } else { "basic strategy, flat bet" }
    );
    report(&total);
    println!(
        "time: Rust {}, {} thread(s) {:.3} s, {:.1} M rounds/s, {} bytes/lane",
        if reference { "scalar" } else { "avx512" },
        threads,
        secs,
        rounds as f64 / secs / 1e6,
        std::mem::size_of::<Sim>()
    );
}
