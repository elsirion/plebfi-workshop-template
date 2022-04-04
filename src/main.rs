use bdk::blockchain::{Blockchain, EsploraBlockchain};
use bdk::database::MemoryDatabase;
use bdk::wallet::AddressIndex;
use bdk::{SignOptions, SyncOptions, Wallet};
use bitcoin::{Address, Amount, Network, Txid};
use rocket::form::Form;
use rocket::FromForm;
use rocket::{get, launch, post, routes, State};
use rocket_dyn_templates::Template;
use serde::Serialize;
use std::ops::Deref;
use std::sync::Mutex;

#[derive(Debug)]
struct WalletState {
    wallet: Wallet<MemoryDatabase>,
    blockchain: EsploraBlockchain,
}

#[derive(Serialize)]
struct Index {
    #[serde(with = "bitcoin::util::amount::serde::as_btc")]
    balance: Amount,
    can_spend: bool,
    address: Address,
}

#[derive(Serialize)]
struct Success {
    #[serde(with = "bitcoin::util::amount::serde::as_btc")]
    amount: Amount,
    address: Address,
    txid: Txid,
}

#[get("/")]
fn main_page(stuff: &State<Mutex<WalletState>>) -> Template {
    let guard = stuff.lock().unwrap();
    let WalletState { wallet, blockchain } = guard.deref();
    wallet.sync(blockchain, SyncOptions::default()).unwrap();
    let balance = wallet.get_balance().unwrap();
    let address = wallet.get_address(AddressIndex::New).unwrap();

    let idx_context = Index {
        balance: Amount::from_sat(balance),
        can_spend: balance > 5000,
        address: address.address,
    };

    Template::render("index", idx_context)
}

#[derive(FromForm)]
struct SendForm {
    address: String,
}

#[post("/send", data = "<address>")]
fn send(stuff: &State<Mutex<WalletState>>, address: Form<SendForm>) -> Template {
    let amount = Amount::from_sat(5000);

    let guard = stuff.lock().unwrap();
    let WalletState { wallet, blockchain } = guard.deref();

    let addr: Address = address.address.parse().unwrap();
    assert_eq!(addr.network, Network::Testnet);
    let mut builder = wallet.build_tx();
    builder.add_recipient(addr.script_pubkey(), amount.as_sat());
    let (mut psbt, _) = builder.finish().unwrap();

    assert!(wallet.sign(&mut psbt, SignOptions::default()).unwrap());

    let tx = psbt.extract_tx();
    let txid = tx.txid();

    blockchain.broadcast(&tx).unwrap();

    let success_ctx = Success {
        amount,
        address: addr,
        txid,
    };

    Template::render("done", success_ctx)
}

#[launch]
fn rocket() -> _ {
    let state = Mutex::new(WalletState {
        wallet: Wallet::new(
            "wpkh(cRqV6ruVC4pgobt9Vakfx2vGgpefAMaU3qbcesZ8sccuH5h7Xrqq)",
            None,
            Network::Testnet,
            MemoryDatabase::new(),
        )
        .unwrap(),
        blockchain: EsploraBlockchain::new("https://blockstream.info/testnet/api", 100),
    });
    rocket::build()
        .mount("/", routes![main_page, send])
        .attach(Template::fairing())
        .manage(state)
}
