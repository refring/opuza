#![allow(clippy::all)]

use ::monero::cryptonote::subaddress::Index;
use ::monero::Address;
use monero_rpc::{GetTransfersCategory, GetTransfersSelector, RpcClient, SubaddressData};
use openssl::sha::sha256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use {core::fmt::Debug, std::error::Error, std::fmt};

pub use piconero::Piconero;

mod https_service;
mod piconero;

#[cfg(test)]
use {lnd_test_context::LndTestContext, std::sync::Arc};

impl From<Address> for AddOpuzaInvoiceResponse {
  fn from(address: Address) -> Self {
    let address_bytes = address.as_bytes();
    AddOpuzaInvoiceResponse {
      payment_hash: hex::encode(sha256(&address_bytes)),
    }
  }
}

impl From<SubaddressData> for OpuzaInvoice {
  fn from(item: SubaddressData) -> Self {
    let monero_invoice: OpuzaInvoice = serde_json::from_str(&item.label).unwrap();

    OpuzaInvoice {
      value: monero_invoice.value,
      is_settled: monero_invoice.is_settled,
      memo: monero_invoice.memo,
      payment_hash: monero_invoice.payment_hash,
      payment_request: monero_invoice.payment_request,
    }
  }
}

#[derive(Debug, Clone)]
pub struct MoneroRpcClient {
  inner: String,
  #[cfg(test)]
  _lnd_test_context: Arc<LndTestContext>,
}

impl MoneroRpcClient {
  pub async fn new(
    rpc_address: String,
    #[cfg(test)] lnd_test_context: LndTestContext,
  ) -> MoneroRpcClient {
    let inner = rpc_address;

    MoneroRpcClient {
      inner,
      #[cfg(test)]
      _lnd_test_context: Arc::new(lnd_test_context),
    }
  }

  pub async fn ping(&self) -> Result<(), OpuzaRpcError> {
    let daemon_client = RpcClient::new(self.inner.clone());
    // let daemon_rpc = daemon_client.daemon_rpc();
    let daemon = daemon_client.wallet();

    let block_height = daemon.get_height().await;

    block_height.map_err(|_| OpuzaRpcError)?;

    Ok(())
  }

  pub async fn add_invoice(
    &self,
    memo: &str,
    value: Piconero,
  ) -> Result<AddOpuzaInvoiceResponse, OpuzaRpcError> {
    let daemon_client = RpcClient::new(self.inner.clone());
    let wallet_rpc = daemon_client.wallet();

    let block_height = wallet_rpc.get_height().await;

    block_height.map_err(|_| OpuzaRpcError)?;

    let mut monero_invoice = OpuzaInvoice {
      value: value.value(),
      memo: memo.to_owned(),
      payment_hash: String::from(""),
      is_settled: false,
      payment_request: String::from(""),
    };

    let (address, index) = wallet_rpc
      .create_address(0, Some(serde_json::to_string(&monero_invoice).unwrap()))
      .await
      .map_err(|_| OpuzaRpcError)?;

    let cln_inv: AddOpuzaInvoiceResponse = address.into();

    // The invoice id is based on the Monero subaddress
    monero_invoice.payment_hash = cln_inv.payment_hash.clone();
    monero_invoice.payment_request = format!(
      "monero:{}?tx_amount={}&tx_description={}",
      address, monero_invoice.value, monero_invoice.memo
    );

    // Save the metadata we need later on as serialized data in the wallet
    let label = serde_json::to_string(&monero_invoice).unwrap();
    wallet_rpc
      .label_address(
        Index {
          major: 0,
          minor: index,
        },
        label,
      )
      .await
      .map_err(|_| OpuzaRpcError)?;

    Ok(cln_inv as _)
  }

  pub async fn lookup_invoice(
    &self,
    r_hash: [u8; 32],
  ) -> Result<Option<OpuzaInvoice>, OpuzaRpcError> {
    let payment_hash_hex = hex::encode(&r_hash);

    let daemon_client = RpcClient::new(self.inner.clone());
    // let daemon_rpc = daemon_client.daemon_rpc();
    let wallet_rpc = daemon_client.wallet();

    let address = wallet_rpc.get_address(0, None).await;

    let sub_address_data: Vec<SubaddressData> = address
      .unwrap()
      .addresses
      .into_iter()
      .filter(|x| x.label.contains(&payment_hash_hex))
      .collect();

    let sub_address = sub_address_data.get(0).ok_or_else(|| OpuzaRpcError);

    let cln_inv: OpuzaInvoice = sub_address?.clone().into();

    println!("lookup invoice {:?}", cln_inv);

    self.update_payments().await;

    Ok(Some(cln_inv))
  }

  async fn update_payments(&self) {
    let daemon_client = RpcClient::new(self.inner.clone());
    let wallet_rpc = daemon_client.wallet();

    let mut category_selector = HashMap::new();
    category_selector.insert(GetTransfersCategory::In, true);

    let mut transfer_selector = GetTransfersSelector::default();
    transfer_selector.category_selector = category_selector;

    let transfers = wallet_rpc.get_transfers(transfer_selector).await;

    if let Some(transfers) = transfers.unwrap().get(&GetTransfersCategory::In) {
      for transfer in transfers.iter() {
        println!("Transfer: {:?}", transfer);
        let address_filter = vec![transfer.subaddr_index.minor];
        let address = wallet_rpc.get_address(0, Some(address_filter)).await;

        let address_tmp = address.unwrap();
        let sub_address = address_tmp.addresses.get(0).ok_or_else(|| OpuzaRpcError);
        let cln_inv: OpuzaInvoice = sub_address.clone().unwrap().clone().into();
        println!("Invoice: {:?}", cln_inv);

        if transfer.double_spend_seen == false && transfer.amount.as_pico() >= cln_inv.value {
          let mut monero_invoice: OpuzaInvoice =
            serde_json::from_str(&sub_address.unwrap().label).unwrap();
          monero_invoice.is_settled = true;
          // Save the metadata we need later on as serialized data in the wallet
          let label = serde_json::to_string(&monero_invoice).unwrap();
          let index = Index {
            major: 0,
            minor: transfer.subaddr_index.minor.clone(),
          };
          let _result = wallet_rpc
            .label_address(index, label)
            .await
            .map_err(|_| OpuzaRpcError);
        }
      }
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpuzaInvoice {
  pub value: u64,
  pub is_settled: bool,
  pub memo: String,
  pub payment_hash: String,
  pub payment_request: String,
}

pub fn buffer_to_hex<T, S>(buffer: &T, serializer: S) -> Result<S::Ok, S::Error>
where
  T: AsRef<[u8]>,
  S: Serializer,
{
  serializer.serialize_str(&hex::encode(&buffer.as_ref()))
}

/// Deserializes a lowercase hex string to a `Vec<u8>`.
pub fn hex_to_buffer<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
where
  D: Deserializer<'de>,
{
  use serde::de::Error;

  let mut bytes: [u8; 32] = [0; 32];

  String::deserialize(deserializer).and_then(|string| {
    hex::decode_to_slice(&string, &mut bytes as &mut [u8])
      .map_err(|err| Error::custom(err.to_string()))?;
    Ok(bytes)
  })
}

#[derive(Debug, Clone)]
pub struct AddOpuzaInvoiceResponse {
  pub payment_hash: String,
}

#[derive(Debug, Clone)]
pub struct OpuzaRpcError;

impl fmt::Display for OpuzaRpcError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "failed Monero node request")
  }
}

impl Error for OpuzaRpcError {
  fn description(&self) -> &str {
    // TODO: replace with actual description from error status.
    "failed Monero node request"
  }
}
