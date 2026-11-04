//! Reusable UI building blocks, organised by atomic design.
//!
//! - [`tone`]: semantic color roles (foundation). Components take a
//!   [`Tone`], never a raw color, so restyling happens in one place.
//! - [`atoms`]: smallest pieces: button, badge, chip, tooltip, text.
//! - [`molecules`]: atoms combined: banner, data table.
//!
//! Feature views (organisms such as the draft banner) compose these and
//! hold no styling of their own. Every component is generic over the
//! message type so any feature can use it.
//!
//! Owned by 09 (design tokens). New shared visuals go here, not into a
//! feature file; feature-specific layouts stay with their feature.

pub mod atoms;
pub mod molecules;
pub mod tone;

pub use tone::Tone;
