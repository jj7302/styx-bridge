#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Symbol,
};

const NONCE: Symbol = symbol_short!("NONCE");

#[derive(Clone)]
#[derive(Debug)]
#[contracttype]
pub struct ClaimableBalance {
    pub token: Address,
    pub amount: i128,
    pub sender: Address,
    pub destination: BytesN<32>,
    pub last_event_nonce: u32,
}

#[contract]
pub struct ClaimableBalanceContract;

#[contractimpl]
impl ClaimableBalanceContract {
    pub fn deposit(
        env: Env,
        from: Address,
        token: Address,
        amount: i128,
        destination: BytesN<32>,
    ) -> u32 {
        from.require_auth();

        token::Client::new(&env, &token).transfer(&from, &env.current_contract_address(), &amount);

        let mut nonce: u32 = env.storage().instance().get(&NONCE).unwrap_or(0);

        nonce += 1;

        env.storage().instance().set(&NONCE, &nonce);

        let event_data = ClaimableBalance {
            token,
            amount,
            sender: from,
            destination,
            last_event_nonce: nonce,
        };

        env.events().publish(
            (symbol_short!("Deposit"),),
            event_data
        );
        nonce
    }
    pub fn get_current_nonce(env: Env) -> u32 {
        let nonce: u32 = env.storage().instance().get(&NONCE).unwrap_or(0);
        nonce
    }
}

#[cfg(test)]
mod test;
