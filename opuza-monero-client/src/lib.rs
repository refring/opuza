#![allow(clippy::all)]

use ::monero::cryptonote::subaddress::Index;
use ::monero::Address;
use hex::FromHex;
use monero_rpc::TransferHeight::Confirmed;
use monero_rpc::{
  BlockHeightFilter, GetTransfersCategory, GetTransfersSelector, SubaddressData, TransferHeight,
};
use openssl::sha::sha256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use {core::fmt::Debug, std::error::Error, std::fmt};

pub use piconero::Piconero;

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
    serde_json::from_str::<OpuzaInvoice>(&item.label).unwrap_or_default()
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
      .build(self.inner.clone())
      .unwrap();
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
      .build(self.inner.clone())
      .unwrap();
    let wallet_rpc = daemon_client.wallet();

    let block_height = wallet_rpc.get_height().await;

    block_height.map_err(|_| OpuzaRpcError)?;

    let mut monero_invoice = OpuzaInvoice::new();
    monero_invoice.value = value.value();
    monero_invoice.memo = memo.to_owned();

    let (address, index) = wallet_rpc
      .create_address(0, Some(serde_json::to_string(&monero_invoice).unwrap()))
      .await
      .map_err(|_| OpuzaRpcError)?;

    let cln_inv: AddOpuzaInvoiceResponse = address.into();

    // The invoice id is based on the Monero subaddress
    monero_invoice.payment_hash = cln_inv.payment_hash.clone();
    monero_invoice.payment_request = format!(
      "monero:{}?tx_amount={}&tx_description={}",
      address,
      Piconero::new(monero_invoice.value).as_xmr(),
      monero_invoice.memo
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
      .build(self.inner.clone())
      .unwrap();
    let wallet_rpc = daemon_client.wallet();

    // Retrieve the address data using the payment_hash as a key
    let sub_address = wallet_rpc
      .get_attribute(format!("inv_{}", &payment_hash_hex))
      .await
      .map_err(|_| OpuzaRpcError)?;

    let address = Address::from_hex(sub_address).map_err(|_| OpuzaRpcError)?;
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
      .build(self.inner.clone())
      .unwrap();
    let wallet_rpc = daemon_client.wallet();

    let mut category_selector = HashMap::new();
    category_selector.insert(GetTransfersCategory::In, true);
    category_selector.insert(GetTransfersCategory::Pending, true);
    category_selector.insert(GetTransfersCategory::Pool, true);

    let mut transfer_selector = GetTransfersSelector::default();
    transfer_selector.category_selector = category_selector;

    let last_block_height = wallet_rpc
      .get_attribute("block_height".to_string())
      .await
      .map_err(|_| OpuzaRpcError);

    let last_block_height: u64 = match last_block_height {
      Ok(height) => {
        let block_height_attribute: u64 = height.parse().unwrap();
        // We check the last 10 blocks for transactions
        block_height_attribute - 10
      }
      Err(_e) => 0,
    };

    transfer_selector.block_height_filter = Some(BlockHeightFilter {
      min_height: Some(last_block_height),
      max_height: None,
    });

    let transfers = wallet_rpc.get_transfers(transfer_selector).await;

    let current_block_height = wallet_rpc.get_height().await.unwrap();
    println!(
      "Start scanning from {}, current block height {}",
      last_block_height, current_block_height
    );

    let mut update_block_height = true;

    for (_transfer_category, transfers) in transfers.unwrap().into_iter() {
      for transfer in transfers.iter() {
        println!("==\nTransfer: {:?}", transfer);
        let address_filter = vec![transfer.subaddr_index.minor];
        let address = wallet_rpc.get_address(0, Some(address_filter)).await;

        let address_tmp = address.unwrap();
        let sub_address = address_tmp.addresses.get(0).ok_or_else(|| OpuzaRpcError);
        let cln_inv: OpuzaInvoice = sub_address.clone().unwrap().clone().into();
        println!("Invoice: {:?}\n==\n", cln_inv);

        let mut minimum_confirmations = 0;

        // If double_spend_seen is set to true we want a couple confirmations, see:
        // https://github.com/monero-project/monero/commit/ccf53a566c1c2e980ed30a7371b8789ffb4c01a7
        if transfer.double_spend_seen != false {
          minimum_confirmations += 1;
        }

        // Locked transactions are not spendable until the block in transfer.unlock_time
        // Make sure that the minimum amount on confirmations is adjusted for this
        if transfer.unlock_time > 0 && transfer.unlock_time > current_block_height.get() {
          minimum_confirmations = if let Confirmed(block_height) = transfer.height {
            transfer.unlock_time - block_height.get()
          } else {
            transfer.unlock_time - current_block_height.get()
          };

          println!(
            "Found locked transaction, setting min confirms to {}",
            minimum_confirmations
          );
        }

        let transfer_confirmations = transfer.confirmations.unwrap_or(0);

        if transfer.height == TransferHeight::InPool
          && minimum_confirmations > 0
          && transfer_confirmations > 0
        {
          minimum_confirmations = transfer_confirmations + minimum_confirmations;
        }

        if transfer_confirmations >= minimum_confirmations
          && transfer.amount.as_pico() >= cln_inv.value
        {
          let mut monero_invoice: OpuzaInvoice = sub_address.unwrap().clone().into();

          if monero_invoice.is_settled != true {
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
        } else {
          // When we arrive here we skipped a transaction so we want to try again later
          update_block_height = false;
        }
      }
    }

    if update_block_height == true {
      // All transactions have been processed up until update_block_height
      // This is mechanism is mainly because of unlock_time
      let _attribute_result = wallet_rpc
        .set_attribute("block_height".to_string(), current_block_height.to_string())
        .await;
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpuzaInvoice {
  pub value: u64,
  pub amount_settled: u64,
  pub is_settled: bool,
  pub memo: String,
  pub payment_hash: String,
  pub payment_request: String,
}

impl OpuzaInvoice {
  fn new() -> Self {
    Default::default()
  }
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
