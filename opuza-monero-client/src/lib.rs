#![allow(clippy::all)]

use ::monero::cryptonote::subaddress::Index;
use ::monero::Address;
use hex::FromHex;
use monero_rpc::{BlockHeightFilter, GetTransfersCategory, GetTransfersSelector, RpcClient, SubaddressData};
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
    let daemon_client = monero_rpc::RpcClientBuilder::new()
        .build(self.inner.clone()).unwrap();
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
    let daemon_client = monero_rpc::RpcClientBuilder::new()
        .build(self.inner.clone()).unwrap();
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

    // Save the payment hash as an attribute with the address as a value so we can lookup easier later
    wallet_rpc
      .set_attribute(
        format!("inv_{}", monero_invoice.payment_hash),
        address.as_hex(),
      )
      .await
      .map_err(|_| OpuzaRpcError)?;

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

    let daemon_client = monero_rpc::RpcClientBuilder::new()
        .build(self.inner.clone()).unwrap();
    let wallet_rpc = daemon_client.wallet();

    // Retrieve the address data using the payment_hash as a key
    let sub_address = wallet_rpc
      .get_attribute(format!("inv_{}", &payment_hash_hex))
      .await
      .map_err(|_| OpuzaRpcError)?;

    // @TODO replace when monero-rpc-rs switches to monero-rs v18
    // let address = Address::from_hex(sub_address).map_err(|_| OpuzaRpcError)?;
    let address = from_hex(sub_address).map_err(|_| OpuzaRpcError)?;
    let sub_address_idx = wallet_rpc
      .get_address_index(address)
      .await
      .map_err(|_| OpuzaRpcError)?;

    let address = wallet_rpc
      .get_address(sub_address_idx.major, Some(vec![sub_address_idx.minor]))
      .await;

    let sub_address_data: Vec<SubaddressData> = address.map_err(|_| OpuzaRpcError)?.addresses;

    let sub_address = sub_address_data.get(0).ok_or_else(|| OpuzaRpcError);

    let cln_inv: OpuzaInvoice = sub_address?.clone().into();

    println!("lookup invoice {:?}", cln_inv);

    Ok(Some(cln_inv))
  }

  pub async fn update_payments(&self) {
    let daemon_client = monero_rpc::RpcClientBuilder::new()
        .build(self.inner.clone()).unwrap();
    let wallet_rpc = daemon_client.wallet();

    let mut category_selector = HashMap::new();
    category_selector.insert(GetTransfersCategory::In, true);

    let mut transfer_selector = GetTransfersSelector::default();
    transfer_selector.category_selector = category_selector;

    let last_block_height = wallet_rpc.get_attribute("block_height".to_string()).await
        .map_err(|_| OpuzaRpcError);

    let last_block_height: u64 = match last_block_height{
      Ok(height) => height.parse().unwrap(),
      Err(e) => 0
    };

    println!("Start scanning for transfers from blockheight {}", last_block_height);

    transfer_selector.block_height_filter = Some(BlockHeightFilter{
      min_height: Some(last_block_height),
      max_height: None
    });

    let transfers = wallet_rpc.get_transfers(transfer_selector).await;

    let current_block_height = wallet_rpc.get_height().await.unwrap();
    println!("Current block height {}", current_block_height);

    if let Some(transfers) = transfers.unwrap().get(&GetTransfersCategory::In) {
      // store block_height
      wallet_rpc.set_attribute("block_height".to_string(), current_block_height.to_string()).await;

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

fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Address, monero::util::address::Error> {
  let hex = hex.as_ref();
  let hex = hex.strip_prefix("0x".as_bytes()).unwrap_or(hex);
  let bytes = hex::decode(hex).map_err(|_| monero::util::address::Error::InvalidFormat)?;
  Address::from_bytes(&bytes)
}
