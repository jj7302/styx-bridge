#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, xdr::ToXdr, Address, Bytes, BytesN,
    ConversionError, Env, Symbol, TryFromVal, Val, Vec,
};

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DataKey {
    EventNonce = 0,
    BatchNonce = 1,
    ValsetCheckpoint = 2,
}

impl TryFromVal<Env, DataKey> for Val {
    type Error = ConversionError;

    fn try_from_val(_env: &Env, v: &DataKey) -> Result<Self, Self::Error> {
        Ok((*v as u32).into())
    }
}

const MIN_POWER: u32 = 2863311530; //TODO: find value for power threshold

#[derive(Clone, Debug)]
#[contracttype]
pub struct DepositEventData {
    pub token: Address,
    pub amount: i128,
    pub sender: Address,
    pub destination: BytesN<32>,
    pub last_event_nonce: u32,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ValsetEventData {
    pub newValsetNonce: BytesN<32>,
    pub eventNonce: u32,
    pub rewardAmount: u32,
    pub rewardToken: Address,
    pub validators: Vec<Address>,
    pub powers: Vec<u32>,
}

#[derive(Clone)]
#[contracttype]
pub struct ValsetArgs {
    pub validators: Vec<Address>,
    pub powers: Vec<u32>,
    pub valset_nonce: u32,
    pub reward_amount: u32,
    pub reward_token: Address,
}

#[contracttype]
pub struct Signature {
    pub v: BytesN<32>,
    pub r: BytesN<32>,
    pub s: BytesN<32>,
}

fn make_checkpoint(e: &Env, valset: &ValsetArgs, styx_id: &BytesN<32>) -> BytesN<32> {
    let mut payload = Bytes::new(&e);
    payload.append(&valset.clone().to_xdr(&e)); //TODO: see if this works
    payload.append(&styx_id.clone().to_xdr(&e));
    payload.append(&"valsetargs".to_xdr(&e));
    let checkpoint = e.crypto().keccak256(&payload);
    return checkpoint;
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

        //TODO: see what happens if insufficient funds
        token::Client::new(&env, &token).transfer(&from, &env.current_contract_address(), &amount);

        let mut nonce: u32 = env
            .storage()
            .instance()
            .get(&DataKey::EventNonce)
            .unwrap_or(0);

        nonce += 1;

        env.storage().instance().set(&DataKey::EventNonce, &nonce);

        let event_data = DepositEventData {
            token,
            amount,
            sender: from,
            destination,
            last_event_nonce: nonce,
        };

        env.events()
            .publish((symbol_short!("Deposit"),), event_data);
        env.storage().instance().extend_ttl(100, 100); //TODO: Figure out TTL stuff
        nonce
    }

    pub fn get_current_value(env: Env) -> u32 {
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::EventNonce)
            .unwrap_or(0);
        count
    }

    pub fn recieve_tx(
        env: Env,
        current_valset: ValsetArgs,
        sigs: Vec<Signature>,
        amounts: Vec<u32>,
        destinations: Vec<Address>,
        fees: Vec<u32>,
        batch_nonce: u32,
        token_contract: Address,
        batch_timeout: u32,
    ) {
    }

    pub fn initalize(
        env: Env,
        styx_id: BytesN<32>,
        validators: Vec<Address>, //make sure we actually want these to be of address
        powers: Vec<u32>,
        token: Address
    ) {
        if (validators.is_empty()) {
            panic!("Validator set is empty");
        }
        if (validators.len() != powers.len()) {
            panic!("Validator and power set are not the same length");
        }

        let mut cumulative_power = 0;
        for power in powers.iter() {
            cumulative_power += power;
            if cumulative_power > MIN_POWER {
                break;
            }
        }
        if cumulative_power <= MIN_POWER {
            panic!("InsufficientPower");
        }

        let valset = ValsetArgs {
            validators: validators.clone(),
            powers: powers.clone(),
            valset_nonce: 0,
            reward_amount: 0,
            reward_token: token.clone(),
        };

        let new_checkpoint = make_checkpoint(&env, &valset, &styx_id);

        let nonce: u32 = 0;
        env.storage().instance().set(&DataKey::EventNonce, &nonce);
        env.storage()
            .instance()
            .set(&DataKey::ValsetCheckpoint, &new_checkpoint);

        let event_data = ValsetEventData {
            newValsetNonce: new_checkpoint,
            eventNonce: 0,
            rewardAmount: 0,
            rewardToken: token,
            validators: validators,
            powers: powers,
        };

        env.events()
            .publish((symbol_short!("ValsetUp"),), event_data);
    }
}

#[cfg(test)]
mod test;
