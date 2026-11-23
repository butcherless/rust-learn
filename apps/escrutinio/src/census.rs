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
//! Las mesas de cada circunscripción, para saber cuándo está completa (RN-12), son las de las
//! listas de votación del INE en 2026 (`elecgral_2026_lv_tab1.xlsx`, 61.033) y las del fichero de
//! Interior en 2023 (60.314).
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
    /// Mesas electorales de la elección que se muestra.
    pub stations: u32,
    /// Electores CER y mesas de 2023.
    census_2023: u64,
    stations_2023: u32,
}

impl District {
    /// La misma circunscripción con el censo de `election`.
    pub fn for_election(self, election: Election) -> Self {
        match election {
            Election::Generales2026 => self,
            Election::Generales2023 => Self {
                census: self.census_2023,
                stations: self.stations_2023,
                ..self
            },
        }
    }
}

/// El total nacional del CER de 2026 que publica el INE, para comprobar los datos.
pub const TOTAL_CENSUS: u64 = 35_953_542;
/// El total del censo de escrutinio de las mesas de 2023, para comprobar los datos.
pub const TOTAL_CENSUS_2023: u64 = 35_141_198;
/// Las mesas de toda España: 2026 según el INE y 2023 según Interior.
pub const TOTAL_STATIONS: u32 = 61_033;
pub const TOTAL_STATIONS_2023: u32 = 60_314;

/// Una provincia con su (censo, mesas) de 2026 y de 2023.
const fn province(
    code: &'static str,
    name: &'static str,
    (census, stations): (u64, u32),
    (census_2023, stations_2023): (u64, u32),
) -> District {
    District {
        code,
        name,
        territory: Territory::Province,
        census,
        stations,
        census_2023,
        stations_2023,
    }
}

/// Una ciudad autónoma con su (censo, mesas) de 2026 y de 2023.
const fn city(
    code: &'static str,
    name: &'static str,
    (census, stations): (u64, u32),
    (census_2023, stations_2023): (u64, u32),
) -> District {
    District {
        code,
        name,
        territory: Territory::AutonomousCity,
        census,
        stations,
        census_2023,
        stations_2023,
    }
}

/// Las 50 provincias y las 2 ciudades autónomas, en el orden de la tabla del INE.
pub const DISTRICTS: [District; 52] = [
    province("02", "Albacete", (299_550, 527), (299_924, 521)),
    province(
        "03",
        "Alicante / Alacant",
        (1_303_575, 2_234),
        (1_261_010, 2_227),
    ),
    province("04", "Almería", (487_775, 850), (471_427, 837)),
    province("01", "Araba/Álava", (256_960, 422), (252_058, 427)),
    province("33", "Asturias", (835_351, 1_443), (835_593, 1_528)),
    province("05", "Ávila", (127_004, 354), (128_300, 358)),
    province("06", "Badajoz", (538_937, 904), (538_574, 902)),
    province("07", "Illes Balears", (828_523, 1_275), (795_915, 1_249)),
    province("08", "Barcelona", (4_132_494, 6_548), (4_025_818, 6_472)),
    province("48", "Bizkaia", (909_176, 1_402), (900_563, 1_410)),
    province("09", "Burgos", (278_214, 753), (277_537, 746)),
    province("10", "Cáceres", (321_039, 513), (322_765, 524)),
    province("11", "Cádiz", (1_007_030, 1_617), (986_857, 1_528)),
    province("39", "Cantabria", (473_141, 742), (466_589, 777)),
    province("12", "Castellón / Castelló", (435_013, 754), (419_903, 740)),
    province("13", "Ciudad Real", (385_578, 711), (384_479, 707)),
    province("14", "Córdoba", (627_827, 956), (626_123, 948)),
    province("15", "A Coruña", (928_060, 1_528), (922_306, 1_544)),
    province("16", "Cuenca", (147_724, 379), (149_438, 383)),
    province("20", "Gipuzkoa", (561_839, 867), (556_075, 858)),
    province("17", "Girona", (545_015, 894), (522_354, 863)),
    province("18", "Granada", (728_577, 1_143), (712_178, 1_121)),
    province("19", "Guadalajara", (202_513, 588), (191_796, 587)),
    province("21", "Huelva", (399_651, 656), (394_310, 651)),
    province("22", "Huesca", (166_250, 404), (165_249, 403)),
    province("23", "Jaén", (500_508, 870), (504_825, 873)),
    province("24", "León", (370_867, 676), (374_272, 722)),
    province("25", "Lleida", (304_354, 562), (298_948, 549)),
    province("27", "Lugo", (264_082, 517), (268_657, 533)),
    province("28", "Madrid", (5_036_558, 7_608), (4_848_057, 7_118)),
    province("29", "Málaga", (1_218_788, 1_778), (1_178_993, 1_734)),
    province("30", "Murcia", (1_099_584, 1_743), (1_061_943, 1_726)),
    province("31", "Navarra", (500_298, 956), (489_978, 997)),
    province("32", "Ourense", (249_092, 523), (252_827, 539)),
    province("34", "Palencia", (128_712, 342), (130_129, 335)),
    province("35", "Las Palmas", (872_890, 1_559), (843_241, 1_564)),
    province("36", "Pontevedra", (776_140, 1_344), (772_695, 1_344)),
    province("26", "La Rioja", (238_458, 518), (234_383, 516)),
    province("37", "Salamanca", (268_150, 633), (268_868, 648)),
    province(
        "38",
        "Santa Cruz de Tenerife",
        (801_955, 1_382),
        (774_004, 1_379),
    ),
    province("40", "Segovia", (117_269, 328), (115_809, 330)),
    province("41", "Sevilla", (1_563_228, 2_625), (1_525_548, 2_588)),
    province("42", "Soria", (68_010, 257), (68_463, 256)),
    province("43", "Tarragona", (599_500, 1_072), (575_113, 1_039)),
    province("44", "Teruel", (100_354, 328), (101_249, 328)),
    province("45", "Toledo", (553_837, 932), (529_311, 903)),
    province(
        "46",
        "Valencia / València",
        (1_984_250, 3_207),
        (1_928_949, 3_200),
    ),
    province("47", "Valladolid", (422_416, 774), (416_646, 763)),
    province("49", "Zamora", (138_140, 368), (142_159, 369)),
    province("50", "Zaragoza", (729_968, 1_484), (713_505, 1_467)),
    city("51", "Ceuta", (62_231, 96), (60_335, 96)),
    city("52", "Melilla", (57_087, 87), (55_150, 87)),
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
    fn the_2023_census_adds_up_to_the_stations_and_codes_are_unique() {
        let districts = ordered(Election::Generales2023);
        assert_eq!(
            districts.iter().map(|d| d.census).sum::<u64>(),
            TOTAL_CENSUS_2023
        );
        assert_eq!(
            districts.iter().map(|d| d.stations).sum::<u32>(),
            TOTAL_STATIONS_2023
        );
        let stations_2026 = ordered(Election::Generales2026)
            .iter()
            .map(|d| d.stations)
            .sum::<u32>();
        assert_eq!(stations_2026, TOTAL_STATIONS);
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
