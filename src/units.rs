//! This module defines various unit types and their conversions.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{AddAssign, Mul, SubAssign};

macro_rules! base_unit_struct {
    ($name:ident) => {
        /// A unit type representing a dimensionless value.
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            PartialOrd,
            Serialize,
            Deserialize,
            derive_more::Add,
            derive_more::Sub,
        )]
        pub struct $name(pub f64);

        impl std::ops::Div<$name> for $name {
            type Output = Dimensionless;
            fn div(self, rhs: $name) -> Dimensionless {
                Dimensionless(self.0 / rhs.0)
            }
        }
        impl std::iter::Sum for $name {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold($name(0.0), |a, b| $name(a.0 + b.0))
            }
        }
        impl AddAssign for $name {
            fn add_assign(&mut self, other: Self) {
                self.0 += other.0;
            }
        }
        impl SubAssign for $name {
            fn sub_assign(&mut self, other: Self) {
                self.0 -= other.0;
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl float_cmp::ApproxEq for $name {
            type Margin = float_cmp::F64Margin;
            fn approx_eq<T: Into<Self::Margin>>(self, other: Self, margin: T) -> bool {
                self.0.approx_eq(other.0, margin)
            }
        }
        impl $name {
            /// Returns the underlying f64 value.
            pub fn value(&self) -> f64 {
                self.0
            }
            /// Returns true if the value is a normal number.
            pub fn is_normal(&self) -> bool {
                self.0.is_normal()
            }
            /// Returns true if the value is finite.
            pub fn is_finite(&self) -> bool {
                self.0.is_finite()
            }
            /// Returns the absolute value of this unit.
            pub fn abs(&self) -> Self {
                $name(self.0.abs())
            }
            /// Returns the max of two values
            pub fn max(&self, other: Self) -> Self {
                Self(self.0.max(other.0))
            }
            /// Returns the min of two values
            pub fn min(&self, other: Self) -> Self {
                Self(self.0.min(other.0))
            }
        }
    };
}

// Define Dimensionless first
base_unit_struct!(Dimensionless);

// Add extra methods for Dimensionless
impl From<f64> for Dimensionless {
    fn from(val: f64) -> Self {
        Self(val)
    }
}
impl From<Dimensionless> for f64 {
    fn from(val: Dimensionless) -> Self {
        val.0
    }
}

impl Mul for Dimensionless {
    type Output = Dimensionless;

    fn mul(self, rhs: Self) -> Self::Output {
        Dimensionless(self.0 * rhs.0)
    }
}

impl Dimensionless {
    /// Raises this dimensionless number to the power of `rhs`.
    pub fn powi(self, rhs: i32) -> Self {
        Dimensionless(self.0.powi(rhs))
    }
}

// Define all other units with Dimensionless interactions
macro_rules! unit_struct {
    ($name:ident) => {
        base_unit_struct!($name);

        impl std::ops::Mul<Dimensionless> for $name {
            type Output = $name;
            fn mul(self, rhs: Dimensionless) -> $name {
                $name(self.0 * rhs.0)
            }
        }
        impl std::ops::Mul<$name> for Dimensionless {
            type Output = $name;
            fn mul(self, rhs: $name) -> $name {
                $name(self.0 * rhs.0)
            }
        }
        impl std::ops::Div<Dimensionless> for $name {
            type Output = $name;
            fn div(self, rhs: Dimensionless) -> $name {
                $name(self.0 / rhs.0)
            }
        }
    };
}

// Base quantities
unit_struct!(Money);
unit_struct!(Flow);
unit_struct!(Activity);
unit_struct!(Capacity);
unit_struct!(Year);

// Derived quantities
unit_struct!(MoneyPerYear);
unit_struct!(MoneyPerFlow);
unit_struct!(MoneyPerCapacity);
unit_struct!(MoneyPerCapacityPerYear);
unit_struct!(MoneyPerActivity);
unit_struct!(ActivityPerCapacity);
unit_struct!(FlowPerActivity);

macro_rules! impl_div {
    ($Lhs:ident, $Rhs:ident, $Out:ident) => {
        impl std::ops::Div<$Rhs> for $Lhs {
            type Output = $Out;
            fn div(self, rhs: $Rhs) -> $Out {
                $Out(self.0 / rhs.0)
            }
        }
        impl std::ops::Mul<$Rhs> for $Out {
            type Output = $Lhs;
            fn mul(self, by: $Rhs) -> $Lhs {
                $Lhs(self.0 * by.0)
            }
        }
        impl std::ops::Mul<$Lhs> for $Out {
            type Output = $Rhs;
            fn mul(self, by: $Lhs) -> $Rhs {
                $Rhs(self.0 * by.0)
            }
        }
        impl std::ops::Mul<$Out> for $Rhs {
            type Output = $Lhs;
            fn mul(self, by: $Out) -> $Lhs {
                $Lhs(self.0 * by.0)
            }
        }
        impl std::ops::Mul<$Out> for $Lhs {
            type Output = $Rhs;
            fn mul(self, by: $Out) -> $Rhs {
                $Rhs(self.0 * by.0)
            }
        }
    };
}

// Division rules for derived quantities
impl_div!(Flow, Activity, FlowPerActivity);
impl_div!(Money, Year, MoneyPerYear);
impl_div!(Money, Flow, MoneyPerFlow);
impl_div!(Money, Capacity, MoneyPerCapacity);
impl_div!(Money, Activity, MoneyPerActivity);
impl_div!(Activity, Capacity, ActivityPerCapacity);
impl_div!(MoneyPerYear, Capacity, MoneyPerCapacityPerYear);
impl_div!(MoneyPerActivity, FlowPerActivity, MoneyPerFlow);
