//! Lo que la ventana pide a la red debajo de la próxima ausencia: los embalses (RN-31) y, después,
//! los mercados (RN-32). Todas las peticiones van en un hilo aparte, de una en una y en orden.

use std::iter;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::markets::{MarketTable, QuoteSource};
use super::reservoirs::{Fetched, HistorySource, ReservoirTable, fetched};
use crate::domain::{Market, Quote, Reservoir};

/// Una respuesta que llega del hilo de las peticiones.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    /// La de un embalse.
    Reservoir(Fetched),
    /// La de los mercados, o nada si ha fallado.
    Markets(Option<Vec<(Market, Quote)>>),
}

/// Las tablas de embalses y de mercados mientras llegan sus respuestas.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// Los embalses.
    pub reservoirs: ReservoirTable,
    /// Los mercados, que se piden cuando terminan los embalses.
    pub markets: MarketTable,
}

impl Panel {
    /// El panel al empezar: se pide el primer embalse, o los mercados si no hay embalses.
    pub fn new(reservoirs: &[Reservoir]) -> Self {
        Self {
            reservoirs: ReservoirTable::new(reservoirs),
            markets: MarketTable::default(),
        }
        .with_markets_started()
    }

    /// El panel con `update`.
    pub fn updated(self, update: Update) -> Self {
        match update {
            Update::Reservoir(fetched) => Self {
                reservoirs: self.reservoirs.fetched(fetched),
                ..self
            }
            .with_markets_started(),
            Update::Markets(quotes) => Self {
                markets: self.markets.fetched(quotes.as_deref()),
                ..self
            },
        }
    }

    /// Los mercados pasan a pedirse en cuanto ya no falta ningún embalse.
    fn with_markets_started(self) -> Self {
        if !self.reservoirs.is_fetching() && self.markets.is_waiting() {
            Self {
                markets: self.markets.started(),
                ..self
            }
        } else {
            self
        }
    }

    /// El panel en la otra mitad del parpadeo.
    pub fn ticked(self) -> Self {
        Self {
            reservoirs: self.reservoirs.ticked(),
            markets: self.markets.ticked(),
        }
    }

    /// Si aún falta alguna respuesta.
    pub fn is_fetching(&self) -> bool {
        self.reservoirs.is_fetching() || self.markets.is_fetching()
    }
}

/// Las respuestas en orden: primero los embalses, de uno en uno, y después los mercados. Cada
/// petición se hace al pedir el elemento siguiente.
pub fn updates<'a, R: HistorySource, Q: QuoteSource>(
    reservoir_source: &'a R,
    quote_source: &'a Q,
    reservoirs: &'a [Reservoir],
) -> impl Iterator<Item = Update> + 'a {
    fetched(reservoir_source, reservoirs)
        .map(Update::Reservoir)
        .chain(iter::once_with(|| {
            Update::Markets(quote_source.quotes().ok())
        }))
}

/// Manda cada respuesta de [`updates`] por `sender` en cuanto llega. Para en cuanto nadie la
/// escucha, sin hacer más peticiones.
pub fn send_updates(updates: impl Iterator<Item = Update>, sender: &Sender<Update>) {
    updates
        .map(|update| sender.send(update))
        .take_while(Result::is_ok)
        .for_each(drop);
}

/// [`updates`] en otro hilo. Las respuestas llegan por el `Receiver`.
pub fn spawn<R, Q>(
    reservoir_source: R,
    quote_source: Q,
    reservoirs: &'static [Reservoir],
) -> Receiver<Update>
where
    R: HistorySource + Send + 'static,
    Q: QuoteSource + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        send_updates(
            updates(&reservoir_source, &quote_source, reservoirs),
            &sender,
        );
    });
    receiver
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::adapters::feed::Reading;
    use crate::adapters::markets::MarketError;
    use crate::adapters::markets::fixtures::{brent, ibex};
    use crate::adapters::reservoirs::ReservoirError;
    use crate::adapters::reservoirs::fixtures::{RESERVOIRS, atazar};
    use crate::domain::{WeeklyLevel, reservoir_level};

    /// Cuenta las peticiones; el histórico solo tiene datos para `atazar-el`.
    #[derive(Default)]
    struct Counting {
        asked: Cell<usize>,
    }

    impl HistorySource for Counting {
        fn history(&self, slug: &str) -> Result<Vec<WeeklyLevel>, ReservoirError> {
            self.asked.set(self.asked.get() + 1);
            Ok(if slug == "atazar-el" {
                vec![atazar().latest]
            } else {
                Vec::new()
            })
        }
    }

    impl QuoteSource for Counting {
        fn quotes(&self) -> Result<Vec<(Market, Quote)>, MarketError> {
            self.asked.set(self.asked.get() + 1);
            Ok(vec![(Market::Ibex35, ibex()), (Market::Brent, brent())])
        }
    }

    fn reservoir(index: usize) -> Update {
        Update::Reservoir(Fetched {
            index,
            level: (index == 0)
                .then(|| reservoir_level(&[atazar().latest]))
                .flatten(),
        })
    }

    #[test]
    fn rn_32_the_markets_are_asked_for_once_after_the_reservoirs() {
        let (history, quotes) = (Counting::default(), Counting::default());
        assert_eq!(
            updates(&history, &quotes, &RESERVOIRS).collect::<Vec<_>>(),
            [
                reservoir(0),
                reservoir(1),
                reservoir(2),
                Update::Markets(Some(vec![
                    (Market::Ibex35, ibex()),
                    (Market::Brent, brent())
                ])),
            ]
        );
        assert_eq!((history.asked.get(), quotes.asked.get()), (3, 1));
    }

    #[test]
    fn rn_31_rn_32_nothing_more_is_asked_for_once_nobody_listens() {
        let (history, quotes) = (Counting::default(), Counting::default());
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        send_updates(updates(&history, &quotes, &RESERVOIRS), &sender);
        assert_eq!((history.asked.get(), quotes.asked.get()), (1, 0));
        // Y en otro hilo llegan todas.
        assert_eq!(
            spawn(Counting::default(), Counting::default(), &RESERVOIRS)
                .iter()
                .count(),
            4
        );
    }

    #[test]
    fn rn_32_the_markets_start_when_the_reservoirs_are_done() {
        let panel = Panel::new(&RESERVOIRS);
        assert!(panel.markets.is_waiting());
        let after_two = [reservoir(0), reservoir(1)]
            .into_iter()
            .fold(panel, Panel::updated);
        assert!(after_two.markets.is_waiting());
        let started = after_two.updated(reservoir(2));
        assert!(!started.markets.is_waiting() && started.is_fetching());
        let done = started.updated(Update::Markets(None));
        assert!(!done.is_fetching());
        assert!(
            done.markets
                .rows()
                .iter()
                .all(|(_, reading)| *reading == Reading::Failed)
        );
        assert!(!Panel::new(&[]).markets.is_waiting());
        let ticked = done.clone().ticked();
        assert_ne!(ticked.reservoirs.lit(), done.reservoirs.lit());
        assert_ne!(ticked.markets.lit(), done.markets.lit());
    }
}
