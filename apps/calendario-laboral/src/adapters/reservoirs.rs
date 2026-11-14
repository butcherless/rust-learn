//! Adaptador de los embalses de RN-31: el cliente de la API de MapasDeEmbalses.es, las peticiones
//! de una en una en otro hilo y el estado de la tabla que dibuja `render`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::domain::{Reservoir, ReservoirLevel, WeeklyLevel, reservoir_level};

/// La raíz de la API pública.
pub const API_URL: &str = "https://mapasdeembalses.es/api/v1";
/// Las semanas que se piden: todo el histórico, que empieza en 1988.
const HISTORY_WEEKS: u32 = 2000;
const TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = concat!("calendario-laboral/", env!("CARGO_PKG_VERSION"));

/// Un error al pedir el histórico de un embalse.
#[derive(Debug, thiserror::Error)]
pub enum ReservoirError {
    /// La petición no ha llegado, la respuesta no es 2xx o su JSON no es el esperado.
    #[error("no se ha podido consultar la API de embalses")]
    Request(#[from] reqwest::Error),
}

/// De dónde sale el histórico semanal de un embalse.
pub trait HistorySource {
    /// El histórico del embalse `slug`, en cualquier orden.
    fn history(&self, slug: &str) -> Result<Vec<WeeklyLevel>, ReservoirError>;
}

/// El cliente bloqueante de la API. No debe crearse ni usarse dentro de un runtime async.
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: reqwest::blocking::Client,
    base_url: String,
}

impl ApiClient {
    /// Un cliente para la API de `base_url`, sin barra al final.
    pub fn new(base_url: &str) -> Result<Self, ReservoirError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.to_owned(),
        })
    }
}

impl HistorySource for ApiClient {
    fn history(&self, slug: &str) -> Result<Vec<WeeklyLevel>, ReservoirError> {
        let url = format!(
            "{}/embalses/{slug}/historico?limit={HISTORY_WEEKS}",
            self.base_url
        );
        let body: HistoryDto = self.client.get(url).send()?.error_for_status()?.json()?;
        Ok(body.data.iter().filter_map(WeekDto::level).collect())
    }
}

#[derive(Deserialize)]
struct HistoryDto {
    data: Vec<WeekDto>,
}

/// Una semana del histórico. Las que no tienen fecha válida o algún dato se descartan.
#[derive(Deserialize)]
struct WeekDto {
    fecha: String,
    capacidad_hm3: Option<f64>,
    volumen_hm3: Option<f64>,
    fill_percentage: Option<f64>,
}

impl WeekDto {
    fn level(&self) -> Option<WeeklyLevel> {
        Some(WeeklyLevel {
            date: NaiveDate::parse_from_str(&self.fecha, "%Y-%m-%d").ok()?,
            capacity_hm3: self.capacidad_hm3?,
            volume_hm3: self.volumen_hm3?,
            percent: self.fill_percentage?,
        })
    }
}

/// La respuesta de la fila `index`: su nivel, o nada si la petición ha fallado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fetched {
    /// La posición del embalse en la tabla.
    pub index: usize,
    /// Su nivel, o nada si ha fallado.
    pub level: Option<ReservoirLevel>,
}

/// Pide los embalses de uno en uno y en orden, y manda cada respuesta por `sender` en cuanto
/// llega. Para en cuanto nadie la escucha.
pub fn fetch_in_order<S: HistorySource>(
    source: &S,
    reservoirs: &[Reservoir],
    sender: &Sender<Fetched>,
) {
    reservoirs
        .iter()
        .enumerate()
        .map(|(index, reservoir)| Fetched {
            index,
            level: source
                .history(reservoir.slug)
                .ok()
                .and_then(|history| reservoir_level(&history)),
        })
        .try_for_each(|fetched| sender.send(fetched))
        .unwrap_or(());
}

/// [`fetch_in_order`] en otro hilo. Las respuestas llegan por el `Receiver`.
pub fn spawn<S: HistorySource + Send + 'static>(
    source: S,
    reservoirs: &'static [Reservoir],
) -> Receiver<Fetched> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || fetch_in_order(&source, reservoirs, &sender));
    receiver
}

/// El estado de una fila.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading {
    /// Aún no se ha pedido.
    Waiting,
    /// Se está pidiendo.
    Fetching,
    /// Ha llegado.
    Loaded(ReservoirLevel),
    /// Ha fallado.
    Failed,
}

/// La tabla de embalses mientras llegan las respuestas: cada embalse con su [`Reading`], y si la
/// luz de la fila que se consulta está encendida en el parpadeo.
#[derive(Debug, Clone, PartialEq)]
pub struct ReservoirTable {
    rows: Vec<(Reservoir, Reading)>,
    lit: bool,
}

impl ReservoirTable {
    /// La tabla al empezar: el primer embalse se está pidiendo y los demás esperan.
    pub fn new(reservoirs: &[Reservoir]) -> Self {
        Self {
            rows: reservoirs
                .iter()
                .enumerate()
                .map(|(index, &reservoir)| {
                    let reading = if index == 0 {
                        Reading::Fetching
                    } else {
                        Reading::Waiting
                    };
                    (reservoir, reading)
                })
                .collect(),
            lit: true,
        }
    }

    /// La tabla con la respuesta de `fetched`: su fila queda cargada o fallida, y la siguiente
    /// pasa a pedirse.
    pub fn fetched(self, fetched: Fetched) -> Self {
        let rows = self
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, (reservoir, reading))| {
                let reading = if index == fetched.index {
                    fetched.level.map_or(Reading::Failed, Reading::Loaded)
                } else if index == fetched.index + 1 && reading == Reading::Waiting {
                    Reading::Fetching
                } else {
                    reading
                };
                (reservoir, reading)
            })
            .collect();
        Self { rows, ..self }
    }

    /// La tabla en la otra mitad del parpadeo.
    pub fn ticked(self) -> Self {
        Self {
            lit: !self.lit,
            ..self
        }
    }

    /// Si aún falta alguna respuesta.
    pub fn is_fetching(&self) -> bool {
        self.rows
            .iter()
            .any(|(_, reading)| matches!(reading, Reading::Waiting | Reading::Fetching))
    }

    /// Las filas, en orden.
    pub fn rows(&self) -> &[(Reservoir, Reading)] {
        &self.rows
    }

    /// Si la luz de la fila que se consulta está encendida.
    pub const fn lit(&self) -> bool {
        self.lit
    }

    /// La fecha del dato más reciente que ha llegado.
    pub fn latest_date(&self) -> Option<NaiveDate> {
        self.rows
            .iter()
            .filter_map(|(_, reading)| match reading {
                Reading::Loaded(level) => Some(level.latest.date),
                Reading::Waiting | Reading::Fetching | Reading::Failed => None,
            })
            .max()
    }
}

/// Constructores compartidos por los tests de los embalses.
#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::domain::fixtures::date;

    /// Tres embalses de prueba.
    pub const RESERVOIRS: [Reservoir; 3] = [
        Reservoir {
            slug: "atazar-el",
            name: "El Atazar",
        },
        Reservoir {
            slug: "pardo-el",
            name: "El Pardo",
        },
        Reservoir {
            slug: "jarosa-la",
            name: "La Jarosa",
        },
    ];

    /// El nivel de El Atazar el 2026-11-10, con su media de 10 años.
    pub fn atazar() -> ReservoirLevel {
        ReservoirLevel {
            latest: WeeklyLevel {
                date: date(2026, 11, 10),
                capacity_hm3: 426.0,
                volume_hm3: 271.0,
                percent: 63.62,
            },
            ten_year_percent: Some(66.2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{RESERVOIRS, atazar};
    use super::*;
    use crate::domain::fixtures::date;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Un histórico en memoria: `atazar-el` tiene datos y los demás vienen vacíos, que deja la
    /// fila como fallida igual que un error.
    struct FakeSource;

    impl HistorySource for FakeSource {
        fn history(&self, slug: &str) -> Result<Vec<WeeklyLevel>, ReservoirError> {
            Ok(if slug == "atazar-el" {
                vec![atazar().latest]
            } else {
                Vec::new()
            })
        }
    }

    /// `ApiClient::history` en un hilo bloqueante, como pide reqwest dentro de un runtime async.
    async fn history_from(server: &MockServer) -> Result<Vec<WeeklyLevel>, ReservoirError> {
        let base_url = server.uri();
        tokio::task::spawn_blocking(move || ApiClient::new(&base_url)?.history("atazar-el"))
            .await
            .expect("blocking task")
    }

    #[tokio::test]
    async fn rn_31_the_client_asks_for_the_whole_history_and_keeps_the_complete_weeks() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/embalses/atazar-el/historico"))
            .and(query_param("limit", "2000"))
            .and(header("user-agent", USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"fecha": "2026-11-10", "volumen_hm3": 271, "capacidad_hm3": 426,
                     "fill_percentage": 63.62, "variacion_semanal_hm3": -3},
                    {"fecha": "2026-11-03", "volumen_hm3": null, "capacidad_hm3": 426,
                     "fill_percentage": null},
                    {"fecha": "no es una fecha", "volumen_hm3": 1, "capacidad_hm3": 2,
                     "fill_percentage": 50}
                ],
                "meta": {"embalse": "Atazar, El"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            history_from(&server).await.expect("history"),
            [WeeklyLevel {
                date: date(2026, 11, 10),
                capacity_hm3: 426.0,
                volume_hm3: 271.0,
                percent: 63.62,
            }]
        );
    }

    #[tokio::test]
    async fn rn_31_an_error_status_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let error = history_from(&server).await.expect_err("404");
        assert_eq!(
            error.to_string(),
            "no se ha podido consultar la API de embalses"
        );
    }

    #[test]
    fn rn_31_requests_go_one_by_one_in_order_and_stop_when_nobody_listens() {
        let receiver = spawn(FakeSource, &RESERVOIRS);
        let level = reservoir_level(&[atazar().latest]);
        assert_eq!(
            receiver.iter().collect::<Vec<_>>(),
            [
                Fetched { index: 0, level },
                Fetched {
                    index: 1,
                    level: None
                },
                Fetched {
                    index: 2,
                    level: None
                },
            ]
        );
        // Sin nadie escuchando, la primera respuesta no se puede mandar y no se pide nada más.
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        fetch_in_order(&PanicsAfterFirst::default(), &RESERVOIRS, &sender);
    }

    /// Falla el test si se le pide más de un embalse.
    #[derive(Default)]
    struct PanicsAfterFirst {
        asked: std::cell::Cell<usize>,
    }

    impl HistorySource for PanicsAfterFirst {
        fn history(&self, slug: &str) -> Result<Vec<WeeklyLevel>, ReservoirError> {
            self.asked.set(self.asked.get() + 1);
            assert_eq!(self.asked.get(), 1, "asked for {slug}");
            Ok(Vec::new())
        }
    }

    #[test]
    fn rn_31_each_answer_fills_its_row_and_starts_the_next_one() {
        let rows = |table: &ReservoirTable| -> Vec<Reading> {
            table.rows().iter().map(|&(_, reading)| reading).collect()
        };
        let start = ReservoirTable::new(&RESERVOIRS);
        assert_eq!(
            rows(&start),
            [Reading::Fetching, Reading::Waiting, Reading::Waiting]
        );
        assert!(start.is_fetching());
        let first = start.fetched(Fetched {
            index: 0,
            level: Some(atazar()),
        });
        assert_eq!(
            rows(&first),
            [
                Reading::Loaded(atazar()),
                Reading::Fetching,
                Reading::Waiting
            ]
        );
        let done = [1, 2].into_iter().fold(first, |table, index| {
            table.fetched(Fetched { index, level: None })
        });
        assert_eq!(
            rows(&done),
            [Reading::Loaded(atazar()), Reading::Failed, Reading::Failed]
        );
        assert!(!done.is_fetching());
        assert_eq!(done.latest_date(), Some(date(2026, 11, 10)));
        assert_eq!(ReservoirTable::new(&RESERVOIRS).latest_date(), None);
    }

    #[test]
    fn rn_31_the_light_blinks_on_each_tick() {
        let table = ReservoirTable::new(&RESERVOIRS);
        assert!(table.lit());
        assert!(!table.clone().ticked().lit());
        assert!(table.ticked().ticked().lit());
    }
}
