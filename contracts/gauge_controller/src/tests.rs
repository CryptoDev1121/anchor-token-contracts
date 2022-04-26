use crate::error::ContractError;

use crate::contract::{execute, instantiate, query};
use crate::mock_querier::{mock_dependencies, BASE_TIME};
use crate::utils::{DecimalRoundedCheckedMul, VOTE_DELAY, WEEK};

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeRelativeWeightAtResponse, GaugeRelativeWeightResponse, GaugeWeightAtResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg, TotalWeightAtResponse, TotalWeightResponse,
};

use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, Decimal, Deps, DepsMut, Timestamp, Uint128};
use serde::de::DeserializeOwned;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        anchor_token: "anchor_token".to_string(),
        anchor_voting_escrow: "anchor_voting_escrow".to_string(),
    };
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("owner", config.owner.as_str());
    assert_eq!("anchor_token", config.anchor_token.as_str());
    assert_eq!("anchor_voting_escrow", config.anchor_voting_escrow.as_str());
}

fn run_execute_msg_expect_ok(deps: DepsMut, sender: String, msg: ExecuteMsg, time: u64) {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    if let Err(err) = execute(deps, env, info, msg) {
        panic!("{}", err);
    }
}

fn run_execute_msg_expect_error(
    expect_err: ContractError,
    deps: DepsMut,
    sender: String,
    msg: ExecuteMsg,
    time: u64,
) {
    let info = mock_info(&sender, &[]);
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    match execute(deps, env, info, msg) {
        Err(err) => assert_eq!(expect_err, err),
        _ => panic!("DO NOT ENTER HERE"),
    };
}

fn run_query_msg_expect_ok<T: DeserializeOwned + std::cmp::PartialEq + std::fmt::Debug>(
    expect_response: T,
    deps: Deps,
    msg: QueryMsg,
    time: u64,
) {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    assert_eq!(
        expect_response,
        from_binary(&query(deps, env, msg).unwrap()).unwrap()
    );
}

fn run_query_msg_expect_error(expect_err: ContractError, deps: Deps, msg: QueryMsg, time: u64) {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(time);
    match query(deps, env, msg) {
        Err(err) => assert_eq!(expect_err, err),
        _ => panic!("DO NOT ENTER HERE"),
    };
}

#[test]
fn test_add_two_gauges_and_change_weight() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeCountResponse>(
        GaugeCountResponse { gauge_count: 1 },
        deps.as_ref(),
        QueryMsg::GaugeCount {},
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(100_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_error(
        ContractError::GaugeNotFound {},
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_2".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeAddrResponse>(
        GaugeAddrResponse {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        deps.as_ref(),
        QueryMsg::GaugeAddr { gauge_id: 0_u64 },
        time,
    );

    run_query_msg_expect_error(
        ContractError::GaugeNotFound {},
        deps.as_ref(),
        QueryMsg::GaugeAddr { gauge_id: 1_u64 },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::GaugeAlreadyExists {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::Unauthorized {},
        deps.as_mut(),
        "addr0000".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(100_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeCountResponse>(
        GaugeCountResponse { gauge_count: 2 },
        deps.as_ref(),
        QueryMsg::GaugeCount {},
        time,
    );

    run_query_msg_expect_ok::<AllGaugeAddrResponse>(
        AllGaugeAddrResponse {
            all_gauge_addr: vec!["gauge_addr_1".to_string(), "gauge_addr_2".to_string()],
        },
        deps.as_ref(),
        QueryMsg::AllGaugeAddr {},
        time,
    );

    run_execute_msg_expect_error(
        ContractError::Unauthorized {},
        deps.as_mut(),
        "addr0000".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::GaugeNotFound {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_3".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(200_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(Uint128::from(2_u64), Uint128::from(3_u64)),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::TimestampError {},
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(200_u64),
        },
        time - WEEK,
    );
}

#[test]
fn test_vote_for_single_gauge_by_single_user() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::InvalidVotingRatio {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 10001,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );

    time += WEEK * (VOTE_DELAY - 1);

    run_execute_msg_expect_error(
        ContractError::VoteTooOften {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(988285242_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(978302799_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 0,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(23333_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 100 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(23333_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::LockExpiresTooSoon {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );
}

#[test]
fn test_vote_for_single_gauge_by_multiple_users() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(0_u64),
        },
        time,
    );

    run_query_msg_expect_error(
        ContractError::TotalWeightIsZero {},
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_error(
        ContractError::TotalWeightIsZero {},
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeightAt {
            gauge_addr: "gauge_addr_1".to_string(),
            time: time + 100 * WEEK,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 768,
        },
        time,
    );

    time += WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_2".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 8453,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(908414277_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightAtResponse>(
        GaugeWeightAtResponse {
            gauge_weight_at: Uint128::from(19956276_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeightAt {
            gauge_addr: "gauge_addr_1".to_string(),
            time: time + 73 * WEEK,
        },
        time,
    );

    time += 23 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(596207044_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 42 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(26089489_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 8 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(19956276_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightAtResponse>(
        GaugeWeightAtResponse {
            gauge_weight_at: Uint128::from(908414277_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeightAt {
            gauge_addr: "gauge_addr_1".to_string(),
            time: time - 73 * WEEK,
        },
        time,
    );
}

#[test]
fn test_vote_for_multiple_gauges_by_single_user() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_2".to_string(),
            weight: Uint128::from(66666_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 4357,
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::InsufficientVotingRatio {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_2".to_string(),
            ratio: 5644,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_2".to_string(),
            ratio: 5643,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(434958398_u64),
                Uint128::from(563375954_u64 + 434958398_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightAtResponse>(
        GaugeRelativeWeightAtResponse {
            gauge_relative_weight_at: Decimal::from_ratio(
                Uint128::from(23333_u64),
                Uint128::from(66666_u64 + 23333_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeightAt {
            gauge_addr: "gauge_addr_1".to_string(),
            time: time + 117 * WEEK,
        },
        time,
    );

    run_query_msg_expect_ok::<TotalWeightAtResponse>(
        TotalWeightAtResponse {
            total_weight_at: Uint128::from(66666_u64 + 23333_u64),
        },
        deps.as_ref(),
        QueryMsg::TotalWeightAt {
            time: time + 117 * WEEK,
        },
        time,
    );

    time += 17 * WEEK;

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(361019437_u64),
                Uint128::from(467613375_u64 + 361019437_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 100 * WEEK;

    run_query_msg_expect_ok::<GaugeRelativeWeightResponse>(
        GaugeRelativeWeightResponse {
            gauge_relative_weight: Decimal::from_ratio(
                Uint128::from(23333_u64),
                Uint128::from(66666_u64 + 23333_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_query_msg_expect_ok::<TotalWeightResponse>(
        TotalWeightResponse {
            total_weight: Uint128::from(66666_u64 + 23333_u64),
        },
        deps.as_ref(),
        QueryMsg::TotalWeight {},
        time,
    );

    run_query_msg_expect_ok::<GaugeRelativeWeightAtResponse>(
        GaugeRelativeWeightAtResponse {
            gauge_relative_weight_at: Decimal::from_ratio(
                Uint128::from(434958398_u64),
                Uint128::from(563375954_u64 + 434958398_u64),
            ),
        },
        deps.as_ref(),
        QueryMsg::GaugeRelativeWeightAt {
            gauge_addr: "gauge_addr_1".to_string(),
            time: time - 117 * WEEK,
        },
        time,
    );
}

#[test]
fn test_vote_for_single_gauge_and_cancel() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(23333_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 4357,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_3".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 5644,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(913245837_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(904113612_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_error(
        ContractError::VoteTooOften {},
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 4357,
        },
        time,
    );

    time += WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 5644,
        },
        time,
    );

    time += 33 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(677126093_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 0,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(310910170_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 17 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 9999,
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(708710679_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );

    time += 300 * WEEK;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(200000_u64),
        },
        time,
    );

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(200000_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );
}

#[test]
fn test_overflow() {
    let x = Decimal::MAX;
    match x.checked_mul(u64::MAX) {
        Err(_) => (),
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_bias_be_negative() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(0_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_1".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 10000,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::ChangeGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(0_u64),
        },
        time,
    );

    time += 20 * WEEK;

    run_query_msg_expect_ok::<GaugeWeightResponse>(
        GaugeWeightResponse {
            gauge_weight: Uint128::from(0_u64),
        },
        deps.as_ref(),
        QueryMsg::GaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
        },
        time,
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let time = BASE_TIME;

    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    assert_eq!("owner", config.owner.as_str());
    assert_eq!("anchor_token", config.anchor_token.as_str());
    assert_eq!("anchor_voting_escrow", config.anchor_voting_escrow.as_str());

    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("gov".to_string()),
        anchor_token: Some("anchor2.0".to_string()),
        anchor_voting_escrow: Some("voting_escrow2.0".to_string()),
    };

    run_execute_msg_expect_error(
        ContractError::Unauthorized {},
        deps.as_mut(),
        "addr0001".to_string(),
        msg.clone(),
        time,
    );

    run_execute_msg_expect_ok(deps.as_mut(), "owner".to_string(), msg, time);

    run_query_msg_expect_ok::<ConfigResponse>(
        ConfigResponse {
            owner: "gov".to_string(),
            anchor_token: "anchor2.0".to_string(),
            anchor_voting_escrow: "voting_escrow2.0".to_string(),
        },
        deps.as_ref(),
        QueryMsg::Config {},
        time,
    );
}

#[test]
fn test_vote_decay_faster() {
    // decay normally
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(2000_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_2".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 5000,
        },
        time,
    );

    let mut gauge_weight_normal = vec![];
    let mut env = mock_env();
    for _ in 0..50 {
        env.block.time = Timestamp::from_seconds(time);
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GaugeWeight {
                gauge_addr: "gauge_addr_1".to_string(),
            },
        )
        .unwrap();
        let gauge_weight: GaugeWeightResponse = from_binary(&res).unwrap();
        gauge_weight_normal.push(gauge_weight.gauge_weight);
        time += WEEK;
    }

    // decay faster
    let mut deps = mock_dependencies(&[]);
    let _res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        InstantiateMsg {
            owner: "owner".to_string(),
            anchor_token: "anchor_token".to_string(),
            anchor_voting_escrow: "anchor_voting_escrow".to_string(),
        },
    )
    .unwrap();

    let mut time = BASE_TIME;

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "owner".to_string(),
        ExecuteMsg::AddGauge {
            gauge_addr: "gauge_addr_1".to_string(),
            weight: Uint128::from(2000_u64),
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_4".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );
    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_5".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );
    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_6".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_7".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_8".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );

    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_9".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );
    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_10".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 2000,
        },
        time,
    );
    run_execute_msg_expect_ok(
        deps.as_mut(),
        "user_2".to_string(),
        ExecuteMsg::VoteForGaugeWeight {
            gauge_addr: "gauge_addr_1".to_string(),
            ratio: 5000,
        },
        time,
    );

    let mut gauge_weight_fast = vec![];
    let mut env = mock_env();
    for _ in 0..50 {
        env.block.time = Timestamp::from_seconds(time);
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GaugeWeight {
                gauge_addr: "gauge_addr_1".to_string(),
            },
        )
        .unwrap();
        let gauge_weight: GaugeWeightResponse = from_binary(&res).unwrap();
        gauge_weight_fast.push(gauge_weight.gauge_weight);
        time += WEEK;
    }

    assert_eq!(gauge_weight_normal, gauge_weight_fast);
}
