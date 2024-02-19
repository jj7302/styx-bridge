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
    Initialized = 3,
    StyxId = 4,
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
    pub new_valset_nonce: BytesN<32>,
    pub event_nonce: u32,
    pub reward_amount: u32,
    pub reward_token: Address,
    pub validators: Vec<BytesN<32>>,
    pub powers: Vec<u32>,
}

#[derive(Clone)]
#[contracttype]
pub struct ValsetArgs {
    pub validators: Vec<BytesN<32>>, //public keys
    pub powers: Vec<u32>,
    pub valset_nonce: u32,
    pub reward_amount: u32,
    pub reward_token: Address,
}

#[contracttype]
pub struct Signature {
    pub exists: bool,
    pub sig: BytesN<64>,
}

fn make_checkpoint(e: &Env, valset: &ValsetArgs, styx_id: &BytesN<32>) -> BytesN<32> {
    let mut payload = Bytes::new(&e);
    payload.append(&valset.clone().to_xdr(&e)); //TODO: see if this works
    payload.append(&styx_id.clone().to_xdr(&e));
    payload.append(&"valsetargs".to_xdr(&e));
    let checkpoint = e.crypto().keccak256(&payload);
    return checkpoint;
}

fn get_transaction_hash(e: &Env, styx_id: &BytesN<32>, amounts: &Vec<u32>, destinations: &Vec<Address>, fees: &Vec<u32>, batch_nonce: &u32, token_contract: &Address, batch_timeout: &u32) -> BytesN<32> {
    let mut payload = Bytes::new(&e);
    payload.append(&"transactionbatch".to_xdr(&e));
    payload.append(&styx_id.clone().to_xdr(&e));
    payload.append(&amounts.clone().to_xdr(&e));
    payload.append(&destinations.clone().to_xdr(&e));
    payload.append(&fees.clone().to_xdr(&e));
    payload.append(&batch_nonce.clone().to_xdr(&e));
    payload.append(&token_contract.clone().to_xdr(&e));
    payload.append(&batch_timeout.clone().to_xdr(&e));
    let transaction_hash = e.crypto().keccak256(&payload);
}

fn verify_signature(e: &Env, validator: &BytesN<32>, hash: &BytesN<32>, sig: &Signature) {
    let verify_bytes = Bytes::from(hash);
    e.crypto()
        .ed25519_verify(validator, &verify_bytes, &sig.sig)
}

fn check_signatures(e: &Env, valset: &ValsetArgs, sigs: &Vec<Signature>, hash: &BytesN<32>) {
    let mut total_power = 0;
    for i in 0..sigs.len() {
        if sigs.get_unchecked(i).exists {
            verify_signature(
                &e,
                &valset.validators.get_unchecked(i),
                &hash,
                &sigs.get_unchecked(i),
            );
            total_power += valset.powers.get_unchecked(i);
        }
    }
    if total_power <= MIN_POWER {
        panic!("Insufficient power from validators");
    }
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
    ) -> u32 {
        let mut nonce: u32 = env
            .storage()
            .instance()
            .get(&DataKey::BatchNonce)
            .unwrap_or(0);

        if batch_nonce <= nonce {
            panic!(
                "InvalidBatchNonce: newNonce={}, currentNonce={}",
                batch_nonce, nonce
            );
        }

        if batch_nonce > nonce + 1_000_000 {
            panic!(
                "InvalidBatchNonce: newNonce={}, currentNonce={}",
                batch_nonce, nonce
            );
        }

        if env.ledger().sequence() >= batch_timeout {
            panic!("BatchTimedOut");
        }

        if current_valset.validators.len() != current_valset.powers.len()
            || current_valset.validators.len() != sigs.len()
        {
            panic!("MalformedCurrentValidatorSet");
        }

        let mut nonce: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ValsetCheckpoint)
            .unwrap_or(0);

        let mut valset_checkpoint: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::ValsetCheckpoint)
            .unwrap();

        let mut styx_id: BytesN<32> = env.storage().instance().get(&DataKey::StyxId).unwrap(); //figure out how to unwrap/if this is ok

        if make_checkpoint(&env, &current_valset, &styx_id) != valset_checkpoint {
            panic!("IncorrectCheckpoint");
        }

        // Check that the transaction batch is well-formed
        if amounts.len() != destinations.len() || amounts.len() != fees.len() {
            panic!("MalformedBatch");
        }

        let t_hash = get_transaction_hash(&e, &styx_id, &amounts, &destinations, &fees, &batch_nonce, &token_contract, &batch_timeout);
        check_signatures(&env, &current_valset, &sigs, &t_hash);

        env.storage().instance().set(&DataKey::BatchNonce, &batch_nonce);
        batch_nonce
        //TODO: send transactions to wallets and publish events
    }

    pub fn initalize(
        env: Env,
        styx_id: BytesN<32>,
        validators: Vec<BytesN<32>>, //make sure we actually want these to be of address
        powers: Vec<u32>,
        token: Address,
    ) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("Contract has already been initialized");
        } else {
            env.storage().instance().set(&DataKey::Initialized, &true);
        }

        if validators.is_empty() {
            panic!("Validator set is empty");
        }
        if validators.len() != powers.len() {
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

        env.storage().instance().set(&DataKey::StyxId, &styx_id);

        let event_data = ValsetEventData {
            new_valset_nonce: new_checkpoint,
            event_nonce: 0,
            reward_amount: 0,
            reward_token: token,
            validators: validators,
            powers: powers,
        };

        env.events()
            .publish((symbol_short!("ValsetUp"),), event_data);
    }
}

#[cfg(test)]
mod test;
