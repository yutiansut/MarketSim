use crate::utility::{gen_order_id};


/// Enum for matching over order types
#[derive(Debug, PartialEq)]
pub enum OrderType {
    Enter,
    Update,
    Cancel,
}

impl Clone for OrderType {
	fn clone(&self) -> OrderType { 
		match self {
			OrderType::Enter => OrderType::Enter,
			OrderType::Update => OrderType::Update,
			OrderType::Cancel => OrderType::Cancel,
		}
	}
}


// Enum for matching over bid or ask
#[derive(Debug, PartialEq)]
pub enum TradeType {
    Bid,
    Ask,
}

impl Clone for TradeType {
	fn clone(&self) -> TradeType { 
		match self {
			TradeType::Ask => TradeType::Ask,
			TradeType::Bid => TradeType::Bid,
		}
	}
}

// Enum for matching over LimitOrders and FlowOrders
#[derive(Debug, PartialEq)]
pub enum ExchangeType {
    LimitOrder,
    FlowOrder,
}

impl Clone for ExchangeType {
	fn clone(&self) -> ExchangeType { 
		match self {
			ExchangeType::LimitOrder => ExchangeType::LimitOrder,
			ExchangeType::FlowOrder => ExchangeType::FlowOrder,
		}
	}
}

/// The internal data structure that any exchange format will operate on. 
/// trader_id: String -> identifier of the trader and their order
/// order_id: u64 -> identifier for an order in case a trader has multiple orders
/// order_type: OrderType{Enter, Update, Cancel} -> identifies how the order is used by the exchange
/// trade_type: TradeType{Bid, Ask} -> decides which order book the order is placed in 
///	ex_type: ExchangeType{LimitOrder, FlowOrder} -> identifies which exchange this order is compatible with
/// p_low: f64 -> trader's minimum willingness to buy or sell (FlowOrder)
/// p_high: f64 -> trader's maximum willingness to buy or sell (FlowOrder)
/// price: f64 -> trader's willing ness to buy or sell (LimitOrder)
/// quantity: f64 -> amount of shares to buy/sell
/// gas: f64 -> the gas/tx fee to post an order
pub struct Order {
	pub trader_id: String,
	pub order_id: u64,		
	pub order_type: OrderType,	
	pub trade_type: TradeType,  
	pub ex_type: ExchangeType,
	pub p_low: f64,				
	pub p_high: f64,
	pub price: f64,
	pub quantity: f64,			
	pub gas: f64,
}

impl Order {
    pub fn new(t_id: String, o_t: OrderType, t_t: TradeType, 
    		   e_t: ExchangeType, p_l: f64, p_h: f64, p: f64, q: f64, gas: f64) -> Order
    {
    	Order {
    		trader_id: t_id,	
    		order_id: gen_order_id(),	
			order_type: o_t,	
			trade_type: t_t,  
			ex_type: e_t,
			p_low: p_l,
			p_high: p_h,
			price: p,				
			quantity: q,	
			gas: gas,
    	}
    }

    pub fn describe(&self) {
    	println!("Trader Id: {:?} \n OrderType: {:?}
    		price: {:?}, quantity: {:?}", 
    		self.trader_id, self.order_type,
    		self.price, self.quantity);
    }
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_limit_order() {
		let order = Order::new(
			String::from("trader_id"),
			OrderType::Enter,
			TradeType::Bid,
			ExchangeType::LimitOrder,
			0.0,
			0.0,
			50.0,
			500.0,
			0.05,
		);

		assert_eq!(order.trader_id, "trader_id");
		assert_eq!(order.order_type, OrderType::Enter);
		assert_eq!(order.trade_type, TradeType::Bid);
		assert_eq!(order.ex_type, ExchangeType::LimitOrder);
		assert_eq!(order.price, 50.0);
		assert_eq!(order.quantity, 500.0);
		assert_eq!(order.gas, 0.05);
	}

	#[test]
	fn test_new_flow_order() {
		let order = Order::new(
			String::from("trader_id"),
			OrderType::Enter,
			TradeType::Bid,
			ExchangeType::FlowOrder,
			99.0,
			101.0,
			50.0,
			500.0,
			0.05,
		);

		assert_eq!(order.trader_id, "trader_id");
		assert_eq!(order.order_type, OrderType::Enter);
		assert_eq!(order.trade_type, TradeType::Bid);
		assert_eq!(order.ex_type, ExchangeType::FlowOrder);
		assert_eq!(order.p_low, 99.0);
		assert_eq!(order.p_high, 101.0);
		assert_eq!(order.quantity, 500.0);
		assert_eq!(order.gas, 0.05);
	}
}

























