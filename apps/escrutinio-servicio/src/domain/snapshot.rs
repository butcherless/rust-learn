//! El snapshot que sirve la API (requisitos de escrutinio, RN-13): aquí se calcula todo lo que la
//! TUI pinta, para que no tenga que sumar ni comparar nada.
//!
//! De PostgreSQL llegan los totales de las provincias que tienen alguna mesa, válida o rechazada
//! (`ProvinceTotals`). `snapshots` los junta con las 52 circunscripciones de `reference`: las que no
//! tienen nada salen con ceros. Una provincia está completa cuando se han escrutado todas las
//! mesas posibles, es decir, todas menos las rechazadas: `stations + rejected >= expected`.

use escrutinio_contrato::{CandidacyTotal, ProvinceSnapshot, Snapshots, Total};

use super::reference::STATIONS_2023;

/// Lo acumulado de una provincia, tal como sale de PostgreSQL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvinceTotals {
    pub province: String,
    /// Mesas escrutadas, válidas.
    pub stations: u32,
    /// Mesas rechazadas.
    pub rejected: u32,
    pub census: u64,
    pub voters: u64,
    pub blank: u64,
    pub invalid: u64,
    /// De más a menos votos.
    pub votes: Vec<CandidacyTotal>,
}

/// El snapshot de `province`, que tiene `expected` mesas, con lo que se sepa de ella.
pub fn province_snapshot(
    province: &str,
    expected: u32,
    totals: ProvinceTotals,
) -> ProvinceSnapshot {
    ProvinceSnapshot {
        province: province.to_owned(),
        stations: totals.stations,
        possible: expected.saturating_sub(totals.rejected),
        rejected: totals.rejected,
        complete: expected > 0 && totals.stations + totals.rejected >= expected,
        census: totals.census,
        voters: totals.voters,
        blank: totals.blank,
        invalid: totals.invalid,
        votes: totals.votes,
    }
}

/// El total de España: la suma de las circunscripciones, completo si todas lo están.
pub fn total(provinces: &[ProvinceSnapshot]) -> Total {
    Total {
        stations: provinces.iter().map(|p| p.stations).sum(),
        possible: provinces.iter().map(|p| p.possible).sum(),
        rejected: provinces.iter().map(|p| p.rejected).sum(),
        complete: !provinces.is_empty() && provinces.iter().all(|p| p.complete),
        voters: provinces.iter().map(|p| p.voters).sum(),
    }
}

/// Las 52 circunscripciones por código de provincia, con `totals` de las que tienen datos, y el
/// total. Lo de una provincia que no es una de las 52 se ignora.
pub fn snapshots(totals: Vec<ProvinceTotals>) -> Snapshots {
    let provinces: Vec<ProvinceSnapshot> = STATIONS_2023
        .iter()
        .map(|&(code, expected)| {
            let found = totals
                .iter()
                .find(|t| t.province == code)
                .cloned()
                .unwrap_or_default();
            province_snapshot(code, expected, found)
        })
        .collect();
    Snapshots {
        total: total(&provinces),
        provinces,
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// Lo acumulado de `province` con `stations` válidas y `rejected` rechazadas.
    pub fn totals(province: &str, stations: u32, rejected: u32) -> ProvinceTotals {
        ProvinceTotals {
            province: province.to_owned(),
            stations,
            rejected,
            census: u64::from(stations) * 100,
            voters: u64::from(stations) * 53,
            blank: u64::from(stations),
            invalid: u64::from(stations) * 2,
            votes: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::totals;
    use super::*;
    use crate::domain::reference::TOTAL_2023;

    fn province<'a>(snapshots: &'a Snapshots, code: &str) -> &'a ProvinceSnapshot {
        snapshots
            .provinces
            .iter()
            .find(|p| p.province == code)
            .unwrap_or_else(|| panic!("province {code}"))
    }

    #[test]
    fn always_has_the_52_districts_in_order_even_with_no_data() {
        let snapshots = snapshots(vec![]);
        assert_eq!(snapshots.provinces.len(), 52);
        assert!(
            snapshots
                .provinces
                .windows(2)
                .all(|p| p[0].province < p[1].province)
        );
        assert_eq!(
            snapshots.total,
            Total {
                stations: 0,
                possible: TOTAL_2023,
                rejected: 0,
                complete: false,
                voters: 0
            }
        );
        assert_eq!(province(&snapshots, "24").possible, 722);
        assert!(
            snapshots
                .provinces
                .iter()
                .all(|p| !p.complete && p.stations == 0)
        );
    }

    #[test]
    fn a_province_is_complete_when_every_possible_station_is_counted() {
        // León tiene 722 mesas; con 4 rechazadas, las posibles son 718.
        [
            ("nothing yet", 0, 0, 722, false),
            ("some", 300, 2, 720, false),
            ("all valid but the rejects not seen yet", 718, 0, 722, false),
            ("all valid and 3 of 4 rejects", 718, 3, 719, false),
            ("every possible one", 718, 4, 718, true),
            ("more than expected", 719, 4, 718, true),
        ]
        .into_iter()
        .for_each(|(case, stations, rejected, possible, complete)| {
            let snapshot = province_snapshot("24", 722, totals("24", stations, rejected));
            assert_eq!(
                (snapshot.possible, snapshot.complete),
                (possible, complete),
                "{case}"
            );
        });
        assert!(!province_snapshot("00", 0, ProvinceTotals::default()).complete);
    }

    #[test]
    fn the_total_adds_up_and_is_complete_only_when_every_district_is() {
        let partial = snapshots(vec![totals("24", 700, 4), totals("28", 7_118, 0)]);
        assert_eq!(
            partial.total,
            Total {
                stations: 7_818,
                possible: TOTAL_2023 - 4,
                rejected: 4,
                complete: false,
                voters: 7_818 * 53,
            }
        );
        assert!(province(&partial, "28").complete);
        assert!(!province(&partial, "24").complete);
        let all = snapshots(
            super::super::reference::STATIONS_2023
                .iter()
                .map(|&(code, expected)| {
                    let rejected = if code == "24" { 4 } else { 0 };
                    totals(code, expected - rejected, rejected)
                })
                .collect(),
        );
        assert!(all.total.complete);
        assert_eq!(
            (all.total.stations, all.total.possible, all.total.rejected),
            (60_310, 60_310, 4)
        );
    }

    #[test]
    fn ignores_a_province_that_is_not_one_of_the_52() {
        let snapshots = snapshots(vec![totals("99", 5, 0)]);
        assert_eq!(snapshots.total.stations, 0);
        assert!(snapshots.provinces.iter().all(|p| p.province != "99"));
    }

    #[test]
    fn an_empty_list_of_districts_has_an_incomplete_total() {
        assert!(!total(&[]).complete);
    }
}
