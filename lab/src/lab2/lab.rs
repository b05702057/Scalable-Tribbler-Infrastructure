use crate::lab1::lab::new_client;
use crate::lab2::bin_client::BinStorageClient;
use crate::lab2::front::FrontendServer;

use std::cmp;
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

    // check the channel
    match kc.shutdown {
        None => {
            while clock <= u64::MAX {
                // get the max clock from the storages
                while id < back_num {
                    let client = new_client(&backs[id]).await?;
                    clock = cmp::max(clock, client.clock(clock).await?);
                    id += 1; // next storage
                }

                // set all clocks to the max clock
                id = 0;
                while id < back_num {
                    let client = new_client(&backs[id]).await?;
                    clock = cmp::max(clock, client.clock(clock).await?);
                    id += 1; // next storage
                }

                // prepare for the next synchornization
                thread::sleep(one_sec); // sleep for one second
                id = 0;
            }
        }
        Some(mut receiver) => {
            while clock <= u64::MAX {
                // get the max clock from the storages
                while id < back_num {
                    let client = new_client(&backs[id]).await?;
                    clock = cmp::max(clock, client.clock(clock).await?);
                    id += 1; // next storage
                }

                // set all clocks to the max clock
                id = 0;
                while id < back_num {
                    let client = new_client(&backs[id]).await?;
                    clock = cmp::max(clock, client.clock(clock).await?);
                    id += 1; // next storage
                }

                // check the receiver
                match receiver.recv().await {
                    None => {
                        ();
                    }
                    Some(_) => {
                        break;
                    }
                }

                // prepare for the next synchornization
                thread::sleep(one_sec); // sleep for one second
                id = 0;
            }
        }
    }
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
// 1. When are keepers ready, and do the initialization fail?
// 4. check the log logic
// 6. write concurrent (un)follow test cases
// 7. modify new_bin_client with http?
// 8. building hint 1 (signup, follow) and 5 => crash?
