//! Usage: bj [rounds=100000000] [threads=all] [seed=1] [hilo|basic]
use bj::*;
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rounds: u64 = args.get(1).map_or(100_000_000, |a| a.parse().expect("rounds"));
    let threads: u64 = args
        .get(2)
        .map_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get() as u64), |a| a.parse().expect("threads"))
        .max(1);
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("seed"));
    let counting = args.get(4).map_or(true, |m| m != "basic");

    let mut seeder = seed;
    let t0 = Instant::now();
    let total = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let n = rounds / threads + u64::from(t < rounds % threads);
                let s = splitmix64(&mut seeder);
                scope.spawn(move || {
                    let mut sim = Sim::new(s, counting);
                    for _ in 0..n {
                        sim.play_round();
                    }
                    sim.st
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
        "time: Rust {} thread(s) {:.3} s, {:.1} M rounds/s, sim state {} bytes/thread",
        threads,
        secs,
        rounds as f64 / secs / 1e6,
        std::mem::size_of::<Sim>()
    );
}
