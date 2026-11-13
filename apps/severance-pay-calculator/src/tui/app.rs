//! The terminal UI's state, following The Elm Architecture: [`message`] turns a key into a
//! [`Message`], and [`App::update`] turns the current state and a message into the next state and
//! an [`Effect`]. Drawing lives in [`super::view`]. Nothing here does I/O.

use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use thiserror::Error;

use crate::domain::{CalcError, Dismissal, Input, Report, calculate, forecast};
use crate::report::FORECAST_YEARS;
use crate::sequence::Cycle;

const DATE_FORMAT: &str = "%Y-%m-%d";
const MAX_PAY_LEN: usize = 12;
const MAX_DATE_LEN: usize = 10;

/// A form field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Field {
    #[default]
    AnnualPay,
    Dismissal,
    InitialDate,
    CurrentDate,
}

/// What a text field accepts.
#[derive(Debug, Clone, Copy)]
struct TextRules {
    max_len: usize,
    accepts: fn(char) -> bool,
}

impl Field {
    /// The field's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::AnnualPay => "Annual pay",
            Self::Dismissal => "Type of dismissal",
            Self::InitialDate => "Initial date",
            Self::CurrentDate => "Current date",
        }
    }

    /// The rules of a text field; `None` for the dismissal selector, which takes no typing.
    fn text_rules(self) -> Option<TextRules> {
        match self {
            Self::AnnualPay => Some(TextRules {
                max_len: MAX_PAY_LEN,
                accepts: |c| c.is_ascii_digit() || c == '.',
            }),
            Self::InitialDate | Self::CurrentDate => Some(TextRules {
                max_len: MAX_DATE_LEN,
                accepts: |c| c.is_ascii_digit() || c == '-',
            }),
            Self::Dismissal => None,
        }
    }
}

impl Cycle for Field {
    const ALL: &'static [Self] = &[
        Self::AnnualPay,
        Self::Dismissal,
        Self::InitialDate,
        Self::CurrentDate,
    ];
}

/// A field's current content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldValue<'a> {
    /// What was typed into a text field.
    Text(&'a str),
    /// The selected preset.
    Choice(Dismissal),
}

/// Something the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Focus the next field.
    NextField,
    /// Focus the previous field.
    PreviousField,
    /// Change the dismissal type to the next preset.
    NextOption,
    /// Change the dismissal type to the previous preset.
    PreviousOption,
    /// Type a character into the focused text field.
    Type(char),
    /// Delete the last character of the focused text field.
    Backspace,
    /// Empty the focused text field.
    Clear,
    /// Leave the UI.
    Quit,
}

/// Work the event loop must do after an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Draw and wait for the next key.
    None,
    /// Restore the terminal and leave.
    Quit,
}

/// Why the form cannot be calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FormError {
    #[error("annual pay is not a number")]
    InvalidPay,
    #[error("{} is not a valid YYYY-MM-DD date", .0.label())]
    InvalidDate(Field),
    #[error(transparent)]
    Calc(#[from] CalcError),
}

/// The form's state. Each update consumes it and returns the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    focus: Field,
    annual_pay: String,
    dismissal: Dismissal,
    initial_date: String,
    current_date: String,
}

/// Maps a key press to a message. Unbound keys give `None`.
pub fn message(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Esc => Some(Message::Quit),
        KeyCode::Tab | KeyCode::Down | KeyCode::Enter => Some(Message::NextField),
        KeyCode::BackTab | KeyCode::Up => Some(Message::PreviousField),
        KeyCode::Right | KeyCode::Char(' ') => Some(Message::NextOption),
        KeyCode::Left => Some(Message::PreviousOption),
        KeyCode::Backspace => Some(Message::Backspace),
        KeyCode::Delete => Some(Message::Clear),
        KeyCode::Char(c) => Some(Message::Type(c)),
        _ => None,
    }
}

/// A form filled with `input`'s values, focused on the first field.
impl From<Input> for App {
    fn from(input: Input) -> Self {
        Self {
            focus: Field::default(),
            annual_pay: input.annual_pay.to_string(),
            dismissal: input.dismissal,
            initial_date: input.initial_date.format(DATE_FORMAT).to_string(),
            current_date: input.current_date.format(DATE_FORMAT).to_string(),
        }
    }
}

impl App {
    /// The focused field.
    pub fn focus(&self) -> Field {
        self.focus
    }

    /// The content of `field`.
    pub fn value(&self, field: Field) -> FieldValue<'_> {
        match field {
            Field::AnnualPay => FieldValue::Text(&self.annual_pay),
            Field::Dismissal => FieldValue::Choice(self.dismissal),
            Field::InitialDate => FieldValue::Text(&self.initial_date),
            Field::CurrentDate => FieldValue::Text(&self.current_date),
        }
    }

    /// Computes the next state for `message`, and the effect the event loop must perform.
    pub fn update(self, message: Message) -> (Self, Effect) {
        let next = match message {
            Message::Quit => return (self, Effect::Quit),
            Message::NextField => self.with_focus(Field::next),
            Message::PreviousField => self.with_focus(Field::previous),
            Message::NextOption => self.with_dismissal(Dismissal::next),
            Message::PreviousOption => self.with_dismissal(Dismissal::previous),
            Message::Type(c) => self.typed(c),
            Message::Backspace => self.edited(without_last_char),
            Message::Clear => self.edited(|_| String::new()),
        };
        (next, Effect::None)
    }

    /// Parses the form into the calculation's input.
    pub fn input(&self) -> Result<Input, FormError> {
        let date = |field| {
            self.text(field)
                .and_then(|text| NaiveDate::parse_from_str(text, DATE_FORMAT).ok())
                .ok_or(FormError::InvalidDate(field))
        };
        Ok(Input {
            annual_pay: self.annual_pay.parse().map_err(|_| FormError::InvalidPay)?,
            dismissal: self.dismissal,
            initial_date: date(Field::InitialDate)?,
            current_date: date(Field::CurrentDate)?,
        })
    }

    /// Calculates the form's current values.
    pub fn report(&self) -> Result<Report, FormError> {
        self.input()
            .and_then(|input| calculate(input).map_err(FormError::from))
    }

    /// The form's report today and on each of its next [`FORECAST_YEARS`] anniversaries.
    pub fn forecast(&self) -> Result<Vec<Report>, FormError> {
        self.input()
            .and_then(|input| forecast(input, FORECAST_YEARS).map_err(FormError::from))
    }

    fn text(&self, field: Field) -> Option<&str> {
        match self.value(field) {
            FieldValue::Text(text) => Some(text),
            FieldValue::Choice(_) => None,
        }
    }

    fn with_text(self, field: Field, text: String) -> Self {
        match field {
            Field::AnnualPay => Self {
                annual_pay: text,
                ..self
            },
            Field::InitialDate => Self {
                initial_date: text,
                ..self
            },
            Field::CurrentDate => Self {
                current_date: text,
                ..self
            },
            Field::Dismissal => self,
        }
    }

    fn with_focus(self, change: fn(Field) -> Field) -> Self {
        Self {
            focus: change(self.focus),
            ..self
        }
    }

    fn with_dismissal(self, change: fn(Dismissal) -> Dismissal) -> Self {
        match self.focus {
            Field::Dismissal => Self {
                dismissal: change(self.dismissal),
                ..self
            },
            Field::AnnualPay | Field::InitialDate | Field::CurrentDate => self,
        }
    }

    /// Appends `c` to the focused text field, if the field accepts it.
    fn typed(self, c: char) -> Self {
        if self
            .focus
            .text_rules()
            .is_some_and(|rules| (rules.accepts)(c))
        {
            self.edited(|text| format!("{text}{c}"))
        } else {
            self
        }
    }

    /// Replaces the focused text field with `change` applied to it, unless the result is too
    /// long or the field is not a text field.
    fn edited(self, change: impl FnOnce(&str) -> String) -> Self {
        let field = self.focus;
        let next = self
            .text(field)
            .zip(field.text_rules())
            .map(|(text, rules)| (change(text), rules.max_len))
            .filter(|(next, max_len)| next.chars().count() <= *max_len)
            .map(|(next, _)| next);
        match next {
            Some(text) => self.with_text(field, text),
            None => self,
        }
    }
}

fn without_last_char(text: &str) -> String {
    text.char_indices()
        .next_back()
        .map_or(text, |(last, _)| &text[..last])
        .to_owned()
}

#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::domain::fixtures::python_input;

    /// The form with the Python program's values on 2026-11-07.
    pub fn app() -> App {
        App::from(python_input())
    }

    /// Applies `messages` in order, ignoring effects.
    pub fn apply(app: App, messages: &[Message]) -> App {
        messages.iter().fold(app, |app, &m| app.update(m).0)
    }

    /// One [`Message::Type`] per character.
    pub fn typed(text: &str) -> Vec<Message> {
        text.chars().map(Message::Type).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{app, apply, typed};
    use super::*;
    use crate::domain::fixtures::{date, python_input};
    use Message::{
        Backspace, Clear, NextField, NextOption, PreviousField, PreviousOption, Quit, Type,
    };

    #[test]
    fn backspace_removes_one_character() {
        [
            ("", ""),
            ("5", ""),
            ("53000", "5300"),
            ("2021-10-1ñ", "2021-10-1"),
        ]
        .into_iter()
        .for_each(|(text, expected)| {
            assert_eq!(without_last_char(text), expected, "{text:?}");
        });
    }

    #[test]
    fn maps_keys() {
        [
            (KeyEvent::from(KeyCode::Esc), Some(Quit)),
            (
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Some(Quit),
            ),
            (KeyEvent::from(KeyCode::Tab), Some(NextField)),
            (KeyEvent::from(KeyCode::BackTab), Some(PreviousField)),
            (KeyEvent::from(KeyCode::Right), Some(NextOption)),
            (KeyEvent::from(KeyCode::Left), Some(PreviousOption)),
            (KeyEvent::from(KeyCode::Char('7')), Some(Type('7'))),
            (KeyEvent::from(KeyCode::Backspace), Some(Backspace)),
            (KeyEvent::from(KeyCode::Delete), Some(Clear)),
            (KeyEvent::from(KeyCode::F(1)), None),
        ]
        .into_iter()
        .for_each(|(key, expected)| assert_eq!(message(key), expected, "{key:?}"));
    }

    #[test]
    fn starts_from_the_input() {
        assert_eq!(app().input(), Ok(python_input()));
        assert_eq!(app().focus(), Field::AnnualPay);
    }

    #[test]
    fn focus_wraps_around() {
        assert_eq!(apply(app(), &[PreviousField]).focus(), Field::CurrentDate);
        assert_eq!(apply(app(), &[NextField; 4]).focus(), Field::AnnualPay);
    }

    #[test]
    fn quits() {
        assert_eq!(app().update(Quit).1, Effect::Quit);
    }

    #[test]
    fn edits_follow_each_fields_rules() {
        [
            (
                "the annual pay takes digits and a dot",
                [vec![Clear], typed("6x00.51"), vec![Backspace]].concat(),
                Input {
                    annual_pay: 600.5,
                    ..python_input()
                },
            ),
            (
                "dates take digits and dashes",
                [vec![NextField, NextField, Clear], typed("2020-x01/-15")].concat(),
                Input {
                    initial_date: date(2020, 1, 15),
                    ..python_input()
                },
            ),
            (
                "the current date is limited to ten characters",
                [vec![PreviousField, Clear], typed("2030-06-300")].concat(),
                Input {
                    current_date: date(2030, 6, 30),
                    ..python_input()
                },
            ),
            (
                "the dismissal field ignores typing and editing",
                [vec![NextField], typed("45"), vec![Backspace, Clear]].concat(),
                python_input(),
            ),
            (
                "the dismissal changes only when focused",
                vec![NextOption],
                python_input(),
            ),
            (
                "left and right cycle the dismissal",
                vec![NextField, PreviousOption],
                Input {
                    dismissal: Dismissal::Objective,
                    ..python_input()
                },
            ),
            (
                "the dismissal wraps around",
                vec![NextField, NextOption, NextOption],
                Input {
                    dismissal: Dismissal::Objective,
                    ..python_input()
                },
            ),
        ]
        .into_iter()
        .for_each(|(case, messages, expected)| {
            assert_eq!(apply(app(), &messages).input(), Ok(expected), "{case}");
        });
    }

    #[test]
    fn limits_the_annual_pay_length() {
        let typed = apply(app(), &[Type('9'); 20]);
        assert_eq!(
            typed.value(Field::AnnualPay),
            FieldValue::Text(&format!("53000{}", "9".repeat(MAX_PAY_LEN - 5)))
        );
    }

    #[test]
    fn reports_errors() {
        [
            (vec![Clear], FormError::InvalidPay),
            (
                vec![Clear, Type('0')],
                FormError::Calc(CalcError::NonPositivePay),
            ),
            (
                vec![NextField, NextField, Backspace, Backspace, Backspace],
                FormError::InvalidDate(Field::InitialDate),
            ),
            (
                [vec![PreviousField, Clear], typed("2020-01-01")].concat(),
                FormError::Calc(CalcError::DatesReversed),
            ),
        ]
        .into_iter()
        .for_each(|(messages, error)| {
            assert_eq!(apply(app(), &messages).report(), Err(error), "{error}");
        });
    }
}
