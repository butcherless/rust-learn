//! El snapshot que sirve la API (requisitos de escrutinio, RN-12 y RN-13): aquí se calcula todo lo
//! que pintan la TUI y la web, para que no tengan que sumar ni comparar nada.
//!
//! De PostgreSQL llegan los totales de las provincias que tienen alguna mesa, válida o rechazada
//! (`ProvinceTotals`). `snapshots` los junta con las 52 circunscripciones del contrato, en el orden
//! de la tabla: las que no tienen nada salen con ceros. Una provincia está completa cuando se han
//! escrutado todas las mesas posibles, es decir, todas menos las rechazadas:
//! `stations + rejected >= expected`. Su piloto (`pulse`) es verde si está completa, naranja si
//! recibió una mesa en los últimos `RECENT_SECS` segundos, azul si tiene mesas y ninguna reciente,
//! y gris si no tiene ninguna.

use escrutinio_contrato::districts::{self, DISTRICTS, District};
use escrutinio_contrato::{CandidacyTotal, ProvinceSnapshot, Pulse, Snapshots, Total};

/// Cuánto dura el naranja del piloto: una consulta cada 10 s ve el cambio una o dos veces.
pub const RECENT_SECS: u64 = 15;

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
    /// Segundos desde la última mesa válida; `None` si no tiene ninguna.
    pub seconds_since_last: Option<u64>,
    /// De más a menos votos.
    pub votes: Vec<CandidacyTotal>,
}

/// El piloto de una circunscripción (RN-12); `complete` manda sobre todo.
fn pulse(stations: u32, complete: bool, seconds_since_last: Option<u64>) -> Pulse {
    if complete {
        Pulse::Complete
    } else if stations == 0 {
        Pulse::NoData
    } else if seconds_since_last.is_some_and(|seconds| seconds < RECENT_SECS) {
        Pulse::Updated
    } else {
        Pulse::Steady
    }
}

/// El piloto del total mientras no esté completo: naranja si alguna circunscripción lo está, gris
/// si ninguna tiene mesas y azul en el resto de casos.
fn total_pulse(complete: bool, provinces: &[ProvinceSnapshot]) -> Pulse {
    let any = |wanted: Pulse| provinces.iter().any(|p| p.pulse == wanted);
    if complete {
        Pulse::Complete
    } else if any(Pulse::Updated) {
        Pulse::Updated
    } else if any(Pulse::Steady) || any(Pulse::Complete) {
        Pulse::Steady
    } else {
        Pulse::NoData
    }
}

/// El snapshot de `district` con lo que se sepa de ella.
pub fn province_snapshot(district: &District, totals: ProvinceTotals) -> ProvinceSnapshot {
    let complete = district.stations > 0 && totals.stations + totals.rejected >= district.stations;
    ProvinceSnapshot {
        province: district.code.to_owned(),
        name: district.name.to_owned(),
        stations: totals.stations,
        possible: district.stations.saturating_sub(totals.rejected),
        rejected: totals.rejected,
        complete,
        census: totals.census,
        electorate: district.electorate,
        estimated: districts::estimated(district.electorate),
        voters: totals.voters,
        blank: totals.blank,
        invalid: totals.invalid,
        pulse: pulse(totals.stations, complete, totals.seconds_since_last),
        votes: totals.votes,
    }
}

/// El total de España: la suma de las circunscripciones, completo si todas lo están.
pub fn total(provinces: &[ProvinceSnapshot]) -> Total {
    let complete = !provinces.is_empty() && provinces.iter().all(|p| p.complete);
    let electorate = provinces.iter().map(|p| p.electorate).sum();
    Total {
        stations: provinces.iter().map(|p| p.stations).sum(),
        possible: provinces.iter().map(|p| p.possible).sum(),
        rejected: provinces.iter().map(|p| p.rejected).sum(),
        complete,
        voters: provinces.iter().map(|p| p.voters).sum(),
        electorate,
        estimated: provinces.iter().map(|p| p.estimated).sum(),
        pulse: total_pulse(complete, provinces),
    }
}

/// Las 52 circunscripciones en el orden de la tabla, con `totals` de las que tienen datos, y el
/// total. Lo de una provincia que no es una de las 52 se ignora.
pub fn snapshots(totals: Vec<ProvinceTotals>) -> Snapshots {
    let provinces: Vec<ProvinceSnapshot> = DISTRICTS
        .iter()
        .map(|district| {
            let found = totals
                .iter()
                .find(|t| t.province == district.code)
                .cloned()
                .unwrap_or_default();
            province_snapshot(district, found)
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

    /// Lo acumulado de `province` con `stations` válidas y `rejected` rechazadas, la última hace una
    /// hora.
    pub fn totals(province: &str, stations: u32, rejected: u32) -> ProvinceTotals {
        ProvinceTotals {
            province: province.to_owned(),
            stations,
            rejected,
            census: u64::from(stations) * 100,
            voters: u64::from(stations) * 53,
            blank: u64::from(stations),
            invalid: u64::from(stations) * 2,
            seconds_since_last: (stations > 0).then_some(3_600),
            votes: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use escrutinio_contrato::districts::{TOTAL_ELECTORATE, TOTAL_STATIONS, find};

    use super::fixtures::totals;
    use super::*;

    fn province<'a>(snapshots: &'a Snapshots, code: &str) -> &'a ProvinceSnapshot {
        snapshots
            .provinces
            .iter()
            .find(|p| p.province == code)
            .unwrap_or_else(|| panic!("province {code}"))
    }

    fn snapshot_of(code: &str, totals: ProvinceTotals) -> ProvinceSnapshot {
        province_snapshot(find(code).unwrap_or(&DISTRICTS[0]), totals)
    }

    #[test]
    fn always_has_the_52_districts_in_the_table_order_even_with_no_data() {
        let snapshots = snapshots(vec![]);
        assert_eq!(snapshots.provinces.len(), 52);
        assert_eq!(
            snapshots
                .provinces
                .iter()
                .map(|p| p.name.as_str())
                .take(2)
                .collect::<Vec<_>>(),
            ["A Coruña", "Albacete"]
        );
        assert_eq!(
            snapshots.total,
            Total {
                stations: 0,
                possible: TOTAL_STATIONS,
                rejected: 0,
                complete: false,
                voters: 0,
                electorate: TOTAL_ELECTORATE,
                estimated: 24_598_816,
                pulse: Pulse::NoData,
            }
        );
        let leon = province(&snapshots, "24");
        assert_eq!(
            (leon.name.as_str(), leon.possible, leon.electorate),
            ("León", 722, 374_272)
        );
        assert_eq!(leon.estimated, 261_990);
        assert!(
            snapshots
                .provinces
                .iter()
                .all(|p| !p.complete && p.stations == 0 && p.pulse == Pulse::NoData)
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
            let snapshot = snapshot_of("24", totals("24", stations, rejected));
            assert_eq!(
                (snapshot.possible, snapshot.complete),
                (possible, complete),
                "{case}"
            );
        });
        let empty = District {
            code: "00",
            name: "Nada",
            stations: 0,
            electorate: 0,
        };
        assert!(!province_snapshot(&empty, ProvinceTotals::default()).complete);
    }

    #[test]
    fn the_pulse_says_what_happened_to_the_province_lately() {
        let ago = |seconds| ProvinceTotals {
            seconds_since_last: seconds,
            ..totals("28", 100, 0)
        };
        [
            ("no stations", totals("28", 0, 0), Pulse::NoData),
            ("only rejected", totals("28", 0, 2), Pulse::NoData),
            ("just now", ago(Some(0)), Pulse::Updated),
            ("14 s ago", ago(Some(RECENT_SECS - 1)), Pulse::Updated),
            ("15 s ago", ago(Some(RECENT_SECS)), Pulse::Steady),
            ("an hour ago", ago(Some(3_600)), Pulse::Steady),
            ("no time known", ago(None), Pulse::Steady),
        ]
        .into_iter()
        .for_each(|(case, totals, expected)| {
            assert_eq!(snapshot_of("28", totals).pulse, expected, "{case}");
        });
        let complete = ProvinceTotals {
            seconds_since_last: Some(0),
            ..totals("28", 7_118, 0)
        };
        assert_eq!(snapshot_of("28", complete).pulse, Pulse::Complete);
    }

    #[test]
    fn the_total_pulse_follows_its_districts() {
        let recent = |code, stations| ProvinceTotals {
            seconds_since_last: Some(1),
            ..totals(code, stations, 0)
        };
        [
            ("nothing", vec![], Pulse::NoData),
            ("all steady", vec![totals("28", 10, 0)], Pulse::Steady),
            (
                "one updated",
                vec![recent("28", 10), totals("08", 5, 0)],
                Pulse::Updated,
            ),
            ("a complete one only", vec![recent("51", 96)], Pulse::Steady),
        ]
        .into_iter()
        .for_each(|(case, totals, expected)| {
            assert_eq!(snapshots(totals).total.pulse, expected, "{case}");
        });
    }

    #[test]
    fn the_total_adds_up_and_is_complete_only_when_every_district_is() {
        let partial = snapshots(vec![totals("24", 700, 4), totals("28", 7_118, 0)]);
        assert_eq!(
            partial.total,
            Total {
                stations: 7_818,
                possible: TOTAL_STATIONS - 4,
                rejected: 4,
                complete: false,
                voters: 7_818 * 53,
                electorate: TOTAL_ELECTORATE,
                estimated: 24_598_816,
                pulse: Pulse::Steady,
            }
        );
        assert!(province(&partial, "28").complete);
        assert!(!province(&partial, "24").complete);
        let all = snapshots(
            DISTRICTS
                .iter()
                .map(|district| {
                    let rejected = if district.code == "24" { 4 } else { 0 };
                    totals(district.code, district.stations - rejected, rejected)
                })
                .collect(),
        );
        assert!(all.total.complete);
        assert_eq!(all.total.pulse, Pulse::Complete);
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
