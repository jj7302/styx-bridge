#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, AuthorizedInvocation, Ledger};
use soroban_sdk::{symbol_short, token, vec, Address, BytesN, Env, IntoVal};
use token::Client as TokenClient;
use token::StellarAssetClient as TokenAdminClient;

fn create_claimable_balance_contract<'a>(e: &Env) -> ClaimableBalanceContractClient<'a> {
    ClaimableBalanceContractClient::new(e, &e.register_contract(None, ClaimableBalanceContract {}))
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
    sender_address: Address,
    destination: BytesN<32>,
    contract: ClaimableBalanceContractClient<'a>,
}

impl<'a> ClaimableBalanceTest<'a> {
    fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        env.ledger().with_mut(|li| {
            li.timestamp = 12345;
        });

        let sender_address = Address::generate(&env);

        let token_admin = Address::generate(&env);

        let (token, token_admin_client) = create_token_contract(&env, &token_admin);
        token_admin_client.mint(&sender_address, &1000);

        let destination = BytesN::from_array(&env, &[0u8; 32]);

        let contract = create_claimable_balance_contract(&env);

        ClaimableBalanceTest {
            env,
            token,
            sender_address,
            destination,
            contract,
        }
    }
}

#[test]
fn test_deposit_and_increment_nonce() {
    let test = ClaimableBalanceTest::setup();
    let initial_nonce = test.contract.get_current_value();

    // Perform a deposit which should increment the nonce
    let deposit_nonce = test.contract.deposit(
        &test.sender_address,
        &test.token.address,
        &100, // deposit amount
        &test.destination,
    );

    assert_eq!(
        deposit_nonce,
        initial_nonce + 1,
        "Nonce should be incremented by 1"
    );
    assert_eq!(test.token.balance(&test.sender_address), 900);

    // Verify that the nonce has been updated correctly
    let updated_nonce = test.contract.get_current_value();
    assert_eq!(
        updated_nonce,
        initial_nonce + 1,
        "Updated nonce should match expected value"
    );
}

#[test]
#[should_panic(expected = "sender not authorized")]
fn test_deposit_without_authorization() {
    let test = ClaimableBalanceTest::setup();

    // Attempt to deposit without proper authorization
    test.contract.deposit(
        &Address::generate(&test.env), // A different sender, not authorized
        &test.token.address,
        &100,
        &test.destination,
    );
}

// Additional tests could include:
// - Testing edge cases, like depositing with a zero amount.
// - Verifying event emission upon successful deposit.
// - Testing the TTL extension logic, if applicable.
