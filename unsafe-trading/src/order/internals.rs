use std::ops::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::Trade;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub enum OrderKind {
    Limit = 1,
    Market = 2,
    Stop = 3,
    Trailing = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub enum OrderSide {
    Ask = 1,
    Bid = 2,
}

impl OrderSide {
    #[inline]
    pub const fn opposite(&self) -> Self {
        match self {
            OrderSide::Ask => OrderSide::Bid,
            OrderSide::Bid => OrderSide::Ask,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub enum OrderStatus {
    Open = 1,
    Partial = 2,
    Completed = 3,
    Closed = 4,
    Cancelled = 5,
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct OrderId(pub(crate) u64);

impl OrderId {
    pub fn new(order_id: u64) -> Self {
        Self(order_id)
    }
}

impl Deref for OrderId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OrderId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct LimitPrice(pub(crate) u64);

impl LimitPrice {
    pub fn new(limit_price: u64) -> Self {
        Self(limit_price)
    }
}

impl Deref for LimitPrice {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LimitPrice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct Amount(pub(crate) u64);

impl Amount {
    pub fn new(amount: u64) -> Self {
        Self(amount)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Deref for Amount {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Amount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Amount(*self + *rhs)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Amount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Amount(*self - *rhs)
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

pub trait Exchangeable {
    type Opposite;
    fn matches_with(&self, other: &Self::Opposite) -> bool;
    fn trade(&mut self, other: &mut Self::Opposite) -> Option<Trade>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_add_sub_ops() {
        let mut amount_1 = Amount(10);
        let amount_2 = Amount(20);

        assert_eq!(amount_1 + amount_2, Amount(30));
        assert_eq!(amount_2 - amount_1, Amount(10));

        amount_1 += amount_2;

        assert_eq!(amount_1, Amount(30));

        amount_1 -= amount_2;

        assert_eq!(amount_1, Amount(10));
    }
}
