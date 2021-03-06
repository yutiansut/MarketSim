use crate::simulation::simulation_config::{Distributions, Constants};
use crate::simulation::simulation_history::{PriorData, LikelihoodStats, UpdateReason};
use crate::exchange::exchange_logic::TradeResults;
use crate::exchange::MarketType;
use crate::order::order::{Order};
use crate::players::{Player, TraderT};
use crate::players::investor::Investor;
use crate::players::maker::{Maker, MakerT};
use crate::players::miner::Miner;
use crate::log_player_data;

use std::collections::HashMap;
use std::sync::Mutex;
use rand::{thread_rng};
use rand::seq::SliceRandom;


use log::{log, Level};



/// The struct for keeping track of active players and their balances and inventories
/// ClearingHouse is a HashMap indexed by each player's trader_id
pub struct ClearingHouse {
	pub players: Mutex<HashMap<String, Box<dyn Player + Send>>>,
	pub gas_fees: Mutex<Vec<f64>>,
	pub total_tax: Mutex<f64>,
	pub maker_profits: Mutex<Vec<f64>>,
}



impl ClearingHouse {
	/// Create a new ClearingHouse to store player data
	pub fn new() -> Self {
		ClearingHouse {
			players: Mutex::new(HashMap::new()),
			gas_fees: Mutex::new(Vec::<f64>::new()),	
			total_tax: Mutex::new(0.0),
			maker_profits: Mutex::new(vec![0.0, 0.0, 0.0]),
		}
	}


	/// Register an investor to the ClearingHouse Hashmap
	pub fn reg_investor(&self, inv: Investor) {
		let mut players = self.players.lock().unwrap();
		players.entry(inv.trader_id.clone()).or_insert(Box::new(inv));
	}

	/// Register a vector of investors to the ClearingHouse Hashmap
	pub fn reg_n_investors(&self, investors: Vec<Investor>) {
		let mut players = self.players.lock().unwrap();
		for i in investors {
			players.entry(i.trader_id.clone()).or_insert(Box::new(i));
		}
	}

	/// Register a maker to the ClearingHouse Hashmap
	pub fn reg_maker(&self, maker: Maker) {
		let mut players = self.players.lock().unwrap();
		players.entry(maker.trader_id.clone()).or_insert(Box::new(maker));
	}

	/// Register a vector of makers to the ClearingHouse Hashmap
	pub fn reg_n_makers(&self, makers: Vec<Maker>) {
		let mut players = self.players.lock().unwrap();
		for m in makers {
			players.entry(m.trader_id.clone()).or_insert(Box::new(m));
		}
	}

	/// Register a miner to the ClearingHouse Hashmap
	pub fn reg_miner(&self, miner: Miner) {
		let mut players = self.players.lock().unwrap();
		players.entry(miner.trader_id.clone()).or_insert(Box::new(miner));
	}


	// Gets a reference to the player by popping it from the hashmap
	pub fn get_player(&self, id: String) -> Option<Box<dyn Player>> {
		let mut players = self.players.lock().unwrap();
		match players.remove(&id) {
			Some(player) => Some(player),
			None => None,
		}
	}

	// Gets the maker and generates a pair of orders based on supplied parameters 
	pub fn maker_new_orders(&self, id: String, data: &PriorData, inference: &LikelihoodStats, dists: &Distributions, consts: &Constants) -> Option<(Order, Order)>{
		let players = self.players.lock().unwrap();
		match players.get(&id) {
			Some(player) => {
				if let Some(maker) = player.as_any().downcast_ref::<Maker>() {
					// Was able to find the maker in the clearing house and cast Player object to Maker
					let orders = maker.new_orders(data, inference, dists, consts);
					return orders
				} else {
					// Couldn't downcast to maker
					println!("Couldn't downcast to maker: {}", id);
					return None;
				}
			},
			None => {
				println!("Couldn't get maker: {}", id);
				return None;
			}
		} 
	}

	// Gets the maker and cancels all of their enter orders in the clearing house
	// returns a vector of all of their orders with the update OrderType = Cancel
	// to be submitted to the mempool -> order books
	pub fn cancel_all_orders(&self, id: String) -> Result<Vec<Order>, ()> {
		let mut players = self.players.lock().unwrap();
		let mut orders = Vec::new();
		match players.get_mut(&id) {
			Some(player) => {
				// Get the order ids of player's current enter orders
				let order_ids = player.get_enter_order_ids();
				for o_id in order_ids {
					// Check if the player has already sent a cancel for this order id to the mempool
					if player.check_double_cancel(o_id) {continue;}
					// Generate a cancel order for that enter order
					if let Ok(cancel_order) = player.gen_cancel_order(o_id) {
						// Record this in the player's history of sent orders to avoid double cancels.
						player.add_to_sent(o_id, cancel_order.order_type.clone());
						orders.push(cancel_order);
					};
				} 
				Ok(orders)
			},
			None => {
				println!("Couldn't get player to cancel orders: {}", id);
				return Err(());
			}
		}
	}

	pub fn get_player_order_count(&self, id: &String) -> Result<usize, ()> {
		let players = self.players.lock().unwrap();
		match players.get(id) {
			Some(p) => Ok(p.num_orders()),
			None => Err(()),
		}
	}

	pub fn get_type(&self, id: &String) -> Result<TraderT, ()> {
		let players = self.players.lock().unwrap();
		match players.get(id) {
			Some(p) => Ok(p.get_player_type()),
			None => Err(()),
		}
	}

	// Shuffles through the players matching the player_type and returns their id
	pub fn get_rand_player_id(&self, player_type: TraderT) -> Option<String> {
		let players = self.players.lock().unwrap();
		let mut rng = thread_rng();
		let mut _filtered: Vec<(_, _)> = players.iter().filter(|(_k, v)| v.get_player_type() == player_type).collect();
		if let Some((id, _value)) = _filtered.choose(&mut rng) {
			return Some(id.to_string());
		} else {
			return None
		}
	}

	// Returns all player id's for the specified player_type
	pub fn get_filtered_ids(&self, player_type: TraderT) -> Vec<String> {
		let mut ids = Vec::new();
		let players = self.players.lock().unwrap();
		let mut rng = thread_rng();
		let filtered: Vec<(_, _)> = players.iter().filter(|(_k, v)| v.get_player_type() == player_type).collect();
		for (id, _o) in filtered {
			ids.push(id.clone());
		}
		ids.shuffle(&mut rng);
		ids
	}


	/// Adds to the player's balance and returns their updated balance
	pub fn update_player_bal(&self, id: String, bal_to_add: f64) -> Option<f64> {
		let mut players = self.players.lock().unwrap();
		match players.get_mut(&id) {
			Some(player) => { 
				player.update_bal(bal_to_add);
				Some(player.get_bal())
			}
			None => None,
		}
	}


	/// Adds to the player's inventory and returns their updated inventory
	pub fn update_player_inv(&self, id: String, inv_to_add: f64) -> Option<f64> {
		let mut players = self.players.lock().unwrap();
		match players.get_mut(&id) {
			Some(player) => { 
				player.update_inv(inv_to_add);
				Some(player.get_inv())
			}
			None => None,
		}
	}


	/// Updates both a single player's balance and inventory
	/// Returns tuple Option<(updated_bal: f64, updated_inv: f64)>
	pub fn update_player(&self, id: String, bal_to_add: f64, inv_to_add: f64, reason: UpdateReason) -> Option<(f64, f64)>{
		let mut players = self.players.lock().unwrap();
		match players.get_mut(&id) {
			Some(player) => { 
				player.update_inv(inv_to_add);
				player.update_bal(bal_to_add);
				log_player_data!(player.log_to_csv(reason));

				// Track the updates to specific maker types
				if player.get_player_type() == TraderT::Maker {
					if let Some(maker) = player.as_any().downcast_ref::<Maker>() {
						match maker.maker_type {
							MakerT::Aggressive => {
								let mut maker_profits = self.maker_profits.lock().unwrap();
								maker_profits[MakerT::Aggressive as usize] += bal_to_add;
							},
							MakerT::RiskAverse => {
								let mut maker_profits = self.maker_profits.lock().unwrap();
								maker_profits[MakerT::RiskAverse as usize] += bal_to_add;
							},
							MakerT::Random => {
								let mut maker_profits = self.maker_profits.lock().unwrap();
								maker_profits[MakerT::Random as usize] += bal_to_add;
							},
						}
					}
				}
				Some((player.get_bal(), player.get_inv()))
			}
			None => None,
		}
	}	

	// Get count of each type of maker (aggressive, riskaverse, random)
	pub fn get_maker_counts(&self) -> (i64, i64, i64) {
		let players = self.players.lock().unwrap();
		let mut num_agg = 0;
		let mut num_riska = 0;
		let mut num_rand = 0;
		for (_k, player) in players.iter() {
			if player.get_player_type() == TraderT::Maker {
				if let Some(maker) = player.as_any().downcast_ref::<Maker>() {
					match maker.maker_type {
						MakerT::Aggressive => {
							num_agg += 1;
						},
						MakerT::RiskAverse => {
							num_riska += 1;
						},
						MakerT::Random => {
							num_rand += 1;
						},
					}
				}
			}
		}
		(num_agg, num_riska, num_rand)
	}

	pub fn get_bal_inv(&self, id: String) -> Option<(f64, f64)> {
		let players = self.players.lock().unwrap();
		match players.get(&id) {
			Some(player) => { 
				Some((player.get_bal(), player.get_inv()))
			}
			None => None,
		}
	}

	/// Gets the TradeResults from an auction and updates each player
	pub fn update_house(&self, results: TradeResults) {
		match results.auction_type {
			MarketType::CDA => self.cda_cross_update(results),
			MarketType::FBA => self.fba_batch_update(results),
			MarketType::KLF => self.flow_batch_update(results),
		}
	}

	/// Consumes the trade results from CDA limit order cross to update each player's state
	pub fn cda_cross_update(&self, results: TradeResults) {
		match results.cross_results {
			None => return,
			Some(player_updates) => {
				for pu in player_updates {
					if pu.cancel == true {
						// Cancel the player's order in the clearing house
						match self.cancel_player_order(pu.payer_id, pu.payer_order_id) {
							Ok(()) => {},
							Err(e) => println!("cda_cross_update: {:?}, {}", e, pu.payer_order_id),
						}
						continue;
					}

					// Update bidder: -bal, +inv
					let bidder_id = pu.payer_id;
					let volume = pu.volume;
					if volume == 0.0 {
						// no need to update players if no volume is to be traded
						continue;
					}
					let payment = pu.price * volume;
					if let Some((new_bal, new_inv)) = self.update_player(bidder_id.clone(), -payment, volume, UpdateReason::Transact) {
						println!("Updated {}. bal=>{}, inv=>{}", bidder_id.clone(), new_bal, new_inv);
					} else {
						self.report_player(bidder_id.clone());
						panic!("failed to update {}'s balance/inventory", bidder_id);
					}

					// NOTE: in CDA, the order's volume in orderbook is implicitly modified during crossing
					self.update_player_order_vol(bidder_id.clone(), pu.payer_order_id, -volume).expect("Failed to update");

					// Update asker: +bal, -inv
					let asker_id = pu.vol_filler_id;
					if let Some((new_bal, new_inv)) = self.update_player(asker_id.clone(), payment, -volume, UpdateReason::Transact) {
							println!("Updated {}. bal=>{}, inv=>{}", asker_id.clone(), new_bal, new_inv);
					} else {
						self.report_player(asker_id.clone());
						panic!("failed to update {}'s balance/inventory", asker_id);
					}

					// NOTE: in CDA, the order's volume in orderbook is implicitly modified during crossing
					self.update_player_order_vol(asker_id.clone(), pu.vol_filler_order_id, -volume).expect("Failed to update");
				}
			}
		}
	}

	/// Consumes the trade results to update each player's state
	pub fn fba_batch_update(&self, results: TradeResults) {
		match results.cross_results {
			None => return,
			Some(player_updates) => {
				for pu in player_updates {
					if pu.cancel == true {
						// Cancel the player's order in the clearing house
						match self.cancel_player_order(pu.payer_id, pu.payer_order_id) {
							Ok(()) => {},
							Err(e) => println!("fba_batch_update: {:?}, {}", e, pu.payer_order_id),
						}
						continue;
					}
					// Update bidder: -bal, +inv
					let bidder_id = pu.payer_id;
					let volume = pu.volume;
					if volume == 0.0 {
						// no need to update players if no volume is to be traded
						continue;
					}
					let payment = pu.price * volume;
					if let Some((new_bal, new_inv)) = self.update_player(bidder_id.clone(), -payment, volume, UpdateReason::Transact) {
						println!("Updated {}. bal=>{}, inv=>{}", bidder_id.clone(), new_bal, new_inv);
					} else {
						panic!("failed to update {}'s balance/inventory", bidder_id);
					}

					// Subtract interest from the bidder's order in the clearing house
					self.update_player_order_vol(bidder_id.clone(), pu.payer_order_id, -volume).expect("Failed to update");

					// Update asker: +bal, -inv
					let asker_id = pu.vol_filler_id;
					if let Some((new_bal, new_inv)) = self.update_player(asker_id.clone(), payment, -volume, UpdateReason::Transact) {
							println!("Updated {}. bal=>{}, inv=>{}", asker_id.clone(), new_bal, new_inv);
					} else {
						panic!("failed to update {}'s balance/inventory", bidder_id);
					}

					// Subtract interest from the asker's order
					self.update_player_order_vol(asker_id.clone(), pu.vol_filler_order_id, -volume).expect("Failed to update");
				}
			}
		}
	}

	/// Given the clearing price of the last batch, updates every involved player's state
	// For every order that was in the order book at auction time, 
	// Calculate player.demand(price) or player.supply(price)
	pub fn flow_batch_update(&self, results: TradeResults) {
		match results.uniform_price {
			None => return,
			Some(_clearing_price) => {
				if let Some(player_updates) = results.cross_results {
					let id_check = format!("N/A");
					for pu in player_updates {
						if pu.cancel == true {
						// Cancel the player's order in the clearing house
						match self.cancel_player_order(pu.payer_id, pu.payer_order_id) {
							Ok(()) => {},
							Err(e) => println!("flow_batch_update: {:?}, {}", e, pu.payer_order_id),
						}
						continue;
					}
						let volume = pu.volume;
						let payment = pu.price * volume;

						// This was an ask order, update accordingly
						if pu.payer_id == id_check {
							// Update asker: +bal, -inv
							let asker_id = pu.vol_filler_id;
							if let Some((_new_bal, _new_inv)) = self.update_player(asker_id.clone(), payment, -volume, UpdateReason::Transact) {
								// println!("Updated {}. bal=>{}, inv=>{}", asker_id.clone(), _new_bal, _new_inv);
							}
							// Subtract vol from the trader's order
							self.update_player_order_vol(asker_id.clone(), pu.vol_filler_order_id, -volume).expect("Failed to update");
						} 
						// This was a bid order, update accordingly
						else {
							// Update bidder: -bal, +inv
							let bidder_id = pu.payer_id;
							
							if let Some((_new_bal, _new_inv)) = self.update_player(bidder_id.clone(), -payment, volume, UpdateReason::Transact) {
								// println!("Updated {}. bal=>{}, inv=>{}", bidder_id.clone(), _new_bal, _new_inv);
							}

							// Subtract vol from the trader's order
							self.update_player_order_vol(bidder_id.clone(), pu.payer_order_id, -volume).expect("Failed to update");
						}
					}
						
				} else {
					// No cross results, exit
					return;
				}
			}
		}
	}

	
	/// Add a new order to the HashMap indexed by the player's id
	pub fn new_order(&self, order: Order) -> Result<(), &'static str> {
		let mut players = self.players.lock().unwrap();
		// Find the player by trader id and add their order
		match players.get_mut(&order.trader_id) {
			Some(player) => { 
				player.add_order(order);
				Ok(())
			}
			None => Err("Couldn't find trader to add order")
		}
	}

	/// Add a vector of new orders to the HashMap. This is preferable to new_order
	/// as the mutex lock only has to be acquired once.
	pub fn new_orders(&self, orders: Vec<Order>) -> Result<(), &'static str> {
		let mut players = self.players.lock().unwrap();
		for order in orders {
			match players.get_mut(&order.trader_id) {
				Some(player) => { 
					player.add_order(order);
				}
				None => return Err("Couldn't find trader to add order"),
			}
		}
		Ok(())
	}

	/// Replaces a trader's order in the HashMap with the supplied 'order' 
	pub fn update_player_order(&self, order: Order) -> Result<(), &'static str> {
		match self.cancel_player_order(order.trader_id.clone(), order.order_id) {
			Ok(()) => {
				self.new_order(order)
			},
			// Couldn't find order to cancel but still enter order
			Err(_e) => {
				self.new_order(order)
			}
		}
	}


	/// Adds volume to a trader's order to reflect changes in the order book. 
	/// If they updated volume <=0, the order is dropped from the player's list
	pub fn update_player_order_vol(&self, trader_id: String, order_id: u64, vol_to_add: f64) -> Result<(), &'static str> {
		// println!("Updating {}'s order {} volume by {}", trader_id, order_id, vol_to_add);
		// self.report_player(trader_id.clone());
		let mut players = self.players.lock().unwrap();
		if let Some(player) = players.get_mut(&trader_id) {
			player.update_order_vol(order_id, vol_to_add)
		} else {
			return Err("Couldn't find trader to add order");
		}
	}

	/// Cancel's a trader's order in the HashMap with the supplied 'order'
	pub fn cancel_player_order(&self, trader_id: String, order_id: u64) -> Result<(), &str> {
		let mut players = self.players.lock().unwrap();
		if let Some(player) = players.get_mut(&trader_id) {
			let res = player.cancel_order(order_id);
				match res {
					Ok(_) => return Ok(()),
					Err(e) => return Err(e),
				}
		} else {
			return Err("Couldn't find trader to cancel order");
		}
	}

	/// Removes the player from the ClearingHouse HashMap
	pub fn del_player(&self, trader_id: String) -> Option<()>{
		match self.players.lock().unwrap().remove(&trader_id) {
			Some(_p) => Some(()),
			None => None
		}
	}

	pub fn report_player(&self, trader_id: String) {
		let players = self.players.lock().unwrap();
		if let Some(p) = players.get(&trader_id) {
			println!("id={}, bal={}, inv={}, orders={:?}", p.get_id(), p.get_bal(), p.get_inv(), p.copy_orders());
		} else {
			println!("Couldn't report on {}", trader_id);
		}
	}

	/// Utility function for seeing how many Trader's are currently active
	pub fn num_players(&self) -> usize {
		self.players.lock().unwrap().len()
	}

	/// Utility function for seeing how many orders are currently active (not nec in order book)
	pub fn orders_in_house(&self) -> usize {
		let players = self.players.lock().unwrap();
		let mut sum = 0;
		for (_id, p) in players.iter() {
			sum += p.num_orders();
		}
		sum
	}

	// Updates the cummulative gas fees from the simulation, as well subtracts the
	// gas fees from each player's balance
	pub fn apply_gas_fees(&self, to_change: Vec<(String, f64)>, total: f64) {
		{
			// Add the gas fees for this batch
			self.gas_fees.lock().expect("apply_gas_fees").push(total);
		}

		let mut players = self.players.lock().unwrap();
		for c in to_change {
			// Search for c.0 = trader_id, subtract c.1 = gas fee
			match players.get_mut(&c.0) {
				Some(player) => { 
					let _bef = player.get_bal();
					player.update_bal(-c.1);
					// println!("{}, gas:{} before: {}, after: {}\n", c.0, c.1, _bef, player.get_bal());
					log_player_data!(player.log_to_csv(UpdateReason::Gas));
				}
				None => {},
			}
		}
	}

	pub fn add_tax(&self, tax_amt: f64) {
		let mut total = self.total_tax.lock().unwrap();
		*total += tax_amt;
	}


	// Mulitplies all maker's current inv by the tax and subtracts that amount from their player bal
	pub fn tax_makers(&self, tax: f64) {
		let ids = self.get_filtered_ids(TraderT::Maker);
		let mut players = self.players.lock().unwrap();
		for id in ids {
			match players.get_mut(&id) {
				Some(player) => { 
					let _bef = player.get_bal();
					let tax_amt = (player.get_inv() * tax).abs();
					player.update_bal(-tax_amt);
					self.add_tax(tax_amt);
					// println!("{} tax:{}, before: {}, after: {}\n", id, tax_amt, _bef, player.get_bal());
					log_player_data!(player.log_to_csv(UpdateReason::Tax));
				}
				None => {},
			}
		}
	}


	// log all of the player states
	pub fn log_all_players(&self, reason: UpdateReason) {
		let players = self.players.lock().unwrap();
		for (_id, player) in players.iter() {
    		log_player_data!(player.log_to_csv(reason));
		}
	}


	// if player has negative inventory and so will buy at fund_val
	//      cur_inv is negative so cur_inv * fund_val < 0, which subtracts from player bal
	// if player has positive inventory and so will sell at fund_val
	//      cur_inv is positive so cur_inv * fundval > 0 which adds to their player bal
	pub fn liquidate(&self, fund_val: f64) {
		let mut players = self.players.lock().unwrap();
		for (_id, player) in players.iter_mut() {
			let cur_inv = player.get_inv();
			let update_amount = cur_inv * fund_val;
				
			player.update_bal(update_amount);
			player.update_inv(-cur_inv);

			// Update the balances of the specific maker types
			if player.get_player_type() == TraderT::Maker {
				if let Some(maker) = player.as_any().downcast_ref::<Maker>() {
					match maker.maker_type {
						MakerT::Aggressive => {
							let mut maker_profits = self.maker_profits.lock().unwrap();
							maker_profits[MakerT::Aggressive as usize] += update_amount;
						},
						MakerT::RiskAverse => {
							let mut maker_profits = self.maker_profits.lock().unwrap();
							maker_profits[MakerT::RiskAverse as usize] += update_amount;
						},
						MakerT::Random => {
							let mut maker_profits = self.maker_profits.lock().unwrap();
							maker_profits[MakerT::Random as usize] += update_amount;
						},
					}
				}
			}
    		log_player_data!(player.log_to_csv(UpdateReason::Liquify));
		}
		
	}
}



#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;
	use crate::players::maker::{Maker, MakerT};

	#[test]
	fn test_ch() {
		let mut i = Investor::new(format!("{:?}", "BillyBob"));
		i.update_bal(55.0);
		i.update_inv(100.0);

		let mut mkr = Maker::new(format!("{:?}", "NillyNob"), MakerT::Aggressive);
		mkr.update_bal(55.0);
		mkr.update_inv(100.0);

		let min = Miner::new(format!("{:?}", "SquillyFob"));

		let ch = Arc::new(ClearingHouse::new());

		// Test adding new players
		ch.reg_investor(i);
		ch.reg_maker(mkr);
		ch.reg_miner(min);
		assert_eq!(ch.num_players(), 3);

		// Test updating a player's balance
		if let Some(bal) = ch.update_player_bal(format!("{:?}", "BillyBob"), 40.0) {
			assert_eq!(bal, 95.0);
		} else {
			panic!("AHHH failed to update player balance");
		}

		// Test updating a player's balance
		if let Some(inv) = ch.update_player_inv(format!("{:?}", "NillyNob"), -40.0) {
			assert_eq!(inv, 60.0);
		} else {
			panic!("AHHH failed to update player inventory");
		}

		// Test updating both
		if let Some((bal, inv)) = ch.update_player(format!("{:?}", "SquillyFob"), -40.0, 20.0, UpdateReason::Transact) {
			assert_eq!(inv, 20.0);
			assert_eq!(bal, -40.0);
		} else {
			panic!("AHHH failed to update player");
		}

		if let Some(_) = ch.del_player(format!("{:?}", "SquillyFob")) {
			assert_eq!(ch.num_players(), 2);
		} else {
			panic!("AHHH failed to delete player");
		}
	}

	
}







