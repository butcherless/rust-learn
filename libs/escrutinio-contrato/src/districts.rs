//! Las 52 circunscripciones del escrutinio de 2023 (requisitos de escrutinio, RN-10 y RN-13): su
//! nombre, sus mesas y su censo. Es el N con el que el servicio sabe cuándo una circunscripción está
//! completa y lo que pintan la TUI y la web.
//!
//! Las mesas y el censo salen del fichero de mesas de Interior (`09022307.DAT`, sin el CERA). Van en
//! el orden de la tabla (RN-03): las provincias por nombre, sin tildes ni mayúsculas y con la `ñ`
//! detrás de la `n`, y después las ciudades autónomas.

/// Una circunscripción: su código de provincia del INE y sus datos de 2023.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct District {
    pub code: &'static str,
    /// El nombre oficial, con el artículo delante (`A Coruña`).
    pub name: &'static str,
    /// Las mesas del Congreso en 2023.
    pub stations: u32,
    /// El censo de escrutinio de esas mesas.
    pub electorate: u64,
}

const fn district(
    code: &'static str,
    name: &'static str,
    stations: u32,
    electorate: u64,
) -> District {
    District {
        code,
        name,
        stations,
        electorate,
    }
}

/// El total de mesas de España.
pub const TOTAL_STATIONS: u32 = 60_314;
/// El total del censo de escrutinio de las mesas.
pub const TOTAL_ELECTORATE: u64 = 35_141_198;
/// La participación que se estima por defecto, en % del censo (RN-04).
pub const ESTIMATED_TURNOUT: u64 = 70;

/// Las 50 provincias y las 2 ciudades autónomas, en el orden de la tabla.
pub const DISTRICTS: [District; 52] = [
    district("15", "A Coruña", 1_544, 922_306),
    district("02", "Albacete", 521, 299_924),
    district("03", "Alicante / Alacant", 2_227, 1_261_010),
    district("04", "Almería", 837, 471_427),
    district("01", "Araba/Álava", 427, 252_058),
    district("33", "Asturias", 1_528, 835_593),
    district("05", "Ávila", 358, 128_300),
    district("06", "Badajoz", 902, 538_574),
    district("08", "Barcelona", 6_472, 4_025_818),
    district("48", "Bizkaia", 1_410, 900_563),
    district("09", "Burgos", 746, 277_537),
    district("10", "Cáceres", 524, 322_765),
    district("11", "Cádiz", 1_528, 986_857),
    district("39", "Cantabria", 777, 466_589),
    district("12", "Castellón / Castelló", 740, 419_903),
    district("13", "Ciudad Real", 707, 384_479),
    district("14", "Córdoba", 948, 626_123),
    district("16", "Cuenca", 383, 149_438),
    district("20", "Gipuzkoa", 858, 556_075),
    district("17", "Girona", 863, 522_354),
    district("18", "Granada", 1_121, 712_178),
    district("19", "Guadalajara", 587, 191_796),
    district("21", "Huelva", 651, 394_310),
    district("22", "Huesca", 403, 165_249),
    district("07", "Illes Balears", 1_249, 795_915),
    district("23", "Jaén", 873, 504_825),
    district("26", "La Rioja", 516, 234_383),
    district("35", "Las Palmas", 1_564, 843_241),
    district("24", "León", 722, 374_272),
    district("25", "Lleida", 549, 298_948),
    district("27", "Lugo", 533, 268_657),
    district("28", "Madrid", 7_118, 4_848_057),
    district("29", "Málaga", 1_734, 1_178_993),
    district("30", "Murcia", 1_726, 1_061_943),
    district("31", "Navarra", 997, 489_978),
    district("32", "Ourense", 539, 252_827),
    district("34", "Palencia", 335, 130_129),
    district("36", "Pontevedra", 1_344, 772_695),
    district("37", "Salamanca", 648, 268_868),
    district("38", "Santa Cruz de Tenerife", 1_379, 774_004),
    district("40", "Segovia", 330, 115_809),
    district("41", "Sevilla", 2_588, 1_525_548),
    district("42", "Soria", 256, 68_463),
    district("43", "Tarragona", 1_039, 575_113),
    district("44", "Teruel", 328, 101_249),
    district("45", "Toledo", 903, 529_311),
    district("46", "Valencia / València", 3_200, 1_928_949),
    district("47", "Valladolid", 763, 416_646),
    district("49", "Zamora", 369, 142_159),
    district("50", "Zaragoza", 1_467, 713_505),
    district("51", "Ceuta", 96, 60_335),
    district("52", "Melilla", 87, 55_150),
];

/// La circunscripción con el código de provincia `code`, o `None` si no es una de las 52.
pub fn find(code: &str) -> Option<&'static District> {
    DISTRICTS.iter().find(|district| district.code == code)
}

/// Los votos que se estiman en una circunscripción de `electorate` electores, redondeados hacia abajo.
pub fn estimated(electorate: u64) -> u64 {
    electorate * ESTIMATED_TURNOUT / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_52_districts_add_up_to_the_totals_of_2023() {
        assert_eq!(
            DISTRICTS.iter().map(|d| d.stations).sum::<u32>(),
            TOTAL_STATIONS
        );
        assert_eq!(
            DISTRICTS.iter().map(|d| d.electorate).sum::<u64>(),
            TOTAL_ELECTORATE
        );
        let mut codes: Vec<_> = DISTRICTS.iter().map(|d| d.code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 52);
    }

    #[test]
    fn the_provinces_come_first_and_the_autonomous_cities_last() {
        let names: Vec<_> = DISTRICTS.iter().map(|d| d.name).collect();
        assert_eq!(names[..3], ["A Coruña", "Albacete", "Alicante / Alacant"]);
        assert_eq!(names[48..], ["Zamora", "Zaragoza", "Ceuta", "Melilla"]);
    }

    #[test]
    fn finds_a_district_or_nothing() {
        assert_eq!(
            find("24").map(|d| (d.name, d.stations)),
            Some(("León", 722))
        );
        assert_eq!(find("28").map(|d| d.electorate), Some(4_848_057));
        assert_eq!(find("99"), None);
        assert_eq!(find(""), None);
    }

    #[test]
    fn the_estimate_is_the_turnout_of_the_electorate_rounded_down() {
        assert_eq!(estimated(68_463), 47_924);
        assert_eq!(estimated(99), 69);
    }
}
