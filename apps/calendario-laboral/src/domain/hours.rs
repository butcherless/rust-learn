//! Las horas de cada `Shift` de un `Year` (RF-09, RN-17, RN-25).

use crate::domain::{DomainError, Shift};

/// Las horas de los cuatro `Shift` laborables (RF-09).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hours {
    working: f64,
    reduced: f64,
    summer_working: f64,
    summer_reduced: f64,
}

impl Default for Hours {
    fn default() -> Self {
        Self {
            working: 8.5,
            reduced: 6.0,
            summer_working: 8.5,
            summer_reduced: 6.0,
        }
    }
}

impl Hours {
    /// Las horas de `shift`.
    pub const fn of(self, shift: Shift) -> f64 {
        match shift {
            Shift::Working => self.working,
            Shift::Reduced => self.reduced,
            Shift::SummerWorking => self.summer_working,
            Shift::SummerReduced => self.summer_reduced,
        }
    }

    /// Las mismas horas con `hours` para `shift`, que deben estar entre 0 y 24 (RN-17).
    pub fn with(self, shift: Shift, hours: f64) -> Result<Self, DomainError> {
        (0.0..=24.0)
            .contains(&hours)
            .then_some(match shift {
                Shift::Working => Self {
                    working: hours,
                    ..self
                },
                Shift::Reduced => Self {
                    reduced: hours,
                    ..self
                },
                Shift::SummerWorking => Self {
                    summer_working: hours,
                    ..self
                },
                Shift::SummerReduced => Self {
                    summer_reduced: hours,
                    ..self
                },
            })
            .ok_or(DomainError::InvalidHours(hours))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rn_17_hours_are_between_0_and_24() {
        [
            (0.0, true),
            (24.0, true),
            (-0.5, false),
            (24.5, false),
            (f64::NAN, false),
        ]
        .into_iter()
        .for_each(|(hours, valid)| {
            assert_eq!(
                Hours::default().with(Shift::Working, hours).is_ok(),
                valid,
                "{hours}"
            );
        });
    }
}
