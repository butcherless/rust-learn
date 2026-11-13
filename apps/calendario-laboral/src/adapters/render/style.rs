//! Los colores y estilos de cada `DayKind` y de la rejilla (RN-20, RN-21).

use super::*;

/// El estilo de cada `DayKind`: su color y, en `Vacation`, negrita para separarla del rojo de
/// `Holiday`. En cada pareja laborable, la jornada reducida es más tenue que la completa: cian
/// frente a azul y, en `Summer`, un verde pálido de la paleta de 256 frente al verde.
const fn style(kind: DayKind) -> Style {
    let style = Style::new().fg_color(Some(color(kind)));
    match kind {
        DayKind::Vacation => style.bold(),
        DayKind::Working
        | DayKind::Reduced
        | DayKind::SummerWorking
        | DayKind::SummerReduced
        | DayKind::Weekend
        | DayKind::Holiday => style,
    }
}

pub(super) const fn color(kind: DayKind) -> Color {
    match kind {
        DayKind::Working => Color::Ansi(AnsiColor::Blue),
        DayKind::Reduced => Color::Ansi(AnsiColor::Cyan),
        DayKind::SummerWorking => Color::Ansi(AnsiColor::Green),
        DayKind::SummerReduced => Color::Ansi256(Ansi256Color(151)),
        DayKind::Weekend => Color::Ansi(AnsiColor::BrightBlack),
        DayKind::Holiday => Color::Ansi(AnsiColor::Red),
        DayKind::Vacation => Color::Ansi256(Ansi256Color(208)),
    }
}

pub(super) fn paint(kind: DayKind, text: &str) -> String {
    styled(style(kind), text)
}

pub(super) fn styled(style: Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

/// Las líneas de la rejilla de la vista de año, en gris para que no compitan con los días (RN-21).
pub(super) fn paint_grid(text: &str) -> String {
    styled(
        Style::new().fg_color(Some(AnsiColor::BrightBlack.into())),
        text,
    )
}
