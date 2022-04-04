#![allow(dead_code, unused)]

use unsafe_trading::{Amount, LimitPrice, Order, OrderId, OrderKind, OrderSide, TradingEngine};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "UPPERCASE")]
pub enum OrderRequest {
    Create {
        kind: OrderKind,
        side: OrderSide,
        limit_price: u64,
        amount: u64,
    },
    Delete(usize),
}

impl TryInto<Order> for OrderRequest {
    type Error = ();

    fn try_into(self) -> Result<Order, Self::Error> {
        match self {
            OrderRequest::Create {
                kind,
                side,
                limit_price,
                amount,
            } => Ok(Order::new(
                OrderId::new(1),
                kind,
                side,
                Amount::new(amount),
                LimitPrice::new(limit_price),
            )),
            OrderRequest::Delete(_) => Err(()),
        }
    }
}

fn main() {
    let trading_engine = TradingEngine::default();

    let order_request = OrderRequest::Create {
        kind: OrderKind::Limit,
        side: OrderSide::Ask,
        limit_price: 10_000,
        amount: 50,
    };
    println!("{}", serde_json::to_string_pretty(&order_request).unwrap());

    let order: Order = order_request.try_into().unwrap();
    println!("{}", serde_json::to_string_pretty(&order).unwrap());
}
