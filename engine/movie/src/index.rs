//! The game's movie numbers and the files and archive entries they name.
//!
//! | Index | Moment | File stem | Archive entry |
//! | --- | --- | --- | --- |
//! | -2 | the attract clip | none | 1 |
//! | -1 | the logo at start-up | `LOGO` | 1 |
//! | 0 | the intro at start-up | `CLAW0` | 2 |
//! | 2, 4, 6, 8, 10, 12 | after that level | `CLAW1` .. `CLAW6` | 3 .. 8 |
//! | 13 | after level 13 | `CLAW7` | 9 |
//! | 14 | the end of the game, after level 14 | `CLAW8` | 10 |
//!
//! The odd indices 1 to 11 name no movie. Archive entries count from 1.

use std::collections::BTreeSet;
use std::fmt;

/// A movie number, -2 to 14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MovieIndex(i8);

impl MovieIndex {
    /// The attract clip.
    pub const ATTRACT: MovieIndex = MovieIndex(-2);
    /// The logo at start-up.
    pub const LOGO: MovieIndex = MovieIndex(-1);
    /// The intro at start-up.
    pub const INTRO: MovieIndex = MovieIndex(0);
    /// The lowest index.
    pub const MIN: i8 = -2;
    /// The highest index.
    pub const MAX: i8 = 14;

    /// The index `n`, when it is in -2..=14.
    pub fn new(n: i8) -> Option<MovieIndex> {
        (Self::MIN..=Self::MAX)
            .contains(&n)
            .then_some(MovieIndex(n))
    }

    /// Every index, -2 to 14.
    pub fn all() -> impl Iterator<Item = MovieIndex> {
        (Self::MIN..=Self::MAX).map(MovieIndex)
    }

    /// The number.
    pub fn get(self) -> i8 {
        self.0
    }

    /// The movie that plays after retail level `level`: after levels 2, 4,
    /// 6, 8, 10, 12, 13 and 14 (the end of the game); no other level has
    /// one.
    pub fn after_level(level: u32) -> Option<MovieIndex> {
        match level {
            2 | 4 | 6 | 8 | 10 | 12 | 13 | 14 => Some(MovieIndex(level as i8)),
            _ => None,
        }
    }

    /// The movie's file name without its extension, when it has a file.
    pub fn stem(self) -> Option<&'static str> {
        Some(match self.0 {
            -1 => "LOGO",
            0 => "CLAW0",
            2 => "CLAW1",
            4 => "CLAW2",
            6 => "CLAW3",
            8 => "CLAW4",
            10 => "CLAW5",
            12 => "CLAW6",
            13 => "CLAW7",
            14 => "CLAW8",
            _ => return None,
        })
    }

    /// The movie's entry in the movie archive, counting from 1, when it has
    /// one.
    pub fn fec_entry(self) -> Option<u16> {
        Some(match self.0 {
            -2 | -1 => 1,
            0 => 2,
            2 => 3,
            4 => 4,
            6 => 5,
            8 => 6,
            10 => 7,
            12 => 8,
            13 => 9,
            14 => 10,
            _ => return None,
        })
    }
}

impl fmt::Display for MovieIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stem() {
            Some(stem) => write!(f, "movie {} ({stem})", self.0),
            None => write!(f, "movie {}", self.0),
        }
    }
}

/// A set of movie indices, for example the movies a run can play.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovieSet(BTreeSet<MovieIndex>);

impl MovieSet {
    /// An empty set.
    pub fn new() -> MovieSet {
        MovieSet::default()
    }

    /// Whether `index` is in the set.
    pub fn contains(&self, index: MovieIndex) -> bool {
        self.0.contains(&index)
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds `index`; true when it was not there yet.
    pub fn insert(&mut self, index: MovieIndex) -> bool {
        self.0.insert(index)
    }

    /// The indices in order.
    pub fn iter(&self) -> impl Iterator<Item = MovieIndex> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<MovieIndex> for MovieSet {
    fn from_iter<I: IntoIterator<Item = MovieIndex>>(iter: I) -> MovieSet {
        MovieSet(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_run_from_minus_two_to_fourteen() {
        assert_eq!(MovieIndex::new(-3), None);
        assert_eq!(MovieIndex::new(15), None);
        assert_eq!(MovieIndex::new(-2), Some(MovieIndex::ATTRACT));
        assert_eq!(MovieIndex::new(-1), Some(MovieIndex::LOGO));
        assert_eq!(MovieIndex::new(0), Some(MovieIndex::INTRO));
        assert_eq!(MovieIndex::all().count(), 17);
    }

    #[test]
    fn the_map_of_stems_and_entries() {
        let rows: Vec<(i8, Option<&str>, Option<u16>)> = MovieIndex::all()
            .map(|i| (i.get(), i.stem(), i.fec_entry()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (-2, None, Some(1)),
                (-1, Some("LOGO"), Some(1)),
                (0, Some("CLAW0"), Some(2)),
                (1, None, None),
                (2, Some("CLAW1"), Some(3)),
                (3, None, None),
                (4, Some("CLAW2"), Some(4)),
                (5, None, None),
                (6, Some("CLAW3"), Some(5)),
                (7, None, None),
                (8, Some("CLAW4"), Some(6)),
                (9, None, None),
                (10, Some("CLAW5"), Some(7)),
                (11, None, None),
                (12, Some("CLAW6"), Some(8)),
                (13, Some("CLAW7"), Some(9)),
                (14, Some("CLAW8"), Some(10)),
            ]
        );
    }

    #[test]
    fn movies_follow_the_even_levels_and_the_last_two() {
        let after: Vec<(u32, i8)> = (0..=16)
            .filter_map(|l| MovieIndex::after_level(l).map(|i| (l, i.get())))
            .collect();
        assert_eq!(
            after,
            vec![
                (2, 2),
                (4, 4),
                (6, 6),
                (8, 8),
                (10, 10),
                (12, 12),
                (13, 13),
                (14, 14)
            ]
        );
    }

    #[test]
    fn a_set_holds_each_index_once() {
        let mut set = MovieSet::default();
        assert!(set.is_empty());
        assert!(set.insert(MovieIndex::LOGO));
        assert!(!set.insert(MovieIndex::LOGO));
        assert!(set.contains(MovieIndex::LOGO));
        assert!(!set.contains(MovieIndex::INTRO));
        let all: MovieSet = MovieIndex::all().collect();
        assert_eq!(all.iter().count(), 17);
    }
}
