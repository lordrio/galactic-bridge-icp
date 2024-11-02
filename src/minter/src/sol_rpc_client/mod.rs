use crate::{
    escda,
    lifecycle::SolanaRpcUrl,
    sha3_256,
    sol_rpc_client::{
        requests::{GetSignaturesForAddressRequestOptions, GetTransactionRequestOptions},
        responses::{GetTransactionResponse, JsonRpcResponse, SignatureResponse},
        types::{
            ConfirmationStatus, RpcMethod, HEADER_SIZE_LIMIT, SIGNATURE_RESPONSE_SIZE_ESTIMATE,
            TRANSACTION_RESPONSE_SIZE_ESTIMATE,
        },
    },
    state::{mutate_state, read_state, State},
    AGENT_TOKEN_N_EXPIRY, CHAIN_ID,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as base64_url, Engine};
use ic_cdk::api::{
    call::RejectionCode,
    management_canister::http_request::{
        http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, TransformContext,
    },
};
use icrc_ledger_types::icrc1::transfer::Memo;
use serde_json::json;
use std::collections::HashMap;

pub mod requests;
pub mod responses;
pub mod types;

pub const SECONDS: u64 = 1_000_000_000;
pub const REFRESH_PROXY_TOKEN_INTERVAL: u64 = 60 * 60; // 60 minutes
const AGENT_NAME: &str = "Pipans";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolRpcClient {
    rpc_url: SolanaRpcUrl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolRpcError {
    RequestFailed { code: RejectionCode, msg: String },
    JsonRpcFailed { code: i32, msg: String },
    FromUtf8Failed(String),
    FromStringOfJsonFailed(String),
    ToStringOfJsonFailed(String),
}

impl std::fmt::Display for SolRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolRpcError::RequestFailed { code, msg } => {
                write!(f, "Request failed with code {:?}: {}", code, msg)
            }
            SolRpcError::JsonRpcFailed { code, msg } => {
                write!(f, "JSON-RPC failed with code {:?}: {}", code, msg)
            }
            SolRpcError::FromUtf8Failed(err) => {
                write!(f, "FromUtf8 failed: {}", err)
            }
            SolRpcError::FromStringOfJsonFailed(err) => {
                write!(f, "From String of JSON failed: {}", err)
            }
            SolRpcError::ToStringOfJsonFailed(err) => {
                write!(f, "To String of JSON failed: {}", err)
            }
        }
    }
}

impl SolRpcClient {
    const fn new(rpc_url: SolanaRpcUrl) -> Self {
        Self { rpc_url }
    }

    pub fn from_state(state: &State) -> Self {
        Self::new(state.solana_rpc_url())
    }

    async fn get_agent_token() -> String {
        let (token, expire_at) = AGENT_TOKEN_N_EXPIRY.with(|t| t.borrow().clone());
        if expire_at < (ic_cdk::api::time() / SECONDS) {
            // expired
            let expire_at = (ic_cdk::api::time() / SECONDS) + REFRESH_PROXY_TOKEN_INTERVAL;
            let ecdsa_key_name = read_state(|s| s.ecdsa_key_name.clone());
            let token = escda::sign_proxy_token(&ecdsa_key_name, expire_at + 120, AGENT_NAME)
                .await
                .unwrap();
            AGENT_TOKEN_N_EXPIRY.with(|t| *t.borrow_mut() = (token.clone(), expire_at));
            return token;
        }

        token
    }

    async fn rpc_call(
        &self,
        payload: &String,
        effective_size_estimate: u64,
    ) -> Result<String, SolRpcError> {
        //https://idempotent-proxy-cf-worker.rio-lee.workers.dev
        let token = Self::get_agent_token().await;
        let host = "idempotent-proxy-cf-worker.rio-lee.workers.dev";
        let url = format!("https://{}/URL_SOLANA_DEVNET", host);

        ic_cdk::println!("url: {}", url);

        let chain_id = CHAIN_ID.with(|t| *t.borrow());
        let next_chain =
            sha3_256(format!("{}-{}", hex::encode(chain_id), ic_cdk::api::time()).as_bytes());
        // update new chain id
        CHAIN_ID.with_borrow_mut(|i| *i = next_chain);
        let idempotent_key = format!("{}", base64_url.encode(next_chain));

        ic_cdk::println!("idempotent_key: {}", idempotent_key);

        let request_headers = vec![
            HttpHeader {
                name: "Host".to_string(),
                value: format!("{host}:443"),
            },
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
            HttpHeader {
                name: "idempotency-key".to_string(),
                value: idempotent_key.to_string(),
            },
            HttpHeader {
                name: "proxy-authorization".to_string(),
                value: format!("Bearer {}", token),
            },
        ];

        ic_cdk::println!("body: {}", payload);

        let request = CanisterHttpRequestArgument {
            url: url.to_string(),
            method: HttpMethod::POST,
            max_response_bytes: Some(effective_size_estimate),
            body: Some(payload.as_bytes().to_vec()),
            transform: Some(TransformContext::from_name(
                "cleanup_response".to_owned(),
                vec![],
            )),
            headers: request_headers,
        };

        let base_cycles = 400_000_000u128 + 100_000u128 * (2 * effective_size_estimate as u128);

        const BASE_SUBNET_SIZE: u128 = 13;
        const SUBNET_SIZE: u128 = 34;
        let cycles = base_cycles * SUBNET_SIZE / BASE_SUBNET_SIZE;

        match http_request(request, cycles).await {
            Ok((response,)) => {
                let str_body = String::from_utf8(response.body);
                // ic_cdk::println!("response: {:?}", str_body);

                match str_body {
                    Ok(str_body) => Ok(str_body),
                    Err(error) => {
                        ic_cdk::println!("error 00 : {:?}", error);
                        Err(SolRpcError::FromUtf8Failed(error.to_string()))
                    }
                }
            }
            Err((r, m)) => {
                ic_cdk::println!("error 01 : {:?}, {:?}", r, m);
                Err(SolRpcError::RequestFailed { code: r, msg: m })
            }
        }
    }

    async fn _rpc_call_unused(
        &self,
        payload: &String,
        effective_size_estimate: u64,
    ) -> Result<String, SolRpcError> {
        // Details of the values used in the following lines can be found here:
        // https://internetcomputer.org/docs/current/developer-docs/production/computation-and-storage-costs
        let base_cycles = 400_000_000u128 + 100_000u128 * (2 * effective_size_estimate as u128);

        const BASE_SUBNET_SIZE: u128 = 13;
        const SUBNET_SIZE: u128 = 34;
        let cycles = base_cycles * SUBNET_SIZE / BASE_SUBNET_SIZE;

        let request = CanisterHttpRequestArgument {
            url: self.rpc_url.get().to_string(),
            max_response_bytes: Some(effective_size_estimate),
            method: HttpMethod::POST,
            headers: vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            }],
            body: Some(payload.as_bytes().to_vec()),
            transform: Some(TransformContext::from_name(
                "cleanup_response".to_owned(),
                vec![],
            )),
        };

        match http_request(request, cycles).await {
            Ok((response,)) => {
                let str_body = String::from_utf8(response.body);

                match str_body {
                    Ok(str_body) => Ok(str_body),
                    Err(error) => Err(SolRpcError::FromUtf8Failed(error.to_string())),
                }
            }
            Err((r, m)) => Err(SolRpcError::RequestFailed { code: r, msg: m }),
        }
    }

    // Method relies on the getSignaturesForAddress RPC call to get the signatures for the address:
    // https://solana.com/docs/rpc/http/getsignaturesforaddress
    pub async fn get_signatures_for_address(
        &self,
        limit: u8,
        before: Option<&String>,
        until: &String,
    ) -> Result<Vec<SignatureResponse>, SolRpcError> {
        let params: [&dyn erased_serde::Serialize; 2] = [
            &read_state(|s| s.solana_contract_address.clone()),
            &GetSignaturesForAddressRequestOptions {
                limit: Some(limit),
                commitment: Some(ConfirmationStatus::Confirmed.as_str().to_string()),
                before: before.map(|s| s.to_string()),
                until: Some(until.to_string()),
            },
        ];

        let payload = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": mutate_state(State::next_request_id),
            "method": RpcMethod::GetSignaturesForAddress.as_str(),
            "params": params
        }));
        let payload = if let Err(error) = payload {
            return Err(SolRpcError::ToStringOfJsonFailed(error.to_string()));
        } else {
            payload.unwrap()
        };

        // The effective size estimate is the size of the response we expect to get from the RPC
        // Important: all types of transactions are considered here (e.g. withdraw and deposit)
        // This can lead to issues in case new types of transactions are added in the future.
        let effective_size_estimate: u64 =
            (limit as u64) * SIGNATURE_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(&payload, effective_size_estimate).await {
            Ok(response) => {
                let json_response =
                    serde_json::from_str::<JsonRpcResponse<Vec<SignatureResponse>>>(&response);

                // Check if the response is valid
                match json_response {
                    Ok(json_response) => {
                        // In case error is present in the response ignore the result and return the error
                        if let Some(error) = json_response.error {
                            Err(SolRpcError::JsonRpcFailed {
                                code: error.code,
                                msg: error.message,
                            })
                        } else {
                            Ok(json_response.result.unwrap())
                        }
                    }
                    Err(error) => {
                        return Err(SolRpcError::FromStringOfJsonFailed(error.to_string()))
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }

    // Method relies on the gettransaction RPC call to get the transaction data:
    // https://solana.com/docs/rpc/http/gettransaction
    // It is using a batch request to get multiple transactions at once.
    // cURL Example:
    // curl -X POST -H "Content-Type: application/json" -d '[
    //    {"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["1"]}
    //    {"jsonrpc":"2.0","id":2,"method":"getTransaction","params":["2"]}
    // ]' http://localhost:8899
    pub async fn get_transactions(
        &self,
        signatures: Vec<&String>,
    ) -> Result<HashMap<String, Result<Option<GetTransactionResponse>, SolRpcError>>, SolRpcError>
    {
        let mut rpc_request = Vec::new();

        // Due to batching request_id cannot be used in the payload.
        // But still need to increment it to count the call.
        mutate_state(State::next_request_id);

        for (position, signature) in signatures.iter().enumerate() {
            let params: [&dyn erased_serde::Serialize; 2] = [
                &signature,
                &GetTransactionRequestOptions {
                    commitment: Some(ConfirmationStatus::Confirmed.as_str().to_string()),
                },
            ];

            let transaction = json!({
                "jsonrpc": "2.0",
                "id": position + 1,
                "method": RpcMethod::GetTransaction.as_str().to_string(),
                "params": params,
            });
            rpc_request.push(transaction);
        }

        let payload = serde_json::to_string(&rpc_request);
        let payload = if let Err(error) = payload {
            return Err(SolRpcError::ToStringOfJsonFailed(error.to_string()));
        } else {
            payload.unwrap()
        };

        // The effective size estimate is the size of the response we expect to get from the RPC
        let effective_size_estimate: u64 =
            (signatures.len() as u64) * TRANSACTION_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(&payload, effective_size_estimate).await {
            Ok(response) => {
                let json_responses =
                    serde_json::from_str::<Vec<JsonRpcResponse<GetTransactionResponse>>>(&response);

                match json_responses {
                    Ok(responses) => {
                        let mut map = HashMap::<
                            String,
                            Result<Option<GetTransactionResponse>, SolRpcError>,
                        >::new();

                        responses
                            .into_iter()
                            .enumerate()
                            .for_each(|(index, response)| {
                                // In case error is present in the response ignore the result and return the error
                                let result = if let Some(error) = response.error {
                                    Err(SolRpcError::JsonRpcFailed {
                                        code: error.code,
                                        msg: error.message,
                                    })
                                } else {
                                    Ok(response.result)
                                };

                                map.insert(signatures[index].to_string(), result);
                            });

                        Ok(map)
                    }
                    Err(error) => Err(SolRpcError::FromStringOfJsonFailed(error.to_string())),
                }
            }
            Err(error) => return Err(error),
        }
    }
}

// Memo is limited to 32 bytes in size
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize)]
pub struct LedgerMemo(pub u64);

impl From<LedgerMemo> for Memo {
    fn from(memo: LedgerMemo) -> Self {
        let bytes = serde_cbor::ser::to_vec(&memo).expect("Failed to serialize LedgerMemo");
        Memo::from(bytes)
    }
}
