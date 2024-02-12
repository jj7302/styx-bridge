#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, token, vec, Address, BytesN, Env, IntoVal};
use token::Client as TokenClient;
use token::StellarAssetClient as TokenAdminClient;

fn create_claimable_balance_contract<'a>(e: &Env, contract_id: &Address) -> ClaimableBalanceContractClient<'a> {
    ClaimableBalanceContractClient::new(e, contract_id)
}

fn create_token_contract<'a>(e: &Env, admin: &Address) -> (TokenClient<'a>, TokenAdminClient<'a>) {
    let contract_address = e.register_stellar_asset_contract(admin.clone());
    (
        TokenClient::new(e, &contract_address),
        TokenAdminClient::new(e, &contract_address),
    )
}

struct ClaimableBalanceTest<'a> {
    env: Env,
    token: TokenClient<'a>,
    token_admin_client: TokenAdminClient<'a>,
    destination: BytesN<32>,
    contract: ClaimableBalanceContractClient<'a>,
    contract_id: Address
}

impl<'a> ClaimableBalanceTest<'a> {
    fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);

        let (token, token_admin_client) = create_token_contract(&env, &token_admin);

        let destination = BytesN::from_array(&env, &[0u8; 32]);

        let contract_id = env.register_contract(None, ClaimableBalanceContract {});

        let contract = create_claimable_balance_contract(&env, &contract_id);

        ClaimableBalanceTest {
            env,
            token,
            token_admin_client,
            destination,
            contract,
            contract_id
        }
      
    }
}

#[test]
fn test_deposit_and_increment_nonce() {
    let test = ClaimableBalanceTest::setup();
    let initial_nonce = test.contract.get_current_value();

    let sender_address = Address::generate(&test.env);
    test.token_admin_client.mint(&sender_address, &1000);
    // Perform a deposit which should increment the nonce
    let deposit_nonce = test.contract.deposit(
        &sender_address,
        &test.token.address,
        &100, // deposit amount
        &test.destination,
    );

    assert_eq!(
        deposit_nonce,
        initial_nonce + 1,
        "Nonce should be incremented by 1"
    );
    assert_eq!(test.token.balance(&sender_address), 900);

    // Verify that the nonce has been updated correctly
    let updated_nonce = test.contract.get_current_value();
    assert_eq!(
        updated_nonce,
        initial_nonce + 1,
        "Updated nonce should match expected value"
    );

    
    let example_claimable_balance = ClaimableBalance {
      token : test.token.address,
      amount: 100,
      sender: sender_address.clone(),
      destination: test.destination,
      last_event_nonce: updated_nonce
    };

    let events_len = test.env.events().all().len();

    assert_eq!(
      test.env.events().all().slice(events_len - 1..events_len),
      vec![
          &test.env,
          (
              test.contract_id.clone(),
              (symbol_short!("Deposit"),).into_val(&test.env),
              example_claimable_balance.into_val(&test.env)
        ),
      ]
  );
}

#[test]
#[should_panic(expected = "balance is not sufficient to spend")]
fn insufficient_funds_test() {
  let test = ClaimableBalanceTest::setup();
  let sender_address = Address::generate(&test.env);

  // Perform a deposit which should increment the nonce
  let deposit_nonce = test.contract.deposit(
      &sender_address,
      &test.token.address,
      &10000, // deposit amount
      &test.destination,
  );
}
// Additional tests could include:
// - Testing edge cases, like depositing with a zero amount.
// - Verifying event emission upon successful deposit.
// - Testing the TTL extension logic, if applicable.
