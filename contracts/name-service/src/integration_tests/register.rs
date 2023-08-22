use cosmwasm_std::Addr;
use nym_crypto::asymmetric::identity;
use nym_name_service_common::{
    error::NameServiceError, response::PagedNamesListResponse, NameDetails, NymName, RegisteredName,
};
use rstest::rstest;

use crate::{
    constants::NAME_DEFAULT_RETRIEVAL_LIMIT,
    test_helpers::{fixture::new_name, helpers::nyms},
};

use super::test_setup::TestSetup;

#[rstest::fixture]
fn setup() -> TestSetup {
    TestSetup::new()
}

#[rstest]
fn basic_register(mut setup: TestSetup) {
    assert_eq!(
        setup.query_all(),
        PagedNamesListResponse {
            names: vec![],
            per_page: NAME_DEFAULT_RETRIEVAL_LIMIT as usize,
            start_next_after: None,
        }
    );

    // Register a first name
    let owner = Addr::unchecked("owner");
    let name = NymName::new("steves-server").unwrap();
    let (nym_address, id_keypair) = setup.new_nym_address();
    assert_eq!(setup.contract_balance(), nyms(0));
    assert_eq!(setup.balance(&owner), nyms(250));
    assert_eq!(setup.query_signing_nonce(owner.to_string()), 0);

    let reg_name = setup.new_name_from_address(&name, &nym_address, id_keypair);
    let payload = setup.payload_to_sign(&owner, &nyms(100), &reg_name.name);
    let reg_name = reg_name.sign(payload);
    setup.register(&reg_name, &owner);

    // Confirm that the client id in the name matches the identity key.
    let address_client_id = reg_name.name.address.client_id();
    let identity_key = reg_name.keys.public_key().to_base58_string();
    assert_eq!(address_client_id, &identity_key);

    // Deposit is deposited to contract and deducted from owners's balance
    assert_eq!(setup.contract_balance(), nyms(100));
    assert_eq!(setup.balance(&owner), nyms(150));

    // The signing nonce has been incremented
    assert_eq!(setup.query_signing_nonce(owner.to_string()), 1);

    // We can query the full name list
    assert_eq!(
        setup.query_all(),
        PagedNamesListResponse {
            names: vec![RegisteredName {
                id: 1,
                name: NameDetails {
                    name: name.clone(),
                    address: nym_address.clone(),
                    identity_key: reg_name.identity_key().to_string(),
                },
                owner: owner.clone(),
                block_height: 12345,
                deposit: nyms(100),
            }],
            per_page: NAME_DEFAULT_RETRIEVAL_LIMIT as usize,
            start_next_after: Some(1),
        }
    );

    // ... and we can query by id
    assert_eq!(
        setup.query_id(1),
        RegisteredName {
            id: 1,
            name: reg_name.details().clone(),
            owner: owner.clone(),
            block_height: 12345,
            deposit: nyms(100),
        }
    );

    // Register a second name
    let owner2 = Addr::unchecked("owner2");
    let name2 = NymName::new("another_server").unwrap();
    let reg_name2 = setup.new_signed_name(&name2, &owner2, &nyms(100));
    let nym_address2 = reg_name2.address().clone();
    setup.register(&reg_name2, &owner2);

    assert_eq!(setup.contract_balance(), nyms(200));
    assert_eq!(
        setup.query_all(),
        PagedNamesListResponse {
            names: vec![
                new_name(1, &name, &nym_address, &owner, reg_name.identity_key()),
                new_name(2, &name2, &nym_address2, &owner2, reg_name2.identity_key()),
            ],
            per_page: NAME_DEFAULT_RETRIEVAL_LIMIT as usize,
            start_next_after: Some(2),
        }
    );
}

#[rstest]
fn register_fails_when_owner_mismatch(mut setup: TestSetup) {
    let owner = Addr::unchecked("owner");
    let name = NymName::new("steves-server").unwrap();
    let reg_name = setup.new_signed_name(&name, &owner, &nyms(100));
    let res = setup
        .try_register(&reg_name, &Addr::unchecked("owner2"))
        .unwrap_err();
    assert_eq!(
        res.downcast::<NameServiceError>().unwrap(),
        NameServiceError::InvalidEd25519Signature
    );
}

#[rstest]
fn signing_nonce_is_increased_when_registering(mut setup: TestSetup) {
    let owner1 = Addr::unchecked("owner1");
    let owner2 = Addr::unchecked("owner2");

    assert_eq!(setup.query_signing_nonce(owner1.to_string()), 0);
    assert_eq!(setup.query_signing_nonce(owner2.to_string()), 0);

    setup.sign_and_register(&NymName::new("myname1").unwrap(), &owner1, &nyms(100));

    assert_eq!(setup.query_signing_nonce(owner1.to_string()), 1);
    assert_eq!(setup.query_signing_nonce(owner2.to_string()), 0);

    setup.sign_and_register(&NymName::new("myname2").unwrap(), &owner2, &nyms(100));

    assert_eq!(setup.query_signing_nonce(owner1.to_string()), 1);
    assert_eq!(setup.query_signing_nonce(owner2.to_string()), 1);

    setup.sign_and_register(&NymName::new("myname3").unwrap(), &owner2, &nyms(100));

    assert_eq!(setup.query_signing_nonce(owner1.to_string()), 1);
    assert_eq!(setup.query_signing_nonce(owner2.to_string()), 2);
}

#[rstest]
fn creating_two_names_in_a_row_without_announcing_fails(mut setup: TestSetup) {
    let owner = Addr::unchecked("wealthy_owner_1");
    let name1 = NymName::new("steves-server1").unwrap();
    let name2 = NymName::new("steves-server2").unwrap();
    let deposit = nyms(100);

    let s1 = setup.new_signed_name(&name1, &owner, &deposit);

    // This second name will be signed with the same nonce
    let s2 = setup.new_signed_name(&name2, &owner, &deposit);

    // Announce the first service works, and this increments the nonce
    setup.register(&s1, &owner);

    // Now the nonce has been incremented, and the signature will not match
    let resp: NameServiceError = setup
        .try_register(&s2, &owner)
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(resp, NameServiceError::InvalidEd25519Signature,);
}

#[rstest]
fn cant_register_a_name_without_funds(mut setup: TestSetup) {
    assert_eq!(setup.contract_balance(), nyms(0));
    assert_eq!(setup.balance("owner"), nyms(250));
    let name1 = setup.new_signed_name(
        &NymName::new("my_name").unwrap(),
        &Addr::unchecked("owner"),
        &nyms(100),
    );
    setup.register(&name1, &Addr::unchecked("owner"));
    assert_eq!(setup.contract_balance(), nyms(100));
    assert_eq!(setup.balance("owner"), nyms(150));

    let name2 = setup.new_signed_name(
        &NymName::new("my_name2").unwrap(),
        &Addr::unchecked("owner"),
        &nyms(100),
    );
    setup.register(&name2, &Addr::unchecked("owner"));
    assert_eq!(setup.contract_balance(), nyms(200));
    assert_eq!(setup.balance("owner"), nyms(50));
    let name3 = setup.new_signed_name(
        &NymName::new("my_name3").unwrap(),
        &Addr::unchecked("owner"),
        &nyms(100),
    );
    let res = setup
        .try_register(&name3, &Addr::unchecked("owner"))
        .unwrap_err();
    assert_eq!(
        res.downcast::<cosmwasm_std::StdError>().unwrap(),
        cosmwasm_std::StdError::Overflow {
            source: cosmwasm_std::OverflowError::new(
                cosmwasm_std::OverflowOperation::Sub,
                "50",
                "100"
            )
        }
    );
}

#[rstest]
fn cant_register_the_same_name_multiple_times(mut setup: TestSetup) {
    let name1 = setup.new_signed_name(
        &NymName::new("name").unwrap(),
        &Addr::unchecked("owner"),
        &nyms(100),
    );
    setup.register(&name1, &Addr::unchecked("owner"));
    let resp = setup
        .try_register(&name1, &Addr::unchecked("owner"))
        .unwrap_err();

    assert_eq!(
        resp.downcast::<NameServiceError>().unwrap(),
        NameServiceError::NameAlreadyRegistered {
            name: NymName::new("name").unwrap()
        }
    );
}

fn clone_keys(keys: &identity::KeyPair) -> identity::KeyPair {
    let priv_bytes = keys.private_key().to_bytes();
    let pub_bytes = keys.public_key().to_bytes();
    identity::KeyPair::from_bytes(&priv_bytes, &pub_bytes).unwrap()
}

#[rstest]
fn can_register_multiple_names_for_the_same_nym_address(mut setup: TestSetup) {
    let name1 = NymName::new("name1").unwrap();
    let name2 = NymName::new("name2").unwrap();
    // let address = Address::new("nym.add@ress").unwrap();
    let (address, id_keys) = setup.new_nym_address();
    let owner = Addr::unchecked("owner");

    // We duplicate the keypair here to ensure that the same keypair is used for both names.
    // The private key lacks a clone method, for good reason, so we have to serialize and
    // deserialize it.
    let id_keys2 = clone_keys(&id_keys);
    assert_eq!(id_keys.public_key(), id_keys2.public_key());
    assert_eq!(
        id_keys.private_key().to_base58_string(),
        id_keys2.private_key().to_base58_string()
    );

    // let reg_name1 = setup.new_signed_name(&name1, &owner, &nyms(100));
    let reg_name1 = setup.new_name_from_address(&name1, &address, id_keys);
    let payload = setup.payload_to_sign(&owner, &nyms(100), &reg_name1.name);
    let reg_name1 = reg_name1.sign(payload);
    setup.register(&reg_name1, &owner);

    // let reg_name2 = setup.new_signed_name(&name2, &owner, &nyms(100));
    let reg_name2 = setup.new_name_from_address(&name2, &address, id_keys2);
    let payload = setup.payload_to_sign(&owner, &nyms(100), &reg_name2.name);
    let reg_name2 = reg_name2.sign(payload);
    setup.register(&reg_name2, &owner);

    dbg!(&setup.query_all().names);
    assert_eq!(
        setup.query_all().names,
        vec![
            new_name(1, &name1, &address, &owner, reg_name1.identity_key()),
            new_name(2, &name2, &address, &owner, reg_name2.identity_key()),
        ],
    );
}
