use clap::builder::Str;
use reth_db::RawTable;
use reth_db::transaction::DbTx;
use reth_db::{ClientVersion, cursor::DbCursorRO, mdbx::DatabaseArguments, open_db_read_only};
use reth_db::{Database, cursor};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about = "Utilitary program to snapshot Ethereum state in reth client.", long_about = None)]
struct Args {
    /// Reth database path to read the state from
    #[arg(short, long)]
    database_path: String,
}

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct Contract {
    address: String,
    code: String,
    storage: Vec<(String, String)>,
}

fn main() {
    println!(
        "
 ▄████▄   ██░ ██  ▄▄▄       ▒█████    ██████ 
▒██▀ ▀█  ▓██░ ██▒▒████▄    ▒██▒  ██▒▒██    ▒ 
▒▓█    ▄ ▒██▀▀██░▒██  ▀█▄  ▒██░  ██▒░ ▓██▄   
▒▓▓▄ ▄██▒░▓█ ░██ ░██▄▄▄▄██ ▒██   ██░  ▒   ██▒
▒ ▓███▀ ░░▓█▒░██▓ ▓█   ▓██▒░ ████▓▒░▒██████▒▒
░ ░▒ ▒  ░ ▒ ░░▒░▒ ▒▒   ▓▒█░░ ▒░▒░▒░ ▒ ▒▓▒ ▒ ░
  ░  ▒    ▒ ░▒░ ░  ▒   ▒▒ ░  ░ ▒ ▒░ ░ ░▒  ░ ░
░         ░  ░░ ░  ░   ▒   ░ ░ ░ ▒  ░  ░  ░  
░ ░       ░  ░  ░      ░  ░    ░ ░        ░  
░                                            
    "
    );

    let args = Args::parse();

    let path = Path::new(&args.database_path);
    let database_args = DatabaseArguments::new(ClientVersion::default());

    let db = open_db_read_only(path, database_args).unwrap();

    let mut state: HashSet<Contract> = HashSet::new();

    let _ = db.view(|tx| {
        tx.inner.disable_timeout(); // Disable timeout to be sure to read the table entirely

        let table_db = tx.inner.open_db(Some("Bytecodes")).unwrap();

        let stats = tx.inner.db_stat(&table_db).unwrap();
        println!("Number of contracts to backup : {}", stats.entries());

        let mut cursor_bytecodes = tx.cursor_read::<RawTable<reth_db::Bytecodes>>().unwrap();

        cursor_bytecodes.walk(None).unwrap().for_each(|result| {
            let value = result.unwrap();

            let bytecode_hash = value.0.raw_key();
            println!("bytecode hash {}", hex::encode(bytecode_hash));
            let bytecodes = value.1.value().unwrap().bytecode().to_string();

            let mut contract_address = String::new();
            let mut storage: Vec<(String, String)> = vec![];

            let mut cursor_accounts = tx
                .cursor_read::<RawTable<reth_db::PlainAccountState>>()
                .unwrap();
            cursor_accounts.walk(None).unwrap().for_each(|result| {
                let value = result.unwrap();

                if bytecode_hash == &value.1.value().unwrap().get_bytecode_hash().0.to_vec() {
                    contract_address = value.0.key().unwrap().to_string();
                }
            });

            let mut cursor_storage = tx
                .cursor_read::<RawTable<reth_db::PlainStorageState>>()
                .unwrap();
            cursor_storage.walk(None).unwrap().for_each(|result| {
                let value = result.unwrap();

                if contract_address == value.0.key().unwrap().to_string() {
                    let storage_entry = value.1.value().unwrap();
                    storage.push((
                        storage_entry.key.to_string(),
                        storage_entry.value.to_string(),
                    ));
                }
            });

            state.insert(Contract {
                address: contract_address,
                code: bytecodes,
                storage,
            });
        });
    });

    println!("Saving into state.json");
    let mut file = File::create("state.json").unwrap();
    file.write_all(serde_json::to_string(&state).unwrap().as_bytes())
        .unwrap();
}
