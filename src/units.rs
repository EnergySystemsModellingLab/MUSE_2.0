#![allow(missing_docs)]

use derive_more::{Add, Sub};

macro_rules! unit_struct {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Add, Sub)]
        pub struct $name(pub f64);

        impl $name {
            pub fn from(val: f64) -> Self {
                Self(val)
            }

            pub fn value(self) -> f64 {
                self.0
            }
        }
    };
}

macro_rules! impl_mul {
    ($Lhs:ty, $Rhs:ty, $Out:ty) => {
        impl std::ops::Mul<$Rhs> for $Lhs {
            type Output = $Out;
            fn mul(self, rhs: $Rhs) -> $Out {
                <$Out>::from(self.0 * rhs.0)
            }
        }
        impl std::ops::Mul<$Lhs> for $Rhs {
            type Output = $Out;
            fn mul(self, lhs: $Lhs) -> $Out {
                <$Out>::from(self.0 * lhs.0)
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
    };
}

unit_struct!(Dimensionless);
unit_struct!(Money);
unit_struct!(Year);
unit_struct!(Capacity);
unit_struct!(Commodity);

#[derive(Debug, Clone, Copy, PartialEq, derive_more::Add, derive_more::Sub)]
pub struct IYear(pub u32);

unit_struct!(CommodityPerYear);
unit_struct!(MoneyPerYear);
unit_struct!(MoneyPerCommodity);
unit_struct!(MoneyPerCapacity);
unit_struct!(CommodityPerYearPerCapacity);
unit_struct!(MoneyPerYearPerCapacity);
unit_struct!(MoneyPerCommodityPerYear);
unit_struct!(PerYear);

impl_div!(Commodity, Year, CommodityPerYear);
impl_div!(Money, Year, MoneyPerYear);
impl_div!(Money, Commodity, MoneyPerCommodity);
impl_div!(CommodityPerYear, Capacity, CommodityPerYearPerCapacity);
impl_div!(MoneyPerYear, Capacity, MoneyPerYearPerCapacity);
impl_div!(MoneyPerCommodity, Year, MoneyPerCommodityPerYear);
impl_div!(Dimensionless, Year, PerYear);
impl_div!(Money, Capacity, MoneyPerCapacity);

impl_mul!(Dimensionless, Year, Year);
impl_mul!(Dimensionless, Capacity, Capacity);
impl_mul!(Dimensionless, Commodity, Commodity);
impl_mul!(Dimensionless, Money, Money);
impl_mul!(MoneyPerCapacity, Capacity, Money);
impl_mul!(MoneyPerYearPerCapacity, Capacity, MoneyPerYear);
impl_mul!(Money, PerYear, MoneyPerYear);
impl_mul!(Year, PerYear, Dimensionless);

impl IYear {
    pub fn from(val: u32) -> Self {
        Self(val)
    }

    pub fn value(self) -> u32 {
        self.0
    }

    pub fn to_year(self) -> Year {
        Year::from(self.0 as f64)
    }
}

impl Dimensionless {
    pub fn pow(self, rhs: IYear) -> Self {
        Dimensionless::from(self.0.powi(rhs.0 as i32))
    }
}

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
