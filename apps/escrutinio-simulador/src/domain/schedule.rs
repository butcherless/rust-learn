//! Cuándo llega cada mesa (requisitos de escrutinio, «Simulador» y «Carga»).
//!
//! El fichero de Interior no dice cuándo llegó cada mesa, así que `schedule` lo inventa: las mesas
//! con menos censo tienden a llegar antes, y la llegada sigue la curva de `arrival`. Cada mesa tiene
//! su hueco en la curva y llega en un punto al azar dentro de él, hasta medio hueco antes o después
//! de su centro: los intervalos varían entre 0 y el doble del medio, con ráfagas y pausas, pero
//! ninguna mesa adelanta a la siguiente y los tramos de la curva se mantienen. Con la misma semilla
//! sale el mismo plan.

use std::time::Duration;

use super::station::Station;

/// Una mesa y el momento en que se publica, contado desde el inicio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    pub at: Duration,
    pub station: Station,
}

/// Los puntos de la curva de llegada: (fracción de mesas, fracción del tiempo). El 50 % llega en
/// el primer cuarto del tiempo, el 40 % en la mitad siguiente y el 10 % en el último cuarto.
const CURVE: [(f64, f64); 4] = [(0.0, 0.0), (0.5, 0.25), (0.9, 0.75), (1.0, 1.0)];

/// La fracción del tiempo en que llega la fracción `share` de las mesas (0–1), interpolando la
/// curva.
pub fn arrival(share: f64) -> f64 {
    let share = share.clamp(0.0, 1.0);
    CURVE
        .windows(2)
        .find(|pair| share <= pair[1].0)
        .map_or(1.0, |pair| {
            let ((s0, t0), (s1, t1)) = (pair[0], pair[1]);
            t0 + (share - s0) / (s1 - s0) * (t1 - t0)
        })
}

/// El plan de publicación, ordenado por momento de llegada.
pub fn schedule(stations: Vec<Station>, duration: Duration, seed: u64) -> Vec<Scheduled> {
    let total = stations.len();
    let (rng, scored) = stations.into_iter().fold(
        (Rng::new(seed), Vec::with_capacity(total)),
        |(rng, mut scored), station| {
            let (factor, rng) = rng.next_unit();
            // Entre la mitad y vez y media de su censo: el censo pesa, pero no decide solo.
            scored.push((f64::from(station.census) * (0.5 + factor), station));
            (rng, scored)
        },
    );
    let mut scored = scored;
    scored.sort_by(|(a, sa), (b, sb)| a.total_cmp(b).then_with(|| sa.key.cmp(&sb.key)));
    let (_, plan) = scored.into_iter().enumerate().fold(
        (rng, Vec::with_capacity(total)),
        |(rng, mut plan), (index, (_, station))| {
            // Un punto al azar dentro del hueco `index`: [index, index + 1) / total.
            let (offset, rng) = rng.next_unit();
            let share = (index as f64 + offset) / total as f64;
            plan.push(Scheduled {
                at: duration.mul_f64(arrival(share)),
                station,
            });
            (rng, plan)
        },
    );
    plan
}

/// Xorshift64*: sin dependencias y determinista. Cada paso devuelve el generador siguiente.
#[derive(Debug, Clone, Copy)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // El estado no puede ser 0.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    /// Un número en [0, 1) y el generador siguiente.
    fn next_unit(self) -> (f64, Self) {
        let x = self.0 ^ (self.0 >> 12);
        let x = x ^ (x << 25);
        let x = x ^ (x >> 27);
        let value = x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11;
        (value as f64 / (1u64 << 53) as f64, Self(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::station::fixtures::{key, row};
    use crate::domain::station::{CandidacyVotes, Station};

    fn station(index: u32, census: u32) -> Station {
        let row = row(key("51", &format!("{index:04}")));
        Station {
            key: row.key,
            census,
            blank: 0,
            invalid: 0,
            votes: vec![CandidacyVotes {
                candidacy: 1,
                votes: 0,
            }],
        }
    }

    fn stations(count: u32) -> Vec<Station> {
        (0..count).map(|i| station(i, 100 + i * 7 % 900)).collect()
    }

    #[test]
    fn the_curve_goes_through_its_points_and_interpolates() {
        [
            (0.0, 0.0),
            (0.25, 0.125),
            (0.5, 0.25),
            (0.7, 0.5),
            (0.9, 0.75),
            (0.95, 0.875),
            (1.0, 1.0),
            (-1.0, 0.0),
            (2.0, 1.0),
        ]
        .into_iter()
        .for_each(|(share, time)| assert!((arrival(share) - time).abs() < 1e-12, "{share}"));
    }

    #[test]
    fn the_plan_is_sorted_within_the_duration_and_keeps_every_station() {
        let duration = Duration::from_secs(7200);
        let plan = schedule(stations(1000), duration, 7);
        assert_eq!(plan.len(), 1000);
        assert!(plan.windows(2).all(|pair| pair[0].at <= pair[1].at));
        assert!(plan.iter().all(|s| s.at < duration));
    }

    #[test]
    fn half_the_stations_arrive_in_the_first_quarter() {
        let duration = Duration::from_secs(7200);
        let plan = schedule(stations(1000), duration, 7);
        let early = plan.iter().filter(|s| s.at < duration / 4).count();
        assert_eq!(early, 500);
    }

    #[test]
    fn smaller_stations_tend_to_arrive_earlier() {
        let plan = schedule(stations(1000), Duration::from_secs(60), 7);
        let mean = |part: &[Scheduled]| {
            part.iter()
                .map(|s| f64::from(s.station.census))
                .sum::<f64>()
                / part.len() as f64
        };
        assert!(mean(&plan[..500]) < mean(&plan[500..]));
    }

    #[test]
    fn the_intervals_vary_around_their_mean() {
        // 500 mesas en el primer cuarto de 1.000 s: de media, una cada 0,5 s.
        let plan = schedule(stations(1000), Duration::from_secs(1000), 7);
        let gaps: Vec<f64> = plan[..500]
            .windows(2)
            .map(|pair| (pair[1].at - pair[0].at).as_secs_f64())
            .collect();
        let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean {mean}");
        assert!(gaps.iter().any(|&gap| gap < 0.15), "no burst");
        assert!(gaps.iter().any(|&gap| gap > 0.85), "no pause");
        assert!(
            gaps.iter().all(|&gap| gap < 1.0),
            "a station outside its slot"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_plan_and_another_seed_another() {
        let plan = |seed| schedule(stations(200), Duration::from_secs(60), seed);
        assert_eq!(plan(1), plan(1));
        assert_ne!(plan(1), plan(2));
    }

    #[test]
    fn an_empty_list_gives_an_empty_plan() {
        assert!(schedule(vec![], Duration::from_secs(60), 1).is_empty());
    }
}
