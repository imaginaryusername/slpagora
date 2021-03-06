pub mod message_header;
pub mod message;
pub mod message_error;
pub mod version_message;
pub mod hash;
pub mod serialize;
pub mod tx;
pub mod incomplete_tx;
pub mod script;
pub mod script_interpreter;
pub mod address;
pub mod outputs;
pub mod wallet;
pub mod trade;

use std::io::{self, Write, Read};
use text_io::{read, try_read, try_scan};
use std::env;


const WALLET_FILE_NAME: &str = "trade.dat";


fn ensure_wallet_interactive() -> Result<wallet::Wallet, Box<std::error::Error>> {
    match std::fs::File::open(WALLET_FILE_NAME) {
        Ok(mut file) => {
            let mut secret_bytes = [0; 32];
            file.read(&mut secret_bytes)?;
            Ok(wallet::Wallet::from_secret(&secret_bytes)?)
        },
        Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
            println!("There's currently no wallet created. Press ENTER to create one at the \
                      current working directory ({}/{}) or enter the path to the wallet file: ",
                     env::current_dir()?.display(),
                     WALLET_FILE_NAME);
            io::stdout().flush()?;
            let wallet_file_path: String = read!("{}\n");
            let wallet_file_path =
                if wallet_file_path.len() != 0 { &wallet_file_path }
                else {WALLET_FILE_NAME};
            use rand::RngCore;
            let mut rng = rand::rngs::OsRng::new().unwrap();
            let mut secret_bytes = [0; 32];
            rng.fill_bytes(&mut secret_bytes);
            let _ = secp256k1::SecretKey::from_slice(&secret_bytes)?;
            std::fs::File::create(wallet_file_path)?.write(&secret_bytes)?;
            Ok(wallet::Wallet::from_secret(&secret_bytes)?)
        },
        err => {err?; unreachable!()},
    }
}

pub fn show_qr(s: &str) {
    use std::process::Command;
    println!("{}", String::from_utf8(
        Command::new("python3")
            .args(&[
                "-c",
                &format!(
                    "import pyqrcode; print(pyqrcode.create('{}').terminal())",
                    s,
                ),
            ])
            .output()
            .unwrap().stdout
    ).unwrap());
}

fn show_balance(w: &wallet::Wallet) {
    let balance = w.get_balance();
    println!("Your wallet's balance is: {} sats or {} BCH.",
             balance,
             balance as f64 / 100_000_000.0);
}

fn do_transaction(w: &wallet::Wallet) -> Result<(), Box<std::error::Error>> {
    let (mut tx_build, balance) = w.init_transaction();
    println!("Your wallet's balance is: {} sats or {} BCH.",
             balance,
             balance as f64 / 100_000_000.0);
    print!("Enter the address to send to: ");
    io::stdout().flush()?;
    let addr_str: String = read!("{}\n");
    let receiving_addr = match address::Address::from_cash_addr(addr_str)  {
        Ok(addr) => addr,
        Err(err) => {
            println!("Please enter a valid address: {:?}", err);
            return Ok(());
        }
    };
    if receiving_addr.prefix() == "simpleledger" {
        println!("Note: You entered a Simple Ledger Protocol (SLP) address, but this wallet only \
                  contains ordinary non-token BCH. The program will proceed anyways.");
    }
    print!("Enter the amount in satoshis to send, or \"all\" (without quotes) to send the entire \
            balance: ");
    io::stdout().flush()?;
    let send_amount_str: String = read!("{}\n");
    let send_amount = if send_amount_str.as_str() == "all" {
        balance
    } else {
        send_amount_str.parse::<u64>()?
    };
    tx_build.add_output(&outputs::P2PKHOutput {
        value: send_amount,
        address: receiving_addr,
    });
    let mut output_back_to_wallet = outputs::P2PKHOutput {
        value: 0,
        address: w.address().clone(),
    };
    let back_to_wallet_idx = tx_build.add_output(&output_back_to_wallet);
    let estimated_size = tx_build.estimate_size();
    let send_back_to_wallet_amount = balance - (send_amount + estimated_size + 5);
    if send_back_to_wallet_amount < w.dust_amount() {
        tx_build.remove_output(back_to_wallet_idx);
    } else {
        output_back_to_wallet.value = send_back_to_wallet_amount;
        tx_build.replace_output(back_to_wallet_idx, &output_back_to_wallet);
    }
    let tx = tx_build.sign();
    w.send_tx(&tx)?;

    Ok(())
}

fn main() -> Result<(), Box<std::error::Error>> {
    let wallet = ensure_wallet_interactive()?;
    println!("Your wallet address is: {}", wallet.address().cash_addr());

    println!("Select an option from below:");
    println!("1: Show wallet balance");
    println!("2: Send BCH from this wallet to an address");
    println!("3: Create a new trade for a token on the BCH blockchain");
    println!("4: List all available token trades on the BCH blockchain");
    println!("Anything else: Exit");
    print!("Your choice: ");
    io::stdout().flush()?;
    let wallet_file_path: String = read!("{}\n");
    match wallet_file_path.as_str() {
        "1" => show_balance(&wallet),
        "2" => do_transaction(&wallet)?,
        "3" => trade::create_trade_interactive(&wallet)?,
        "4" => trade::accept_trades_interactive(&wallet)?,
        _ => println!("Bye, have a great time!"),
    }
    Ok(())
}
