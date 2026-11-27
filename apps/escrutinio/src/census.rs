//! El censo de cada `District` (RN-01, RN-02, RN-10).
//!
//! Son los electores españoles residentes en España (CER) de dos elecciones:
//! - 2026, la maqueta: las listas de votación de las Elecciones a Cortes Generales del 29 de
//!   noviembre de 2026, de la Oficina del Censo Electoral del INE: tabla
//!   `elecgral_2026_lv_tab3_mod.xlsx`, ya modificada por los apéndices de reclamaciones y los autos
//!   del Tribunal Supremo de septiembre y octubre de 2026.
//! - 2023, el escrutinio que reproduce el servicio: la suma del censo de escrutinio de las 60.314
//!   mesas del Congreso del 23 de julio de 2023 (fichero `09022307.DAT` de Interior), sin el CERA.
//!
//! El código es el de provincia del INE, el que usan los mensajes de Kafka y los snapshots. Los
//! nombres son los oficiales, con el artículo delante (`A Coruña`, no `Coruña, A`).
//!
//! `ordered` da el orden de la tabla (RN-03) con el censo de una elección.

use crate::domain::sort_key;

/// El tipo de circunscripción. Las provincias van antes que las ciudades autónomas (RN-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Territory {
    Province,
    AutonomousCity,
}

/// De qué elecciones es el censo (RN-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Election {
    /// Las del 29 de noviembre de 2026: la maqueta.
    Generales2026,
    /// Las del 23 de julio de 2023: el escrutinio que reproduce el servicio.
    Generales2023,
}

impl Election {
    pub fn year(self) -> u16 {
        match self {
            Self::Generales2026 => 2026,
            Self::Generales2023 => 2023,
        }
    }
}

/// Una provincia o ciudad autónoma, con su censo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct District {
    /// El código de provincia del INE: `"28"`.
    pub code: &'static str,
    pub name: &'static str,
    pub territory: Territory,
    /// Electores CER de la elección que se muestra.
    pub census: u64,
    /// Electores CER de 2023.
    census_2023: u64,
}

impl District {
    /// La misma circunscripción con el censo de `election`.
    pub fn for_election(self, election: Election) -> Self {
        match election {
            Election::Generales2026 => self,
            Election::Generales2023 => Self {
                census: self.census_2023,
                ..self
            },
        }
    }
}

/// El total nacional del CER de 2026 que publica el INE, para comprobar los datos.
pub const TOTAL_CENSUS: u64 = 35_953_542;
/// El total del censo de escrutinio de las mesas de 2023, para comprobar los datos.
pub const TOTAL_CENSUS_2023: u64 = 35_141_198;

/// Una provincia con su censo CER de 2026 y de 2023.
const fn province(
    code: &'static str,
    name: &'static str,
    census: u64,
    census_2023: u64,
) -> District {
    District {
        code,
        name,
        territory: Territory::Province,
        census,
        census_2023,
    }
}

/// Una ciudad autónoma con su censo CER de 2026 y de 2023.
const fn city(code: &'static str, name: &'static str, census: u64, census_2023: u64) -> District {
    District {
        code,
        name,
        territory: Territory::AutonomousCity,
        census,
        census_2023,
    }
}

/// Las 50 provincias y las 2 ciudades autónomas, en el orden de la tabla del INE.
pub const DISTRICTS: [District; 52] = [
    province("02", "Albacete", 299_550, 299_924),
    province("03", "Alicante / Alacant", 1_303_575, 1_261_010),
    province("04", "Almería", 487_775, 471_427),
    province("01", "Araba/Álava", 256_960, 252_058),
    province("33", "Asturias", 835_351, 835_593),
    province("05", "Ávila", 127_004, 128_300),
    province("06", "Badajoz", 538_937, 538_574),
    province("07", "Illes Balears", 828_523, 795_915),
    province("08", "Barcelona", 4_132_494, 4_025_818),
    province("48", "Bizkaia", 909_176, 900_563),
    province("09", "Burgos", 278_214, 277_537),
    province("10", "Cáceres", 321_039, 322_765),
    province("11", "Cádiz", 1_007_030, 986_857),
    province("39", "Cantabria", 473_141, 466_589),
    province("12", "Castellón / Castelló", 435_013, 419_903),
    province("13", "Ciudad Real", 385_578, 384_479),
    province("14", "Córdoba", 627_827, 626_123),
    province("15", "A Coruña", 928_060, 922_306),
    province("16", "Cuenca", 147_724, 149_438),
    province("20", "Gipuzkoa", 561_839, 556_075),
    province("17", "Girona", 545_015, 522_354),
    province("18", "Granada", 728_577, 712_178),
    province("19", "Guadalajara", 202_513, 191_796),
    province("21", "Huelva", 399_651, 394_310),
    province("22", "Huesca", 166_250, 165_249),
    province("23", "Jaén", 500_508, 504_825),
    province("24", "León", 370_867, 374_272),
    province("25", "Lleida", 304_354, 298_948),
    province("27", "Lugo", 264_082, 268_657),
    province("28", "Madrid", 5_036_558, 4_848_057),
    province("29", "Málaga", 1_218_788, 1_178_993),
    province("30", "Murcia", 1_099_584, 1_061_943),
    province("31", "Navarra", 500_298, 489_978),
    province("32", "Ourense", 249_092, 252_827),
    province("34", "Palencia", 128_712, 130_129),
    province("35", "Las Palmas", 872_890, 843_241),
    province("36", "Pontevedra", 776_140, 772_695),
    province("26", "La Rioja", 238_458, 234_383),
    province("37", "Salamanca", 268_150, 268_868),
    province("38", "Santa Cruz de Tenerife", 801_955, 774_004),
    province("40", "Segovia", 117_269, 115_809),
    province("41", "Sevilla", 1_563_228, 1_525_548),
    province("42", "Soria", 68_010, 68_463),
    province("43", "Tarragona", 599_500, 575_113),
    province("44", "Teruel", 100_354, 101_249),
    province("45", "Toledo", 553_837, 529_311),
    province("46", "Valencia / València", 1_984_250, 1_928_949),
    province("47", "Valladolid", 422_416, 416_646),
    province("49", "Zamora", 138_140, 142_159),
    province("50", "Zaragoza", 729_968, 713_505),
    city("51", "Ceuta", 62_231, 60_335),
    city("52", "Melilla", 57_087, 55_150),
];

/// Las provincias por orden alfabético y después las ciudades autónomas (RN-03), con el censo de
/// `election`.
pub fn ordered(election: Election) -> Vec<District> {
    let mut districts: Vec<District> = DISTRICTS
        .iter()
        .map(|district| district.for_election(election))
        .collect();
    districts.sort_by_cached_key(|district| (district.territory, sort_key(district.name)));
    districts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_census_adds_up_to_the_ine_total() {
        assert_eq!(
            DISTRICTS
                .iter()
                .map(|district| district.census)
                .sum::<u64>(),
            TOTAL_CENSUS
        );
    }

    #[test]
    fn the_2023_census_adds_up_and_the_codes_are_unique() {
        let districts = ordered(Election::Generales2023);
        assert_eq!(
            districts.iter().map(|d| d.census).sum::<u64>(),
            TOTAL_CENSUS_2023
        );
        let codes: std::collections::BTreeSet<_> = districts.iter().map(|d| d.code).collect();
        assert_eq!(codes.len(), 52);
        let madrid = districts
            .iter()
            .find(|d| d.code == "28")
            .map(|d| (d.name, d.census));
        assert_eq!(madrid, Some(("Madrid", 4_848_057)));
        assert_eq!(
            (
                Election::Generales2026.year(),
                Election::Generales2023.year()
            ),
            (2026, 2023)
        );
    }

    #[test]
    fn the_2023_data_is_the_one_the_service_serves_in_the_same_order() {
        let ours: Vec<_> = ordered(Election::Generales2023)
            .iter()
            .map(|d| (d.code, d.name, d.census))
            .collect();
        let theirs: Vec<_> = escrutinio_contrato::districts::DISTRICTS
            .iter()
            .map(|d| (d.code, d.name, d.electorate))
            .collect();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn there_are_50_provinces_and_2_autonomous_cities() {
        let count = |territory| {
            DISTRICTS
                .iter()
                .filter(|district| district.territory == territory)
                .count()
        };
        assert_eq!(count(Territory::Province), 50);
        assert_eq!(count(Territory::AutonomousCity), 2);
    }

    #[test]
    fn orders_provinces_alphabetically_and_then_the_autonomous_cities() {
        let names: Vec<_> = ordered(Election::Generales2026)
            .iter()
            .map(|district| district.name)
            .collect();
        assert_eq!(
            names[..8],
            [
                "A Coruña",
                "Albacete",
                "Alicante / Alacant",
                "Almería",
                "Araba/Álava",
                "Asturias",
                "Ávila",
                "Badajoz",
            ]
        );
        assert_eq!(
            names[names.len() - 4..],
            ["Zamora", "Zaragoza", "Ceuta", "Melilla"]
        );
        let position = |name| names.iter().position(|&other| other == name);
        assert!(position("Illes Balears") > position("Huesca"));
        assert!(position("La Rioja") < position("Las Palmas"));
    }
}
