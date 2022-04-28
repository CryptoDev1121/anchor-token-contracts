use crate::error::ContractError;
use crate::state::{
    Config, GaugeWeight, UserVote, CONFIG, GAUGE_ADDR, GAUGE_COUNT, GAUGE_WEIGHT, USER_RATIO,
    USER_VOTES,
};
use crate::utils::{
    cancel_scheduled_slope_change, check_if_exists, checkpoint_gauge, deserialize_pair,
    fetch_latest_checkpoint, get_gauge_weight_at, get_period, get_total_weight_at,
    query_last_user_slope, query_user_unlock_period, schedule_slope_change,
    DecimalRoundedCheckedMul,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Decimal, Deps, DepsMut, Env, Fraction, MessageInfo, Response, StdError,
    StdResult, Uint128,
};

use cw_storage_plus::U64Key;

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeRelativeWeightAtResponse, GaugeRelativeWeightResponse, GaugeWeightAtResponse,
    GaugeWeightResponse, InstantiateMsg, MigrateMsg, QueryMsg, TotalWeightAtResponse,
    TotalWeightResponse,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    validate_period_duration(msg.period_duration)?;
    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
            anchor_voting_escrow: deps.api.addr_canonicalize(&msg.anchor_voting_escrow)?,
            period_duration: msg.period_duration,
            user_vote_delay: msg.user_vote_delay,
        },
    )?;
    GAUGE_COUNT.save(deps.storage, &0)?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddGauge { gauge_addr, weight } => {
            add_gauge(deps, env, info, gauge_addr, weight)
        }
        ExecuteMsg::ChangeGaugeWeight { gauge_addr, weight } => {
            change_gauge_weight(deps, env, info, gauge_addr, weight)
        }
        ExecuteMsg::VoteForGaugeWeight { gauge_addr, ratio } => {
            vote_for_gauge_weight(deps, env, info, gauge_addr, ratio)
        }
        ExecuteMsg::UpdateConfig {
            owner,
            anchor_token,
            anchor_voting_escrow,
            user_vote_delay,
        } => update_config(
            deps,
            info,
            owner,
            anchor_token,
            anchor_voting_escrow,
            user_vote_delay,
        ),
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    anchor_token: Option<String>,
    anchor_voting_escrow: Option<String>,
    user_vote_delay: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(anchor_token) = anchor_token {
        config.anchor_token = deps.api.addr_canonicalize(&anchor_token)?;
    }

    if let Some(anchor_voting_escrow) = anchor_voting_escrow {
        config.anchor_voting_escrow = deps.api.addr_canonicalize(&anchor_voting_escrow)?;
    }

    if let Some(user_vote_delay) = user_vote_delay {
        config.user_vote_delay = user_vote_delay;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

fn validate_period_duration(period_duration: u64) -> StdResult<()> {
    if Uint128::from(period_duration) <= Uint128::zero() {
        Err(StdError::generic_err("period_duration must be > 0"))
    } else {
        Ok(())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GaugeCount {} => Ok(to_binary(&query_gauge_count(deps)?)?),
        QueryMsg::GaugeWeight { gauge_addr } => {
            Ok(to_binary(&query_gauge_weight(deps, env, gauge_addr)?)?)
        }
        QueryMsg::GaugeWeightAt { gauge_addr, time } => {
            Ok(to_binary(&query_gauge_weight_at(deps, gauge_addr, time)?)?)
        }
        QueryMsg::TotalWeight {} => Ok(to_binary(&query_total_weight(deps, env)?)?),
        QueryMsg::TotalWeightAt { time } => Ok(to_binary(&query_total_weight_at(deps, time)?)?),
        QueryMsg::GaugeRelativeWeight { gauge_addr } => Ok(to_binary(
            &query_gauge_relative_weight(deps, env, gauge_addr)?,
        )?),
        QueryMsg::GaugeRelativeWeightAt { gauge_addr, time } => Ok(to_binary(
            &query_gauge_relative_weight_at(deps, gauge_addr, time)?,
        )?),
        QueryMsg::GaugeAddr { gauge_id } => Ok(to_binary(&query_gauge_addr(deps, gauge_id)?)?),
        QueryMsg::AllGaugeAddr {} => Ok(to_binary(&query_all_gauge_addr(deps)?)?),
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
    }
}

fn add_gauge(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    gauge_addr: String,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_canonicalize(info.sender.as_str())?;
    let config = CONFIG.load(deps.storage)?;

    if config.owner != sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&gauge_addr)?;

    if check_if_exists(deps.storage, &addr) {
        return Err(ContractError::GaugeAlreadyExists {});
    }

    let gauge_count = GAUGE_COUNT.load(deps.storage)?;

    GAUGE_ADDR.save(deps.storage, U64Key::new(gauge_count), &addr)?;
    GAUGE_COUNT.save(deps.storage, &(gauge_count + 1))?;

    let period = get_period(env.block.time.seconds(), config.period_duration);

    GAUGE_WEIGHT.save(
        deps.storage,
        (addr.clone(), U64Key::new(period)),
        &GaugeWeight {
            bias: weight,
            slope: Decimal::zero(),
        },
    )?;

    Ok(Response::new().add_attribute("action", "add_gauge"))
}

fn change_gauge_weight(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    gauge_addr: String,
    weight: Uint128,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_canonicalize(info.sender.as_str())?;
    let config = CONFIG.load(deps.storage)?;

    if config.owner != sender {
        return Err(ContractError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&gauge_addr)?;
    let period = get_period(env.block.time.seconds(), config.period_duration);

    checkpoint_gauge(deps.storage, &addr, period)?;

    let latest_checkpoint = fetch_latest_checkpoint(deps.storage, &addr)?;

    let pair = latest_checkpoint.unwrap();
    let (_, latest_weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;

    GAUGE_WEIGHT.save(
        deps.storage,
        (addr.clone(), U64Key::new(period)),
        &GaugeWeight {
            bias: weight,
            slope: latest_weight.slope,
        },
    )?;

    Ok(Response::new().add_attribute("action", "change_gauge_weight"))
}

fn vote_for_gauge_weight(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    gauge_addr: String,
    ratio: u64,
) -> Result<Response, ContractError> {
    if ratio > 10000_u64 {
        return Err(ContractError::InvalidVotingRatio {});
    }

    let sender = info.sender;
    let config = CONFIG.load(deps.storage)?;
    let addr = deps.api.addr_validate(&gauge_addr)?;
    let current_period = get_period(env.block.time.seconds(), config.period_duration);
    let mut old_ratio = 0;
    if let Some(vote) = USER_VOTES.may_load(deps.storage, (sender.clone(), addr.clone()))? {
        old_ratio = vote.ratio;
        if current_period < vote.vote_period + config.user_vote_delay {
            return Err(ContractError::VoteTooOften {});
        }
    }

    let used_ratio = USER_RATIO
        .may_load(deps.storage, sender.clone())?
        .unwrap_or(0);

    if used_ratio - old_ratio + ratio > 10000_u64 {
        return Err(ContractError::InsufficientVotingRatio {});
    }

    let user_unlock_period = query_user_unlock_period(deps.as_ref(), sender.clone())?;

    if user_unlock_period <= current_period {
        return Err(ContractError::LockExpiresTooSoon {});
    }

    let user_full_slope = query_last_user_slope(deps.as_ref(), sender.clone())?;

    let mut user_slope = Decimal::from_ratio(
        Uint128::from(ratio).checked_mul(Uint128::from(user_full_slope.numerator()))?,
        Uint128::from(10000_u64).checked_mul(Uint128::from(user_full_slope.denominator()))?,
    );

    checkpoint_gauge(deps.storage, &addr, current_period)?;

    let pair = fetch_latest_checkpoint(deps.storage, &addr)?.unwrap();
    let (_, mut weight) = deserialize_pair::<GaugeWeight>(Ok(pair))?;

    let dt = user_unlock_period - current_period;

    if user_slope.checked_mul(dt)?.is_zero() {
        user_slope = Decimal::zero();
    }

    weight.slope = weight.slope + user_slope;
    weight.bias += user_slope.checked_mul(dt)?;

    schedule_slope_change(deps.storage, &addr, user_slope, user_unlock_period)?;

    if let Some(vote) = USER_VOTES.may_load(deps.storage, (sender.clone(), addr.clone()))? {
        if vote.unlock_period > current_period {
            let dt = vote.unlock_period - current_period;

            weight.slope = if weight.slope > vote.slope {
                weight.slope - vote.slope
            } else {
                Decimal::zero()
            };
            weight.bias = weight.bias.saturating_sub(vote.slope.checked_mul(dt)?);

            cancel_scheduled_slope_change(deps.storage, &addr, vote.slope, vote.unlock_period)?;
        }

        USER_RATIO.update(
            deps.storage,
            sender.clone(),
            |ratio_opt| -> Result<u64, ContractError> { Ok(ratio_opt.unwrap() - vote.ratio) },
        )?;
    }

    GAUGE_WEIGHT.save(
        deps.storage,
        (addr.clone(), U64Key::new(current_period)),
        &weight,
    )?;

    USER_VOTES.save(
        deps.storage,
        (sender.clone(), addr.clone()),
        &UserVote {
            slope: user_slope,
            vote_period: current_period,
            unlock_period: user_unlock_period,
            ratio,
        },
    )?;

    USER_RATIO.update(
        deps.storage,
        sender,
        |ratio_opt| -> Result<u64, ContractError> {
            if let Some(pratio) = ratio_opt {
                Ok(pratio + ratio)
            } else {
                Ok(ratio)
            }
        },
    )?;

    Ok(Response::new().add_attribute("action", "vote_for_gauge_weight"))
}

fn query_gauge_weight(
    deps: Deps,
    env: Env,
    gauge_addr: String,
) -> Result<GaugeWeightResponse, ContractError> {
    let addr = deps.api.addr_validate(&gauge_addr)?;
    Ok(GaugeWeightResponse {
        gauge_weight: get_gauge_weight_at(deps.storage, &addr, env.block.time.seconds())?,
    })
}

fn query_total_weight(deps: Deps, env: Env) -> Result<TotalWeightResponse, ContractError> {
    Ok(TotalWeightResponse {
        total_weight: get_total_weight_at(deps.storage, env.block.time.seconds())?,
    })
}

fn query_gauge_relative_weight(
    deps: Deps,
    env: Env,
    gauge_addr: String,
) -> Result<GaugeRelativeWeightResponse, ContractError> {
    let addr = deps.api.addr_validate(&gauge_addr)?;
    let gauge_weight = get_gauge_weight_at(deps.storage, &addr, env.block.time.seconds())?;
    let total_weight = get_total_weight_at(deps.storage, env.block.time.seconds())?;

    if total_weight == Uint128::zero() {
        return Err(ContractError::TotalWeightIsZero {});
    }

    Ok(GaugeRelativeWeightResponse {
        gauge_relative_weight: Decimal::from_ratio(gauge_weight, total_weight),
    })
}

fn query_gauge_weight_at(
    deps: Deps,
    gauge_addr: String,
    time: u64,
) -> Result<GaugeWeightAtResponse, ContractError> {
    let addr = deps.api.addr_validate(&gauge_addr)?;

    Ok(GaugeWeightAtResponse {
        gauge_weight_at: get_gauge_weight_at(deps.storage, &addr, time)?,
    })
}

fn query_total_weight_at(deps: Deps, time: u64) -> Result<TotalWeightAtResponse, ContractError> {
    Ok(TotalWeightAtResponse {
        total_weight_at: get_total_weight_at(deps.storage, time)?,
    })
}

fn query_gauge_relative_weight_at(
    deps: Deps,
    gauge_addr: String,
    time: u64,
) -> Result<GaugeRelativeWeightAtResponse, ContractError> {
    let addr = deps.api.addr_validate(&gauge_addr)?;
    let gauge_weight = get_gauge_weight_at(deps.storage, &addr, time)?;
    let total_weight = get_total_weight_at(deps.storage, time)?;

    if total_weight == Uint128::zero() {
        return Err(ContractError::TotalWeightIsZero {});
    }

    Ok(GaugeRelativeWeightAtResponse {
        gauge_relative_weight_at: Decimal::from_ratio(gauge_weight, total_weight),
    })
}

fn query_gauge_count(deps: Deps) -> Result<GaugeCountResponse, ContractError> {
    Ok(GaugeCountResponse {
        gauge_count: GAUGE_COUNT.load(deps.storage)?,
    })
}

fn query_gauge_addr(deps: Deps, gauge_id: u64) -> Result<GaugeAddrResponse, ContractError> {
    if gauge_id >= GAUGE_COUNT.load(deps.storage)? {
        return Err(ContractError::GaugeNotFound {});
    }

    let gauge_addr = GAUGE_ADDR.load(deps.storage, U64Key::new(gauge_id))?;

    Ok(GaugeAddrResponse {
        gauge_addr: gauge_addr.to_string(),
    })
}

fn query_all_gauge_addr(deps: Deps) -> Result<AllGaugeAddrResponse, ContractError> {
    let gauge_count = GAUGE_COUNT.load(deps.storage)?;
    let mut all_gauge_addr = vec![];

    for i in 0..gauge_count {
        let gauge_addr = GAUGE_ADDR.load(deps.storage, U64Key::new(i))?;
        all_gauge_addr.push(gauge_addr.to_string());
    }

    Ok(AllGaugeAddrResponse { all_gauge_addr })
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        anchor_token: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
        anchor_voting_escrow: deps
            .api
            .addr_humanize(&config.anchor_voting_escrow)?
            .to_string(),
        period_duration: config.period_duration,
        user_vote_delay: config.user_vote_delay,
    })
}

pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
