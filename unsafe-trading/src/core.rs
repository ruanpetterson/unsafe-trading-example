use std::pin::Pin;
use std::ptr::NonNull;
use std::{collections::BTreeMap, fmt::Debug};

use colored::*;

use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::order::{
    Amount, Exchangeable, LimitPrice, Order, OrderId, OrderKind, OrderSide, OrderStatus, Trade,
};

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct TradingEngine {
    #[cfg_attr(feature = "serde", serde(skip_serializing))]
    orders: IndexMap<OrderId, Pin<Box<Order>>>,
    orderbook: Orderbook,
    events: Vec<TradingEngineResponse>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TradingEngineResponse {
    OrderReceived {
        id: OrderId,
    },
    OrderAddedToOrderbook {
        id: OrderId,
    },
    OrderPartiallyFilled {
        id: OrderId,
        previous_remaining: Amount,
        current_remaining: Amount,
    },
    OrderCompleted {
        id: OrderId,
    },
    OrderReceivedCompletedBeforeEnterInOrderbook {
        id: OrderId,
    },
    OrderRemovedFromOrderbook {
        id: OrderId,
    },
}

impl Debug for TradingEngineResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingEngineResponse::OrderReceived { id } => {
                write!(f, "{} Order {} received", "[BEGIN]".green().bold(), id.0)
            }
            TradingEngineResponse::OrderAddedToOrderbook { id } => {
                write!(
                    f,
                    "{}   Order {} added to orderbook\n",
                    "[END]".cyan().bold(),
                    id.0
                )
            }
            TradingEngineResponse::OrderPartiallyFilled {
                id,
                previous_remaining,
                current_remaining,
            } => write!(
                f,
                "        Order {} partially filled (current: {}, previous: {})",
                id.0, current_remaining.0, previous_remaining.0
            ),
            TradingEngineResponse::OrderCompleted { id } => {
                write!(f, "        Order {} completed", id.0)
            }
            TradingEngineResponse::OrderRemovedFromOrderbook { id } => {
                write!(f, "        Order {} removed from orderbook", id.0)
            }
            TradingEngineResponse::OrderReceivedCompletedBeforeEnterInOrderbook { id } => write!(
                f,
                "{}   Order {} completed before entered in orderbook\n",
                "[END]".cyan().bold(),
                id.0
            ),
        }
    }
}

impl Default for TradingEngine {
    fn default() -> Self {
        Self {
            orders: IndexMap::with_capacity(1024),
            orderbook: Orderbook::default(),
            events: Vec::default(),
        }
    }
}

impl TradingEngine {
    fn new() -> Self {
        Self::default()
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            orders: IndexMap::with_capacity(capacity),
            orderbook: Orderbook::default(),
            events: Vec::default(),
        }
    }
}

impl TradingEngine {
    fn insert(&mut self, order: Order) {
        let order_id = order.id;

        // Pin Order in heap
        let mut boxed = Box::pin(order);

        // Get Order address
        let pin = Pin::as_mut(&mut boxed);
        let ptr = NonNull::from(pin.get_mut());

        // Insert Order in index with its pointer
        self.orders.insert(order_id, boxed);

        // Insert Order in orders lists
        self.orderbook.insert(ptr);
    }

    pub fn try_insert(&mut self, mut order: Order) -> Result<(), ()> {
        let order_id = order.id;

        if self.get(&order_id).is_some() {
            return Err(());
        }

        self.events
            .push(TradingEngineResponse::OrderReceived { id: order.id });

        while let Some(mut top_order) = self.pop_from_orderbook(&order) {
            if let Some(trade) = order.trade(&mut top_order) {
                let trade_amount = trade.amount;
                let trade_price = trade.price;

                let (incoming_order_status, top_order_status) = (order.status, top_order.status);

                match (incoming_order_status, top_order_status) {
                    (OrderStatus::Partial, OrderStatus::Completed) => {
                        self.events
                            .push(TradingEngineResponse::OrderPartiallyFilled {
                                id: order_id,
                                previous_remaining: order.remaining + trade.amount,
                                current_remaining: order.remaining,
                            });
                        self.events
                            .push(TradingEngineResponse::OrderCompleted { id: top_order.id });
                        self.events
                            .push(TradingEngineResponse::OrderRemovedFromOrderbook {
                                id: top_order.id,
                            });
                        continue;
                    }
                    (OrderStatus::Open, OrderStatus::Open)
                    | (OrderStatus::Open, OrderStatus::Partial)
                    | (OrderStatus::Partial, OrderStatus::Partial)
                    | (OrderStatus::Completed, OrderStatus::Partial) => {
                        self.events
                            .push(TradingEngineResponse::OrderPartiallyFilled {
                                id: top_order.id,
                                previous_remaining: top_order.remaining + trade.amount,
                                current_remaining: top_order.remaining,
                            });
                        self.insert(top_order);
                        self.events
                            .push(TradingEngineResponse::OrderCompleted { id: order.id });
                        break;
                    }
                    (OrderStatus::Completed, OrderStatus::Completed) => {
                        self.events
                            .push(TradingEngineResponse::OrderCompleted { id: top_order.id });
                        self.events
                            .push(TradingEngineResponse::OrderRemovedFromOrderbook {
                                id: top_order.id,
                            });
                        self.events
                            .push(TradingEngineResponse::OrderCompleted { id: order.id });
                        break;
                    }
                    _ => unreachable!(),
                }
            }
        }

        if order.status != OrderStatus::Completed && order.current_kind == OrderKind::Limit {
            self.events
                .push(TradingEngineResponse::OrderAddedToOrderbook { id: order.id });
            self.insert(order);
        } else {
            self.events.push(
                TradingEngineResponse::OrderReceivedCompletedBeforeEnterInOrderbook {
                    id: order.id,
                },
            );
        }

        Ok(())
    }

    pub fn remove(&mut self, order_id: &OrderId) -> Option<Order> {
        let pin = self.orders.remove(order_id)?;
        Some(*Pin::into_inner(pin))
    }

    #[must_use]
    pub fn get(&self, order_id: &OrderId) -> Option<&Order> {
        let order = self.orders.get(order_id)?;

        // SAFETY: if Order is in indexes, it should be a valid pointer.
        unsafe { Some(Pin::into_inner(order.as_ref())) }
    }

    #[must_use]
    pub fn get_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        let order = self.orders.get_mut(order_id)?;

        // SAFETY: if Order is in indexes, it should be a valid pointer.
        unsafe { Some(Pin::into_inner(order.as_mut())) }
    }

    #[must_use]
    pub fn pop_from_orderbook(&mut self, opposite_order: &Order) -> Option<Order> {
        let order = self.orderbook.pop(opposite_order)?;
        let order_id = unsafe { order.as_ref() }.id;

        self.remove(&order_id)
    }
}

type Orders = BTreeMap<OrderId, NonNull<Order>>;
type Levels = BTreeMap<LimitPrice, Orders>;
type Sides = IndexMap<OrderSide, Levels>;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Orderbook {
    #[cfg_attr(feature = "serde", serde(skip_serializing))]
    sides: Sides,
    ask_length: Amount,
    bid_length: Amount,
}

impl Default for Orderbook {
    fn default() -> Self {
        let mut sides = IndexMap::new();
        sides.insert(OrderSide::Ask, BTreeMap::default());
        sides.insert(OrderSide::Bid, BTreeMap::default());

        Self {
            sides,
            ask_length: Amount(0),
            bid_length: Amount(0),
        }
    }
}

impl Orderbook {
    fn pop(&mut self, incoming_order: &Order) -> Option<NonNull<Order>> {
        let opposite_side = incoming_order.side.opposite();

        let (_level_limit_price, orders) = match incoming_order.side {
            OrderSide::Ask => self.sides.get(&opposite_side)?.iter().rev().next()?,
            OrderSide::Bid => self.sides.get(&opposite_side)?.iter().next()?,
        };

        let (_order_id, order) = orders.iter().next()?;
        let order = unsafe { order.as_ref() };

        self.remove(order)
    }

    fn insert(&mut self, order: NonNull<Order>) {
        // Matching algorithm
        let (id, side, limit_price, remaining) = {
            let order = unsafe { order.as_ref() };

            (order.id, order.side, order.limit_price, order.remaining)
        };

        match side {
            OrderSide::Ask => self.ask_length += remaining,
            OrderSide::Bid => self.bid_length += remaining,
        }

        self.sides
            .entry(side)
            .or_insert_with(Default::default)
            .entry(limit_price)
            .or_insert_with(Default::default)
            .insert(id, order);
    }

    fn remove(&mut self, order: &Order) -> Option<NonNull<Order>> {
        // Remove remaing orders from total count
        match order.side {
            OrderSide::Ask => self.ask_length -= order.remaining,
            OrderSide::Bid => self.bid_length -= order.remaining,
        }

        let side = order.side;
        let limit_price = order.limit_price;

        // Remove order from tree
        let level = self.sides.get_mut(&side)?.get_mut(&limit_price)?;
        let ptr = level.remove(&order.id);

        // If level is empty, remove it
        if level.is_empty() {
            self.sides.get_mut(&side)?.remove(&limit_price)?;
        }

        ptr
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Scheduler;

impl Scheduler {
    pub fn insert(&mut self, order: Pin<Box<Order>>) {
        todo!()
    }
    pub fn remove(&mut self, order: &Order) -> Option<Order> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::time;

    use super::*;

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
    fn it_works() {
        let mut trading_engine = TradingEngine::with_capacity(1024);
        for i in 1..=11 {
            let mut order = EXAMPLE_ORDER;
            order.id = OrderId(i);
            assert!(trading_engine.try_insert(order).is_ok());
        }

        for i in 12..=17 {
            let mut order = EXAMPLE_ORDER;
            order.id = OrderId(i);
            order.side = OrderSide::Bid;
            order.amount = Amount(200);
            order.remaining = order.amount;
            assert!(trading_engine.try_insert(order).is_ok());
        }

        for event in &trading_engine.events {
            eprintln!("{:?}", event);
        }
    }
}
