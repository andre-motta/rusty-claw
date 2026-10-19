//! The objects of a boss fight that are not the boss himself: on the
//! thirteenth level, RedTail's stakes ([`spikes`]), his wind ([`wind`])
//! and the debris it blows ([`debris`]), his knives ([`knife`]) and his
//! flares ([`flare`]). RedTail himself is an enemy
//! ([`super::enemy::redtail`]).
//!
//! None of them is built yet (each family's `BUILT` is false): the
//! stakes and the wind, which the level places, stay inert placeholders,
//! and the knives, flares and debris, which only RedTail and his wind
//! create, are never made.

pub mod debris;
pub mod flare;
pub mod knife;
pub mod spikes;
pub mod wind;
