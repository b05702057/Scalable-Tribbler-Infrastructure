use crate::lab1::lab::new_client;
use crate::lab2::bin_client::BinStorageClient;
use crate::lab2::front::FrontendServer;

use std::cmp;
use std::string::String;
use std::thread;
use std::time;
use tribbler::{config::KeeperConfig, err::TribResult, storage::BinStorage, trib::Server};

/// This function accepts a list of backend addresses, and returns a type which
/// should implement the [BinStorage] trait to access the underlying storage system.
#[allow(unused_variables)]
pub async fn new_bin_client(backs: Vec<String>) -> TribResult<Box<dyn BinStorage>> {
    let mut http_backs = Vec::<String>::new();
    for back in backs {
        http_backs.push("http://".to_owned() + &back);
    }
    return Ok(Box::new(BinStorageClient { backs: http_backs })); // We don't have to write "backs : backs" since they have the same name.
}

/// this async function accepts a [KeeperConfig] that should be used to start
/// a new keeper server on the address given in the config.
///
/// This function should block indefinitely and only return upon erroring. Make
/// sure to send the proper signal to the channel in `kc` when the keeper has
/// started.
// #[tokio::main]
#[allow(unused_variables)]
pub async fn serve_keeper(kc: KeeperConfig) -> TribResult<()> {
    let mut clock = 0;
    let backs = kc.backs;
    let back_num = backs.len();
    let mut id = 0;
    let one_sec = time::Duration::from_secs(1);

    // send true when the keeper is ready
    let _ = match kc.ready {
        Some(unwrapped_ready) => unwrapped_ready.send(true),
        None => Ok(()),
    };

    let handle1 = tokio::spawn(async move {
        while clock <= u64::MAX {
            // get the max clock from the storages
            while id < back_num {
                let client = new_client(&backs[id]).await.unwrap();
                clock = cmp::max(clock, client.clock(clock).await.unwrap());
                id += 1; // next storage
            }

            // set all clocks to the max clock
            id = 0;
            while id < back_num {
                let client = new_client(&backs[id]).await.unwrap();
                clock = cmp::max(clock, client.clock(clock).await.unwrap());
                id += 1; // next storage
            }

            // prepare for the next synchornization
            thread::sleep(one_sec); // sleep for one second
            id = 0;
        }
    });

    let handle2 = tokio::spawn(async move {
        match kc.shutdown {
            None => {
                let result = handle1.await;
                println!("{:?}", result);
            }
            Some(mut receiver) => match receiver.recv().await {
                None => {
                    let result = handle1.await;
                    println!("{:?}", result);
                }
                Some(_) => {
                    handle1.abort();
                }
            },
        }
    });

    let result = handle2.await;
    println!("{:?}", result);
    return Ok(());
}

/// this function accepts a [BinStorage] client which should be used in order to
/// implement the [Server] trait.
///
/// You'll need to translate calls from the tribbler front-end into storage
/// calls using the [BinStorage] interface.
///
/// Additionally, two trait bounds [Send] and [Sync] are required of your
/// implementation. This should guarantee your front-end is safe to use in the
/// tribbler front-end service launched by the`trib-front` command
#[allow(unused_variables)]
pub async fn new_front(
    bin_storage: Box<dyn BinStorage>,
) -> TribResult<Box<dyn Server + Send + Sync>> {
    return Ok(Box::new(FrontendServer { bin_storage }));
}

// Questions
// 1. write concurrent (un)follow test cases in front_trib
