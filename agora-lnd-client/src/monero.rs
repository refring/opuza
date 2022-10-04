use std::collections::HashMap;
use monero_rpc::{GetTransfersCategory, GetTransfersSelector, RpcClient, SubaddressData};
use monero::Address;
use openssl::sha::{sha256};
use serde::{Deserialize, Serialize};

use {
    crate::millisatoshi::Millisatoshi, crate::AddLightningInvoiceResponse, crate::LightningError,
    crate::LightningInvoice, crate::LightningNodeClient, async_trait::async_trait, std::str,
};

#[cfg(test)]
use {lnd_test_context::LndTestContext, std::sync::Arc};

#[cfg(unix)]
impl From<Address> for AddLightningInvoiceResponse {
    fn from(address: Address) -> Self {
        let address_bytes = address.as_bytes();
        AddLightningInvoiceResponse {
            payment_hash: sha256(&address_bytes).as_slice().to_vec(),
        }
    }
}

#[cfg(unix)]
impl From<SubaddressData> for LightningInvoice {
    fn from(item: SubaddressData) -> Self {
        let monero_invoice: MoneroInvoice = serde_json::from_str(&item.label).unwrap();

        let mut payment_hash_vec = [0; 32];
        hex::decode_to_slice(monero_invoice.payment_hash, &mut payment_hash_vec).expect("failed decoding hex");

        LightningInvoice {
            value_msat: Millisatoshi::new(monero_invoice.value_msat),
            is_settled: monero_invoice.is_settled,
            memo: monero_invoice.memo,
            // payment_hash: sha256(&bytes).as_slice().to_vec(),
            payment_hash: payment_hash_vec.to_vec(),
            payment_request: monero_invoice.payment_request
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MoneroRpcClient {
    inner: String,
    #[cfg(test)]
    _lnd_test_context: Arc<LndTestContext>,
}

#[async_trait]
impl LightningNodeClient for MoneroRpcClient {
    #[cfg(unix)]
    async fn ping(&self) -> Result<(), LightningError> {
        let daemon_client = RpcClient::new(self.inner.clone());
        // let daemon_rpc = daemon_client.daemon_rpc();
        let daemon = daemon_client.wallet();

        let block_height = daemon.get_height().await;

        block_height.map_err(|_| LightningError)?;

        Ok(())
    }

    #[cfg(not(unix))]
    async fn ping(&self) -> Result<(), LightningError> {
        Err(LightningError)
    }

    #[cfg(unix)]
    async fn add_invoice(
        &self,
        memo: &str,
        value_msat: Millisatoshi,
    ) -> Result<AddLightningInvoiceResponse, LightningError> {
        let daemon_client = RpcClient::new(self.inner.clone());
        let wallet_rpc = daemon_client.wallet();

        let block_height = wallet_rpc.get_height().await;

        block_height.map_err(|_| LightningError)?;

        let mut monero_invoice = MoneroInvoice{
            value_msat: value_msat.value(),
            memo: memo.to_owned(),
            payment_hash: "".to_string(),
            is_settled: false,
            payment_request: String::from("")
        };

        let (address, index) = wallet_rpc.
            create_address(0, Some(serde_json::to_string(&monero_invoice).unwrap()))
            .await
            .map_err(|_| LightningError)?;

        let cln_inv: AddLightningInvoiceResponse = address.into();

        // The invoice id is based on the Monero subaddress
        let invoice_id = hex::encode(cln_inv.payment_hash.clone());
        monero_invoice.payment_hash = invoice_id;
        monero_invoice.payment_request = format!("monero:{}?tx_amount={}&tx_description={}", address, monero_invoice.value_msat, monero_invoice.memo);

        // Save the metadata we need later on as serialized data in the wallet
        let label = serde_json::to_string(&monero_invoice).unwrap();
        wallet_rpc.label_address(0, index, label).await.map_err(|_| LightningError)?;

        Ok(cln_inv as _)
    }

    #[cfg(not(unix))]
    async fn add_invoice(
        &self,
        _memo: &str,
        _value_msat: Millisatoshi,
    ) -> Result<AddLightningInvoiceResponse, LightningError> {
        Err(LightningError)
    }

    #[cfg(unix)]
    async fn lookup_invoice(
        &self,
        r_hash: [u8; 32],
    ) -> Result<Option<LightningInvoice>, LightningError> {
        let payment_hash_hex = hex::encode(&r_hash);

        let daemon_client = RpcClient::new(self.inner.clone());
        // let daemon_rpc = daemon_client.daemon_rpc();
        let wallet_rpc = daemon_client.wallet();

        let address = wallet_rpc.get_address(0, None).await;

        let sub_address_data: Vec<SubaddressData> =
            address.unwrap().addresses
                .into_iter()
                .filter(|x| x.label.contains(&payment_hash_hex)).collect();

        let sub_address = sub_address_data.get(0).ok_or_else(|| LightningError);

        let cln_inv: LightningInvoice = sub_address?.clone().into();

        println!("lookup invoice {:?}", cln_inv);

        self.update_payments().await;

        Ok(Some(cln_inv))
    }

    #[cfg(not(unix))]
    async fn lookup_invoice(
        &self,
        _r_hash: [u8; 32],
    ) -> Result<Option<LightningInvoice>, LightningError> {
        Err(LightningError)
    }
}

impl MoneroRpcClient{
    async fn update_payments(&self) {
        let daemon_client = RpcClient::new(self.inner.clone());
        let wallet_rpc = daemon_client.wallet();

        let mut category_selector = HashMap::new();
        category_selector.insert(GetTransfersCategory::In, true);

        let mut transfer_selector = GetTransfersSelector::default();
        transfer_selector.category_selector = category_selector;

        let transfers = wallet_rpc.get_transfers(transfer_selector).await;

        // Lookup invoices for transactions
        for transfer in transfers.unwrap().get(&GetTransfersCategory::In).unwrap().iter() {
            println!("Transfer: {:?}", transfer);
            let address_filter = vec![transfer.subaddr_index.minor];
            let address = wallet_rpc.get_address(0, Some(address_filter)).await;

            let address_tmp = address.unwrap();
            let sub_address = address_tmp.addresses.get(0).ok_or_else(|| LightningError);
            let mut cln_inv: LightningInvoice = sub_address.clone().unwrap().clone().into();
            println!("Invoice: {:?}", cln_inv);

            if transfer.double_spend_seen == false && transfer.amount > cln_inv.value_msat.value() {
                let mut monero_invoice: MoneroInvoice = serde_json::from_str(&sub_address.unwrap().label).unwrap();
                monero_invoice.is_settled = true;
                // Save the metadata we need later on as serialized data in the wallet
                let label = serde_json::to_string(&monero_invoice).unwrap();
                let bla = wallet_rpc.label_address(0, transfer.subaddr_index.minor, label).await.map_err(|_| LightningError);
            }
        }


    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoneroInvoice {
    pub value_msat: u64,
    pub is_settled: bool,
    pub memo: std::string::String,
    pub payment_hash: String,
    pub payment_request: std::string::String,
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
}
