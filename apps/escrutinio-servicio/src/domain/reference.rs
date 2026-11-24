//! Las mesas de cada circunscripción (requisitos de escrutinio, RN-13): el N con el que el servicio
//! sabe cuándo se han escrutado todas las posibles.
//!
//! Son las de las Elecciones al Congreso del 23 de julio de 2023, del fichero de mesas de Interior
//! (`09022307.DAT`, sin el CERA): 60.314 en las 52 circunscripciones, por código de provincia del
//! INE. El servicio solo sirve ese escrutinio; con los datos de 2026 habrá que elegir la tabla.

/// El total de mesas de España.
pub const TOTAL_2023: u32 = 60_314;

/// Las mesas de cada provincia en 2023, por orden de código.
pub const STATIONS_2023: [(&str, u32); 52] = [
    ("01", 427),
    ("02", 521),
    ("03", 2_227),
    ("04", 837),
    ("05", 358),
    ("06", 902),
    ("07", 1_249),
    ("08", 6_472),
    ("09", 746),
    ("10", 524),
    ("11", 1_528),
    ("12", 740),
    ("13", 707),
    ("14", 948),
    ("15", 1_544),
    ("16", 383),
    ("17", 863),
    ("18", 1_121),
    ("19", 587),
    ("20", 858),
    ("21", 651),
    ("22", 403),
    ("23", 873),
    ("24", 722),
    ("25", 549),
    ("26", 516),
    ("27", 533),
    ("28", 7_118),
    ("29", 1_734),
    ("30", 1_726),
    ("31", 997),
    ("32", 539),
    ("33", 1_528),
    ("34", 335),
    ("35", 1_564),
    ("36", 1_344),
    ("37", 648),
    ("38", 1_379),
    ("39", 777),
    ("40", 330),
    ("41", 2_588),
    ("42", 256),
    ("43", 1_039),
    ("44", 328),
    ("45", 903),
    ("46", 3_200),
    ("47", 763),
    ("48", 1_410),
    ("49", 369),
    ("50", 1_467),
    ("51", 96),
    ("52", 87),
];

/// Las mesas de la provincia `code`, o `None` si no es una de las 52 circunscripciones.
pub fn expected(code: &str) -> Option<u32> {
    STATIONS_2023
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|&(_, stations)| stations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_52_districts_add_up_to_the_stations_of_2023() {
        assert_eq!(
            STATIONS_2023.iter().map(|&(_, n)| n).sum::<u32>(),
            TOTAL_2023
        );
        assert!(STATIONS_2023.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn finds_a_province_or_nothing() {
        assert_eq!(expected("28"), Some(7_118));
        assert_eq!(expected("24"), Some(722));
        assert_eq!(expected("99"), None);
        assert_eq!(expected(""), None);
    }
}
