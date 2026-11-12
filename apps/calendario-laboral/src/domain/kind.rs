//! Los conjuntos cerrados del dominio: `MarkKind`, `DayKind`, `Category` y `Shift` (§2.1).

use std::fmt;

/// Lo que fija un `Mark` en los días que cubre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// La jornada completa en un día que sería reducido (RN-26).
    Working,
    /// Una jornada más corta.
    Reduced,
    /// Un festivo nacional, autonómico o local.
    Holiday,
    /// Un día de vacaciones, que consume cupo.
    Vacation,
}

impl MarkKind {
    /// Todos los tipos, en el orden en que se muestran al usuario.
    pub const ALL: [Self; 4] = [Self::Working, Self::Reduced, Self::Holiday, Self::Vacation];

    /// El nombre que se usa en la línea de comandos y en el fichero.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Working => "laborable",
            Self::Reduced => "reducido",
            Self::Holiday => "festivo",
            Self::Vacation => "vacaciones",
        }
    }
}

impl fmt::Display for MarkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// La clase general a la que pertenece un `DayKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Un día con horas de trabajo.
    Working,
    /// Un día que nunca se trabaja, como el fin de semana.
    NonWorking,
    /// Un día laborable que no se trabaja: festivo o vacaciones.
    Absence,
}

/// Una de las cuatro jornadas laborables cuyas horas se configuran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    /// Jornada normal fuera del verano.
    Working,
    /// Jornada reducida fuera del verano.
    Reduced,
    /// Jornada normal en verano.
    SummerWorking,
    /// Jornada reducida en verano.
    SummerReduced,
}

impl Shift {
    /// Todas las jornadas.
    pub const ALL: [Self; 4] = [
        Self::Working,
        Self::Reduced,
        Self::SummerWorking,
        Self::SummerReduced,
    ];

    /// El nombre que se usa en la línea de comandos.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Working => "laborable",
            Self::Reduced => "reducido",
            Self::SummerWorking => "laborable-verano",
            Self::SummerReduced => "reducido-verano",
        }
    }
}

impl fmt::Display for Shift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// El tipo efectivo de un día, tras resolver marcas y reglas (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayKind {
    /// Día laborable normal fuera del verano.
    Working,
    /// Día de jornada reducida fuera del verano.
    Reduced,
    /// Día laborable normal en verano.
    SummerWorking,
    /// Día de jornada reducida en verano.
    SummerReduced,
    /// Un día de fin de semana.
    Weekend,
    /// Un festivo.
    Holiday,
    /// Un día de vacaciones.
    Vacation,
}

impl DayKind {
    /// Todos los tipos, en el orden de la leyenda y del resumen.
    pub const ALL: [Self; 7] = [
        Self::Working,
        Self::Reduced,
        Self::SummerWorking,
        Self::SummerReduced,
        Self::Weekend,
        Self::Holiday,
        Self::Vacation,
    ];

    /// La letra que identifica el tipo sin color.
    pub const fn letter(self) -> char {
        match self {
            Self::Working => 'L',
            Self::Reduced => 'R',
            Self::SummerWorking => 'l',
            Self::SummerReduced => 'r',
            Self::Weekend => 'S',
            Self::Holiday => 'F',
            Self::Vacation => 'V',
        }
    }

    /// El nombre que ve el usuario.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Working => "Laborable",
            Self::Reduced => "Reducido",
            Self::SummerWorking => "Laborable verano",
            Self::SummerReduced => "Reducido verano",
            Self::Weekend => "Fin de semana",
            Self::Holiday => "Festivo",
            Self::Vacation => "Vacaciones",
        }
    }

    /// La categoría del tipo.
    pub const fn category(self) -> Category {
        match self {
            Self::Working | Self::Reduced | Self::SummerWorking | Self::SummerReduced => {
                Category::Working
            }
            Self::Weekend => Category::NonWorking,
            Self::Holiday | Self::Vacation => Category::Absence,
        }
    }

    /// La jornada cuyas horas se aplican, en los tipos laborables.
    pub const fn shift(self) -> Option<Shift> {
        match self {
            Self::Working => Some(Shift::Working),
            Self::Reduced => Some(Shift::Reduced),
            Self::SummerWorking => Some(Shift::SummerWorking),
            Self::SummerReduced => Some(Shift::SummerReduced),
            Self::Weekend | Self::Holiday | Self::Vacation => None,
        }
    }
}

impl fmt::Display for DayKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_kinds_have_the_letter_category_and_shift_of_the_table() {
        [
            (
                DayKind::Working,
                'L',
                Category::Working,
                Some(Shift::Working),
            ),
            (
                DayKind::Reduced,
                'R',
                Category::Working,
                Some(Shift::Reduced),
            ),
            (
                DayKind::SummerWorking,
                'l',
                Category::Working,
                Some(Shift::SummerWorking),
            ),
            (
                DayKind::SummerReduced,
                'r',
                Category::Working,
                Some(Shift::SummerReduced),
            ),
            (DayKind::Weekend, 'S', Category::NonWorking, None),
            (DayKind::Holiday, 'F', Category::Absence, None),
            (DayKind::Vacation, 'V', Category::Absence, None),
        ]
        .into_iter()
        .for_each(|(kind, letter, category, shift)| {
            assert_eq!(
                (kind.letter(), kind.category(), kind.shift()),
                (letter, category, shift),
                "{kind}"
            );
        });
    }

    #[test]
    fn names_are_the_ones_of_the_command_line() {
        let marks: Vec<String> = MarkKind::ALL.iter().map(ToString::to_string).collect();
        let shifts: Vec<String> = Shift::ALL.iter().map(ToString::to_string).collect();
        assert_eq!(marks, ["laborable", "reducido", "festivo", "vacaciones"]);
        assert_eq!(
            shifts,
            [
                "laborable",
                "reducido",
                "laborable-verano",
                "reducido-verano"
            ]
        );
        assert_eq!(DayKind::SummerReduced.to_string(), "Reducido verano");
    }
}
