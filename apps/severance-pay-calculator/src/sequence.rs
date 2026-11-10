//! Helpers for ordered sequences, shared by the domain and the UI.

/// A type with a fixed, ordered set of values that can be stepped through, wrapping around.
pub trait Cycle: Copy + PartialEq + 'static {
    /// Every value, in order.
    const ALL: &'static [Self];

    /// The value after this one; the first one after the last.
    fn next(self) -> Self {
        shifted(self, 1)
    }

    /// The value before this one; the last one before the first.
    fn previous(self) -> Self {
        shifted(self, Self::ALL.len().saturating_sub(1))
    }
}

/// The value `by` places after `value`, wrapping around. A value missing from `ALL` stays put.
fn shifted<T: Cycle>(value: T, by: usize) -> T {
    T::ALL
        .iter()
        .position(|candidate| *candidate == value)
        .and_then(|index| (index + by).checked_rem(T::ALL.len()))
        .and_then(|index| T::ALL.get(index).copied())
        .unwrap_or(value)
}

/// Chains `groups`, putting `separator()` between consecutive groups.
///
/// A stable, group-level stand-in for the unstable `Iterator::intersperse`.
pub fn separated<T, G: IntoIterator<Item = T>>(
    groups: impl IntoIterator<Item = G>,
    separator: impl Fn() -> T,
) -> impl Iterator<Item = T> {
    groups
        .into_iter()
        .enumerate()
        .flat_map(move |(index, group)| (index > 0).then(&separator).into_iter().chain(group))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Light {
        Red,
        Amber,
        Green,
    }

    impl Cycle for Light {
        const ALL: &'static [Self] = &[Self::Red, Self::Amber, Self::Green];
    }

    #[test]
    fn cycles_wrapping_around() {
        [
            (Light::Red.next(), Light::Amber),
            (Light::Green.next(), Light::Red),
            (Light::Red.previous(), Light::Green),
            (Light::Amber.previous(), Light::Red),
        ]
        .into_iter()
        .for_each(|(actual, expected)| assert_eq!(actual, expected));
    }

    #[test]
    fn separates_groups() {
        let joined = |groups: Vec<Vec<i32>>| separated(groups, || 0).collect::<Vec<_>>();
        assert_eq!(joined(vec![]), Vec::<i32>::new());
        assert_eq!(joined(vec![vec![1, 2]]), [1, 2]);
        assert_eq!(joined(vec![vec![1], vec![], vec![2, 3]]), [1, 0, 0, 2, 3]);
    }
}
