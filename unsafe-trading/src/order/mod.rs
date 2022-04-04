use std::cmp;
use std::collections::{BTreeMap, HashMap};
use std::ops::{Deref, DerefMut};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

mod internals;

pub use internals::*;

#[derive(Debug)]
#[cfg_attr(test, derive(Copy, Clone))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Order {
    pub(crate) id: OrderId,
    pub(crate) initial_kind: OrderKind,
    pub(crate) current_kind: OrderKind,
    pub(crate) side: OrderSide,
    pub(crate) amount: Amount,
    pub(crate) remaining: Amount,
    pub(crate) limit_price: LimitPrice,
    pub(crate) status: OrderStatus,
    pub(crate) created_at: u128,
}

impl Order {
    pub fn new(
        id: OrderId,
        kind: OrderKind,
        side: OrderSide,
        amount: Amount,
        limit_price: LimitPrice,
    ) -> Self {
        Self {
            id,
            initial_kind: kind,
            current_kind: kind,
            side,
            amount,
            remaining: amount,
            limit_price,
            status: OrderStatus::Open,
            created_at: 0,
        }
    }

    pub fn cancel(&mut self) {
        self.update(|order| {
            order.status = if order.remaining == order.amount {
                OrderStatus::Cancelled
            } else {
                OrderStatus::Closed
            };
        });
    }

    pub(crate) fn update<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self),
    {
        f(self)
    }
}

impl Exchangeable for Order {
    type Opposite = Order;

    #[inline]
    fn matches_with(&self, other: &Self::Opposite) -> bool {
        if self.side == OrderSide::Ask && other.side == OrderSide::Bid {
            self.limit_price.le(&other.limit_price)
        } else if self.side == OrderSide::Bid && other.side == OrderSide::Ask {
            self.limit_price.ge(&other.limit_price)
        } else {
            false
        }
    }

    fn trade(&mut self, other: &mut Self::Opposite) -> Option<Trade> {
        if self.matches_with(&other) {
            let amount = cmp::min(self.remaining, other.remaining);
            let price = match self.side {
                OrderSide::Ask => cmp::max(self.limit_price, other.limit_price).0,
                OrderSide::Bid => cmp::min(self.limit_price, other.limit_price).0,
            };

            self.update(|order| {
                order.remaining -= amount;
                order.status = if order.remaining.is_zero() {
                    OrderStatus::Completed
                } else {
                    OrderStatus::Partial
                };
            });

            other.update(|order| {
                order.remaining -= amount;
                order.status = if order.remaining.is_zero() {
                    OrderStatus::Completed
                } else {
                    OrderStatus::Partial
                };
            });

            Some(Trade {
                maker_id: self.id,
                taker_id: other.id,
                amount,
                price,
                created_at: 0,
            })
        } else {
            None
        }
    }
}

#[repr(transparent)]
#[cfg_attr(test, derive(Copy, Clone))]
pub struct AskOrder(Order);

impl Deref for AskOrder {
    type Target = Order;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AskOrder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Exchangeable for AskOrder {
    type Opposite = BidOrder;

    #[inline]
    fn matches_with(&self, other: &Self::Opposite) -> bool {
        self.0.limit_price.le(&other.0.limit_price)
    }

    fn trade(&mut self, other: &mut Self::Opposite) -> Option<Trade> {
        if self.matches_with(&other) {
            self.0.trade(&mut other.0)
        } else {
            None
        }
    }
}

impl From<Order> for AskOrder {
    fn from(order: Order) -> Self {
        Self(order)
    }
}

impl Into<Order> for AskOrder {
    fn into(self) -> Order {
        self.0
    }
}

#[repr(transparent)]
#[cfg_attr(test, derive(Copy, Clone))]
pub struct BidOrder(Order);

impl Deref for BidOrder {
    type Target = Order;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BidOrder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Exchangeable for BidOrder {
    type Opposite = AskOrder;

    #[inline]
    fn matches_with(&self, other: &Self::Opposite) -> bool {
        self.0.limit_price.ge(&other.0.limit_price)
    }

    fn trade(&mut self, other: &mut Self::Opposite) -> Option<Trade> {
        if self.matches_with(&other) {
            self.0.trade(&mut other.0)
        } else {
            None
        }
    }
}

impl From<Order> for BidOrder {
    fn from(order: Order) -> Self {
        Self(order)
    }
}

impl Into<Order> for BidOrder {
    fn into(self) -> Order {
        self.0
    }
}

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Order {}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match self.limit_price.cmp(&other.limit_price) {
            cmp::Ordering::Equal => self.id.cmp(&other.id),
            ord => ord,
        }
    }
}

#[derive(Debug)]
pub struct Trade {
    pub(crate) maker_id: OrderId,
    pub(crate) taker_id: OrderId,
    pub(crate) price: u64,
    pub(crate) amount: Amount,
    pub(crate) created_at: u128,
}

impl Trade {
    pub fn try_new<T>(maker: &mut T, taker: &mut T::Opposite) -> Option<Self>
    where
        T: Exchangeable,
    {
        maker.trade(taker)
    }
}

#[cfg(test)]
mod tests {
    use std::any::{Any, TypeId};

    use super::*;

    mod helpers {
        use super::OrderId;

        use rand::Rng;

        pub(super) fn gen_order_id() -> OrderId {
            let mut rng = rand::thread_rng();
            OrderId(rng.gen())
        }
    }

    const EXAMPLE_ORDER: Order = Order {
        id: OrderId(1),
        side: OrderSide::Ask,
        amount: Amount(100),
        remaining: Amount(100),
        limit_price: LimitPrice(500),
        initial_kind: OrderKind::Limit,
        current_kind: OrderKind::Limit,
        status: OrderStatus::Open,
        created_at: 0,
    };

    #[test]
    fn matching() {
        // Perfect matching
        {
            let mut ask_order = {
                let mut order = EXAMPLE_ORDER.clone();
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order
            };
            let mut bid_order = {
                let mut order = EXAMPLE_ORDER.clone();
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Bid;
                order
            };

            assert!(ask_order.matches_with(&bid_order));
            assert!(ask_order.trade(&mut bid_order).is_some());
        }

        // Taker advantage
        {
            let trade_1a = {
                let mut ask_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Ask;
                    order.limit_price = LimitPrice(400);
                    order
                };
                let mut bid_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Bid;
                    order.limit_price = LimitPrice(500);
                    order
                };
                ask_order.trade(&mut bid_order).unwrap()
            };

            let trade_2a = {
                let mut ask_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Ask;
                    order.limit_price = LimitPrice(400);
                    order
                };
                let mut bid_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Bid;
                    order.limit_price = LimitPrice(500);
                    order
                };
                bid_order.trade(&mut ask_order).unwrap()
            };

            let trade_1b = {
                let mut ask_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Ask;
                    order.limit_price = LimitPrice(400);
                    AskOrder(order)
                };
                let mut bid_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Bid;
                    order.limit_price = LimitPrice(500);
                    BidOrder(order)
                };
                ask_order.trade(&mut bid_order).unwrap()
            };

            let trade_2b = {
                let mut ask_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Ask;
                    order.limit_price = LimitPrice(400);
                    AskOrder(order)
                };
                let mut bid_order = {
                    let mut order = EXAMPLE_ORDER.clone();
                    order.id = helpers::gen_order_id();
                    order.side = OrderSide::Bid;
                    order.limit_price = LimitPrice(500);
                    BidOrder(order)
                };
                bid_order.trade(&mut ask_order).unwrap()
            };

            assert_eq!(trade_1a.price, 500);
            assert_eq!(trade_2a.price, 400);
            assert_eq!(trade_1b.price, 500);
            assert_eq!(trade_2b.price, 400);
        }
    }

    #[test]
    fn no_matching() {
        let trade_1a = {
            let mut ask_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                order
            };
            let mut bid_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Bid;
                order.limit_price = LimitPrice(400);
                order
            };
            ask_order.trade(&mut bid_order)
        };

        let trade_2a = {
            let mut ask_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                order
            };
            let mut bid_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Bid;
                order.limit_price = LimitPrice(400);
                order
            };
            bid_order.trade(&mut ask_order)
        };

        let trade_3 = {
            let mut ask_order_1 = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                order
            };
            let mut ask_order_2 = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                order
            };
            ask_order_1.trade(&mut ask_order_2)
        };

        let trade_1b = {
            let mut ask_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                AskOrder(order)
            };
            let mut bid_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Bid;
                order.limit_price = LimitPrice(400);
                BidOrder(order)
            };
            ask_order.trade(&mut bid_order)
        };

        let trade_2b = {
            let mut ask_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Ask;
                order.limit_price = LimitPrice(500);
                AskOrder(order)
            };
            let mut bid_order = {
                let mut order = EXAMPLE_ORDER;
                order.id = helpers::gen_order_id();
                order.side = OrderSide::Bid;
                order.limit_price = LimitPrice(400);
                BidOrder(order)
            };
            bid_order.trade(&mut ask_order)
        };

        assert!(trade_1a.is_none());
        assert!(trade_2a.is_none());
        assert!(trade_1b.is_none());
        assert!(trade_2b.is_none());
        assert!(trade_3.is_none());
    }

    #[test]
    fn partial_match() {
        let mut ask_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Ask;
            order.amount = Amount(50);
            order.remaining = order.amount;

            order
        };

        let mut bid_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Bid;
            order.amount = Amount(100);
            order.remaining = order.amount;

            order
        };

        let trade = Trade::try_new(&mut ask_order, &mut bid_order);

        assert_eq!(ask_order.status, OrderStatus::Completed);
        assert_eq!(bid_order.status, OrderStatus::Partial);

        let mut ask_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Ask;
            order.amount = Amount(50);
            order.remaining = order.amount;

            order
        };

        let mut bid_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Bid;
            order.amount = Amount(100);
            order.remaining = order.amount;

            order
        };

        let trade = Trade::try_new(&mut bid_order, &mut ask_order);

        assert_eq!(ask_order.status, OrderStatus::Completed);
        assert_eq!(bid_order.status, OrderStatus::Partial);
    }

    #[test]
    fn closed_and_cancelled() {
        let mut open_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.amount = Amount(100);
            order.remaining = order.amount;
            order.status = OrderStatus::Open;
            order
        };
        let mut partial_order = {
            let mut order = open_order;
            order.id = helpers::gen_order_id();
            order.remaining = Amount(50);
            order.status = OrderStatus::Partial;
            order
        };

        assert_eq!(open_order.status, OrderStatus::Open);
        assert_eq!(partial_order.status, OrderStatus::Partial);

        open_order.cancel();
        partial_order.cancel();

        assert_eq!(open_order.status, OrderStatus::Cancelled);
        assert_eq!(partial_order.status, OrderStatus::Closed);
    }

    #[test]
    fn new_trade() {
        let mut ask_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Ask;
            order
        };

        let mut bid_order = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Bid;
            order
        };

        assert!(Trade::try_new(&mut ask_order, &mut bid_order).is_some());

        let mut ask_order_1 = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Ask;
            order
        };

        let mut ask_order_2 = {
            let mut order = EXAMPLE_ORDER;
            order.id = helpers::gen_order_id();
            order.side = OrderSide::Ask;
            order
        };

        assert!(Trade::try_new(&mut ask_order_1, &mut ask_order_2).is_none());
    }

    #[test]
    fn opposite_side() {
        assert_eq!(OrderSide::opposite(&OrderSide::Ask), OrderSide::Bid);
        assert_eq!(OrderSide::opposite(&OrderSide::Bid), OrderSide::Ask);
    }

    #[test]
    fn ordering() {
        let order_1 = {
            let mut order = EXAMPLE_ORDER;
            order.id = OrderId(1);
            order.limit_price = LimitPrice(100);
            order
        };

        let order_2 = {
            let mut order = EXAMPLE_ORDER;
            order.id = OrderId(2);
            order.limit_price = LimitPrice(200);
            order
        };

        let order_3 = {
            let mut order = EXAMPLE_ORDER;
            order.id = OrderId(3);
            order.limit_price = LimitPrice(100);
            order
        };

        assert_eq!(order_1, order_1);
        assert_ne!(order_1, order_2);
        assert_ne!(order_1, order_3);

        assert!(order_1 < order_2);
        assert!(order_1 < order_3);
        assert!(order_2 > order_3);
    }

    #[test]
    fn ask_order_from_into() {
        let (order, ask_order) = {
            let mut order = EXAMPLE_ORDER;
            order.side = OrderSide::Ask;

            let mut ask_order: AskOrder = order.into();
            let order: Order = ask_order.clone().into();

            (order, ask_order)
        };

        assert_eq!(ask_order.type_id(), TypeId::of::<AskOrder>());
        assert_eq!(order.type_id(), TypeId::of::<Order>());
    }

    #[test]
    fn bid_order_from_into() {
        let (order, bid_order) = {
            let mut order = EXAMPLE_ORDER;
            order.side = OrderSide::Bid;

            let bid_order: BidOrder = order.into();
            let order: Order = bid_order.clone().into();

            (order, bid_order)
        };

        assert_eq!(bid_order.type_id(), TypeId::of::<BidOrder>());
        assert_eq!(order.type_id(), TypeId::of::<Order>());
    }
}
