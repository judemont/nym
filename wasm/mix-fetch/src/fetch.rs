// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::client::MixFetchClient;
use crate::config::{MixFetchConfig, MixFetchConfigOpts, MixFetchDebugOverride};
use crate::error::MixFetchError;
use crate::helpers::get_network_requester;
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tsify::Tsify;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::future_to_promise;
use wasm_client_core::config::r#override::DebugWasmOverride;
use wasm_utils::console_log;
use wasm_utils::error::PromisableResultError;

pub type RequestId = u64;

pub(super) static MIX_FETCH: OnceLock<MixFetchClient> = OnceLock::new();

#[derive(Tsify, Debug, Clone, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct MixFetchOpts {
    // ideally we'd have used the `IdentityKey` type alias, but that'd be extremely annoying to get working in TS
    #[serde(flatten)]
    pub(crate) base: MixFetchOptsSimple,

    #[tsify(optional)]
    pub(crate) client_id: Option<String>,

    #[tsify(optional)]
    pub(crate) nym_api_url: Option<String>,

    // currently not used, but will be required once we have coconut
    #[tsify(optional)]
    pub(crate) nyxd_url: Option<String>,

    #[tsify(optional)]
    pub(crate) client_override: Option<DebugWasmOverride>,

    #[tsify(optional)]
    pub(crate) mix_fetch_override: Option<MixFetchDebugOverride>,
}

#[derive(Tsify, Debug, Clone, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct MixFetchOptsSimple {
    // ideally we'd have used the `IdentityKey` type alias, but that'd be extremely annoying to get working in TS
    #[tsify(optional)]
    pub(crate) preferred_gateway: Option<String>,

    #[tsify(optional)]
    pub(crate) preferred_network_requester: Option<String>,

    #[tsify(optional)]
    pub(crate) storage_passphrase: Option<String>,
}

impl<'a> From<&'a MixFetchOpts> for MixFetchConfigOpts {
    fn from(value: &'a MixFetchOpts) -> Self {
        MixFetchConfigOpts {
            id: value.client_id.as_ref().map(|v| v.to_owned()),
            nym_api: value.nym_api_url.as_ref().map(|v| v.to_owned()),
            nyxd: value.nyxd_url.as_ref().map(|v| v.to_owned()),
            debug: value.client_override.as_ref().map(|&v| v.into()),
        }
    }
}

// TODO: in the future make the network requester address optional once there exists some API for obtaining NR addresses
#[wasm_bindgen(js_name = setupMixFetch)]
pub fn setup_mix_fetch(opts: MixFetchOpts) -> Promise {
    if MIX_FETCH.get().is_some() {
        return MixFetchError::AlreadyInitialised.into_rejected_promise();
    }

    // if nym api was overridden, it means we're not using mainnet and we don't have harbourmaster url
    // for anything that's not mainnet
    if opts.nym_api_url.is_some() && opts.base.preferred_network_requester.is_none() {
        return MixFetchError::NoNetworkRequesters.into_rejected_promise();
    }

    future_to_promise(async move {
        let network_requester_address =
            get_network_requester(opts.base.preferred_network_requester.clone())
                .await
                .map_promise_err()?;

        console_log!("going to use {network_requester_address} network requester");

        let mut config = MixFetchConfig::new(network_requester_address, Some((&opts).into()))
            .map_promise_err()?;
        if let Some(dbg) = opts.client_override {
            config.override_debug(dbg);
        }

        if let Some(dbg) = opts.mix_fetch_override {
            config.override_mix_fetch_debug(dbg)
        }

        setup_mix_fetch_async(config, opts.base)
            .await
            .map(|_| JsValue::undefined())
            .map_promise_err()
    })
}

#[wasm_bindgen(js_name = setupMixFetchWithConfig)]
pub fn setup_mix_fetch_with_config(config: MixFetchConfig, opts: MixFetchOptsSimple) -> Promise {
    if MIX_FETCH.get().is_some() {
        return MixFetchError::AlreadyInitialised.into_rejected_promise();
    }

    future_to_promise(async move {
        setup_mix_fetch_async(config, opts)
            .await
            .map(|_| JsValue::undefined())
            .map_promise_err()
    })
}

pub(super) fn set_mix_fetch_client(mix_fetch_client: MixFetchClient) -> Result<(), MixFetchError> {
    MIX_FETCH
        .set(mix_fetch_client)
        .map_err(|_| MixFetchError::AlreadyInitialised)
}

pub(super) fn mix_fetch_client() -> Result<&'static MixFetchClient, MixFetchError> {
    MIX_FETCH.get().ok_or(MixFetchError::Uninitialised)
}

async fn setup_mix_fetch_async(
    config: MixFetchConfig,
    opts: MixFetchOptsSimple,
) -> Result<(), MixFetchError> {
    let preferred_gateway = opts.preferred_gateway;
    let storage_passphrase = opts.storage_passphrase;
    let client = MixFetchClient::new_async(config, preferred_gateway, storage_passphrase).await?;
    set_mix_fetch_client(client)?;
    Ok(())
}
