//! This module defines various unit types and their conversions.

/// Represents a dimensionless quantity.
#[derive(Debug, Clone, Copy, PartialEq, derive_more::Add, derive_more::Sub)]
pub struct Dimensionless(pub f64);

impl std::ops::Mul for Dimensionless {
    type Output = Dimensionless;

    fn mul(self, rhs: Dimensionless) -> Self::Output {
        Dimensionless::from(self.0 * rhs.0)
    }
}

impl std::ops::Div for Dimensionless {
    type Output = Dimensionless;

    fn div(self, rhs: Dimensionless) -> Self::Output {
        Dimensionless::from(self.0 / rhs.0)
    }
}

impl Dimensionless {
    /// Raise the dimensionless quantity to the power of an integer.
    pub fn powi(self, rhs: i32) -> Self {
        Dimensionless::from(self.0.powi(rhs))
    }
}

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

macro_rules! unit_struct {
    ($name:ident) => {
        /// Represents a type of quantity.
        #[derive(Debug, Clone, Copy, PartialEq, derive_more::Add, derive_more::Sub)]
        pub struct $name(pub f64);

        impl $name {
            /// Creates a new instance of the unit type from a f64 value.
            pub fn from(val: f64) -> Self {
                Self(val)
            }

            /// Returns the value of the unit type as a f64.
            pub fn value(self) -> f64 {
                self.0
            }
        }

        impl std::ops::Mul<Dimensionless> for $name {
            type Output = $name;
            fn mul(self, rhs: Dimensionless) -> $name {
                $name::from(self.0 * rhs.0)
            }
        }

        impl std::ops::Mul<$name> for Dimensionless {
            type Output = $name;
            fn mul(self, rhs: $name) -> $name {
                $name::from(self.0 * rhs.0)
            }
        }

        impl std::ops::Div<Dimensionless> for $name {
            type Output = $name;
            fn div(self, rhs: Dimensionless) -> $name {
                $name::from(self.0 / rhs.0)
            }
        }
    };
}

macro_rules! impl_div {
    ($Lhs:ty, $Rhs:ty, $Out:ty) => {
        impl std::ops::Div<$Rhs> for $Lhs {
            type Output = $Out;
            fn div(self, rhs: $Rhs) -> $Out {
                <$Out>::from(self.0 / rhs.0)
            }
        }

        impl std::ops::Mul<$Rhs> for $Out {
            type Output = $Lhs;
            fn mul(self, by: $Rhs) -> $Lhs {
                <$Lhs>::from(self.0 * by.0)
            }
        }

        impl std::ops::Mul<$Lhs> for $Out {
            type Output = $Rhs;
            fn mul(self, by: $Lhs) -> $Rhs {
                <$Rhs>::from(self.0 * by.0)
            }
        }

        impl std::ops::Mul<$Out> for $Rhs {
            type Output = $Lhs;
            fn mul(self, by: $Out) -> $Lhs {
                <$Lhs>::from(self.0 * by.0)
            }
        }

        impl std::ops::Mul<$Out> for $Lhs {
            type Output = $Rhs;
            fn mul(self, by: $Out) -> $Rhs {
                <$Rhs>::from(self.0 * by.0)
            }
        }
    };
}

// Base quantities
unit_struct!(Money);
unit_struct!(Energy);
unit_struct!(Activity);
unit_struct!(Capacity);
unit_struct!(Year);

// Derived quantities
unit_struct!(EnergyPerYear);
unit_struct!(MoneyPerYear);
unit_struct!(MoneyPerEnergy);
unit_struct!(MoneyPerCapacity);
unit_struct!(EnergyPerCapacityPerYear);
unit_struct!(MoneyPerCapacityPerYear);
unit_struct!(MoneyPerEnergyPerYear);
unit_struct!(MoneyPerActivity);
unit_struct!(MoneyPerActivityPerYear);
unit_struct!(ActivityPerCapacity);

// Simple relationships
impl_div!(Energy, Year, EnergyPerYear);
impl_div!(Money, Year, MoneyPerYear);
impl_div!(Money, Energy, MoneyPerEnergy);
impl_div!(Money, Capacity, MoneyPerCapacity);
impl_div!(Money, Activity, MoneyPerActivity);
impl_div!(Activity, Capacity, ActivityPerCapacity);

// Complex relationships
impl_div!(EnergyPerYear, Capacity, EnergyPerCapacityPerYear);
impl_div!(MoneyPerYear, Capacity, MoneyPerCapacityPerYear);
impl_div!(Money, EnergyPerYear, MoneyPerEnergyPerYear);
impl_div!(MoneyPerEnergy, Year, MoneyPerEnergyPerYear);
impl_div!(MoneyPerYear, Activity, MoneyPerActivityPerYear);

/// Represents a number of years as an integer.
#[derive(Debug, Clone, Copy, PartialEq, derive_more::Add, derive_more::Sub)]
pub struct IYear(pub u32);
