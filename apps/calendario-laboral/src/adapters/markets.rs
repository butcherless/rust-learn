//! Adaptador de los mercados de RN-32: el cliente del JSON de cotizaciones de Expansión y el
//! estado de la tabla que dibuja `render`.

use chrono::{NaiveDate, NaiveTime};
use serde::Deserialize;

use super::feed::{Reading, http_client};
use crate::domain::{Market, Quote};

/// La raíz de la web de Expansión.
pub const MARKETS_URL: &str = "https://www.expansion.com";

/// Un error al pedir las cotizaciones.
#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    /// La petición no ha llegado, la respuesta no es 2xx o su JSON no es el esperado.
    #[error("no se han podido consultar las cotizaciones de Expansión")]
    Request(#[from] reqwest::Error),
}

/// De dónde salen las cotizaciones.
pub trait QuoteSource {
    /// La última cotización de cada [`Market`] que traiga la respuesta.
    fn quotes(&self) -> Result<Vec<(Market, Quote)>, MarketError>;
}

/// El cliente bloqueante de Expansión. No debe crearse ni usarse dentro de un runtime async.
#[derive(Debug, Clone)]
pub struct ExpansionClient {
    client: reqwest::blocking::Client,
    base_url: String,
}

impl ExpansionClient {
    /// Un cliente para la web de `base_url`, sin barra al final.
    pub fn new(base_url: &str) -> Result<Self, MarketError> {
        Ok(Self {
            client: http_client()?,
            base_url: base_url.to_owned(),
        })
    }
}

impl QuoteSource for ExpansionClient {
    fn quotes(&self) -> Result<Vec<(Market, Quote)>, MarketError> {
        let codes: Vec<&str> = Market::ALL.into_iter().map(request_code).collect();
        let url = format!(
            "{}/datos-bolsa/lista_valores.html?cod_valor={}",
            self.base_url,
            codes.join(",")
        );
        let values: Vec<ValueDto> = self.client.get(url).send()?.error_for_status()?.json()?;
        Ok(values.iter().filter_map(ValueDto::quote).collect())
    }
}

/// El código con que se pide cada valor.
const fn request_code(market: Market) -> &'static str {
    match market {
        Market::Ibex35 => "I.IB",
        Market::Brent => "MAT.LCO",
    }
}

/// El código con que la respuesta identifica cada valor: el de las materias primas no lleva
/// `MAT.`.
const fn response_code(market: Market) -> &'static str {
    match market {
        Market::Ibex35 => "I.IB",
        Market::Brent => "LCO",
    }
}

/// Un valor de la respuesta. Los que no son de [`Market`] o no tienen precio o fecha válidos se
/// descartan.
#[derive(Deserialize)]
struct ValueDto {
    codigo: String,
    #[serde(rename = "ultimaCotizacion")]
    last: Option<LastDto>,
}

#[derive(Deserialize)]
struct LastDto {
    precio: Option<f64>,
    #[serde(rename = "precioAnterior")]
    previous: Option<f64>,
    fecha: DateDto,
}

/// La fecha de la cotización. Si no se conoce la hora, `time` es `false` y la hora y el minuto
/// valen -1.
#[derive(Deserialize)]
struct DateDto {
    dia: u32,
    mes: u32,
    anyo: i32,
    hora: i64,
    minuto: i64,
    time: bool,
}

impl ValueDto {
    fn quote(&self) -> Option<(Market, Quote)> {
        let market = Market::ALL
            .into_iter()
            .find(|&market| response_code(market) == self.codigo)?;
        let last = self.last.as_ref()?;
        let date = &last.fecha;
        let time = date
            .time
            .then(|| {
                let hour = u32::try_from(date.hora).ok()?;
                let minute = u32::try_from(date.minuto).ok()?;
                NaiveTime::from_hms_opt(hour, minute, 0)
            })
            .flatten();
        let price = last.precio?;
        Some((
            market,
            Quote {
                price,
                previous: last.previous.unwrap_or(price),
                date: NaiveDate::from_ymd_opt(date.anyo, date.mes, date.dia)?,
                time,
            },
        ))
    }
}

/// La tabla de mercados mientras llega la respuesta: cada valor con su [`Reading`], y si la luz
/// está encendida en el parpadeo. Los dos valores se piden juntos.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketTable {
    rows: Vec<(Market, Reading<Quote>)>,
    lit: bool,
}

impl Default for MarketTable {
    /// La tabla al empezar: todos los valores esperan.
    fn default() -> Self {
        Self {
            rows: Market::ALL
                .into_iter()
                .map(|market| (market, Reading::Waiting))
                .collect(),
            lit: true,
        }
    }
}

impl MarketTable {
    /// La tabla mientras se piden los valores.
    pub fn started(self) -> Self {
        self.with_readings(|_| Reading::Fetching)
    }

    /// La tabla con la respuesta: cada valor que trae queda cargado, y los demás, fallidos. Sin
    /// respuesta, fallan todos.
    pub fn fetched(self, quotes: Option<&[(Market, Quote)]>) -> Self {
        self.with_readings(|market| {
            quotes
                .into_iter()
                .flatten()
                .find(|(other, _)| *other == market)
                .map_or(Reading::Failed, |&(_, quote)| Reading::Loaded(quote))
        })
    }

    fn with_readings(self, reading: impl Fn(Market) -> Reading<Quote>) -> Self {
        Self {
            rows: self
                .rows
                .into_iter()
                .map(|(market, _)| (market, reading(market)))
                .collect(),
            ..self
        }
    }

    /// La tabla en la otra mitad del parpadeo.
    pub fn ticked(self) -> Self {
        Self {
            lit: !self.lit,
            ..self
        }
    }

    /// Si los valores aún no se han pedido.
    pub fn is_waiting(&self) -> bool {
        self.rows
            .iter()
            .all(|(_, reading)| *reading == Reading::Waiting)
    }

    /// Si aún falta la respuesta.
    pub fn is_fetching(&self) -> bool {
        self.rows.iter().any(|(_, reading)| reading.is_pending())
    }

    /// Las filas, en orden.
    pub fn rows(&self) -> &[(Market, Reading<Quote>)] {
        &self.rows
    }

    /// Si la luz está encendida.
    pub const fn lit(&self) -> bool {
        self.lit
    }

    /// Si ha llegado algún valor.
    pub fn has_quotes(&self) -> bool {
        self.rows
            .iter()
            .any(|(_, reading)| matches!(reading, Reading::Loaded(_)))
    }
}

/// Constructores compartidos por los tests de los mercados.
#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::domain::fixtures::date;

    /// El IBEX 35 al cierre del 2026-11-13, a las 17:35.
    pub fn ibex() -> Quote {
        Quote {
            price: 20020.1,
            previous: 19965.5,
            date: date(2026, 11, 13),
            time: NaiveTime::from_hms_opt(17, 35, 0),
        }
    }

    /// El Brent del 2026-11-13, sin hora.
    pub fn brent() -> Quote {
        Quote {
            price: 95.9,
            previous: 96.04,
            date: date(2026, 11, 13),
            time: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{brent, ibex};
    use super::*;
    use crate::adapters::feed::USER_AGENT;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// `ExpansionClient::quotes` en un hilo bloqueante, como pide reqwest dentro de un runtime
    /// async.
    async fn quotes_from(server: &MockServer) -> Result<Vec<(Market, Quote)>, MarketError> {
        let base_url = server.uri();
        tokio::task::spawn_blocking(move || ExpansionClient::new(&base_url)?.quotes())
            .await
            .expect("blocking task")
    }

    #[tokio::test]
    async fn rn_32_the_client_asks_for_both_values_and_reads_their_last_quote() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/datos-bolsa/lista_valores.html"))
            .and(query_param("cod_valor", "I.IB,MAT.LCO"))
            .and(header("user-agent", USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"codigo": "I.IB", "nombre": "IBEX 35", "ultimaCotizacion": {
                    "fecha": {"dia": 13, "mes": 11, "anyo": 2026, "hora": 17, "minuto": 35,
                              "segundo": 30, "time": true, "fechaTexto": "13/11/2026"},
                    "precio": 20020.1, "precioAnterior": 19965.5, "maximo": 20053.9}},
                {"codigo": "LCO", "nombre": "Brent", "ultimaCotizacion": {
                    "fecha": {"dia": 13, "mes": 11, "anyo": 2026, "hora": -1, "minuto": -1,
                              "segundo": -1, "time": false},
                    "precio": 95.9, "precioAnterior": 96.04}},
                {"codigo": "I.MA", "nombre": "General Madrid", "ultimaCotizacion": {
                    "fecha": {"dia": 13, "mes": 11, "anyo": 2026, "hora": 17, "minuto": 35,
                              "time": true},
                    "precio": 2000.0, "precioAnterior": 1990.0}},
                {"codigo": "I.IB", "nombre": "sin cotización"}
            ])))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            quotes_from(&server).await.expect("quotes"),
            [(Market::Ibex35, ibex()), (Market::Brent, brent())]
        );
    }

    #[tokio::test]
    async fn rn_32_an_error_status_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let error = quotes_from(&server).await.expect_err("500");
        assert_eq!(
            error.to_string(),
            "no se han podido consultar las cotizaciones de Expansión"
        );
    }

    #[test]
    fn rn_32_the_table_waits_fetches_and_fills_each_value_or_fails_it() {
        let readings = |table: &MarketTable| -> Vec<Reading<Quote>> {
            table.rows().iter().map(|&(_, reading)| reading).collect()
        };
        let waiting = MarketTable::default();
        assert!(waiting.is_waiting() && waiting.is_fetching() && !waiting.has_quotes());
        let started = waiting.started();
        assert_eq!(readings(&started), [Reading::Fetching, Reading::Fetching]);
        assert!(!started.is_waiting());
        [
            (
                "both",
                Some(vec![(Market::Ibex35, ibex()), (Market::Brent, brent())]),
                vec![Reading::Loaded(ibex()), Reading::Loaded(brent())],
            ),
            (
                "only the brent",
                Some(vec![(Market::Brent, brent())]),
                vec![Reading::Failed, Reading::Loaded(brent())],
            ),
            ("no answer", None, vec![Reading::Failed, Reading::Failed]),
        ]
        .into_iter()
        .for_each(|(case, quotes, expected)| {
            let table = started.clone().fetched(quotes.as_deref());
            assert_eq!(readings(&table), expected, "{case}");
            assert!(!table.is_fetching(), "{case}");
        });
        assert!(started.clone().ticked().lit() != started.lit());
    }
}
