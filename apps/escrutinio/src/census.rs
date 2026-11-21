//! El censo de cada `District` (RN-01, RN-02).
//!
//! Son los electores españoles residentes en España (CER) de las listas de votación de las
//! Elecciones a Cortes Generales del 29 de noviembre de 2026, de la Oficina del Censo Electoral
//! del INE: tabla `elecgral_2026_lv_tab3_mod.xlsx`, ya modificada por los apéndices de
//! reclamaciones y los autos del Tribunal Supremo de septiembre y octubre de 2026. Los nombres son
//! los oficiales, con el artículo delante (`A Coruña`, no `Coruña, A`).
//!
//! `ordered` da el orden de la tabla (RN-03).

use crate::domain::sort_key;

/// El tipo de circunscripción. Las provincias van antes que las ciudades autónomas (RN-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Territory {
    Province,
    AutonomousCity,
}

/// Una provincia o ciudad autónoma, con su censo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct District {
    pub name: &'static str,
    pub territory: Territory,
    /// Electores CER.
    pub census: u64,
}

/// El total nacional del CER que publica el INE, para comprobar los datos.
pub const TOTAL_CENSUS: u64 = 35_953_542;

const fn province(name: &'static str, census: u64) -> District {
    District {
        name,
        territory: Territory::Province,
        census,
    }
}

const fn city(name: &'static str, census: u64) -> District {
    District {
        name,
        territory: Territory::AutonomousCity,
        census,
    }
}

/// Las 50 provincias y las 2 ciudades autónomas, en el orden de la tabla del INE.
pub const DISTRICTS: [District; 52] = [
    province("Albacete", 299_550),
    province("Alicante / Alacant", 1_303_575),
    province("Almería", 487_775),
    province("Araba/Álava", 256_960),
    province("Asturias", 835_351),
    province("Ávila", 127_004),
    province("Badajoz", 538_937),
    province("Illes Balears", 828_523),
    province("Barcelona", 4_132_494),
    province("Bizkaia", 909_176),
    province("Burgos", 278_214),
    province("Cáceres", 321_039),
    province("Cádiz", 1_007_030),
    province("Cantabria", 473_141),
    province("Castellón / Castelló", 435_013),
    province("Ciudad Real", 385_578),
    province("Córdoba", 627_827),
    province("A Coruña", 928_060),
    province("Cuenca", 147_724),
    province("Gipuzkoa", 561_839),
    province("Girona", 545_015),
    province("Granada", 728_577),
    province("Guadalajara", 202_513),
    province("Huelva", 399_651),
    province("Huesca", 166_250),
    province("Jaén", 500_508),
    province("León", 370_867),
    province("Lleida", 304_354),
    province("Lugo", 264_082),
    province("Madrid", 5_036_558),
    province("Málaga", 1_218_788),
    province("Murcia", 1_099_584),
    province("Navarra", 500_298),
    province("Ourense", 249_092),
    province("Palencia", 128_712),
    province("Las Palmas", 872_890),
    province("Pontevedra", 776_140),
    province("La Rioja", 238_458),
    province("Salamanca", 268_150),
    province("Santa Cruz de Tenerife", 801_955),
    province("Segovia", 117_269),
    province("Sevilla", 1_563_228),
    province("Soria", 68_010),
    province("Tarragona", 599_500),
    province("Teruel", 100_354),
    province("Toledo", 553_837),
    province("Valencia / València", 1_984_250),
    province("Valladolid", 422_416),
    province("Zamora", 138_140),
    province("Zaragoza", 729_968),
    city("Ceuta", 62_231),
    city("Melilla", 57_087),
];

/// Las provincias por orden alfabético y después las ciudades autónomas (RN-03).
pub fn ordered() -> Vec<District> {
    let mut districts = DISTRICTS.to_vec();
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
        let names: Vec<_> = ordered().iter().map(|district| district.name).collect();
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
