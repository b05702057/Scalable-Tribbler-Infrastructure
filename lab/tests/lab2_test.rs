use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
    },
};
use std::time::Duration;

use lab::{self, lab1, lab2};
use tokio::{sync::mpsc::Sender as MpscSender, task::JoinHandle};

use tribbler::{config::KeeperConfig, trib::{MAX_TRIB_LEN, MAX_TRIB_FETCH}, storage::List};
#[allow(unused_imports)]
use tribbler::{
    self,
    config::BackConfig,
    err::{TribResult, TribblerError},
    storage::{KeyList, KeyString, KeyValue, MemStorage, Pattern, Storage},
};

const DEFAULT_KEEPER: &str = "localhost:32243";
const DEFAULT_ADDR: &str = "localhost";
const DEFAULT_PORT: u32 = 32244;

async fn setup_n(s: u32) -> TribResult<(Vec<String>, Vec<JoinHandle<TribResult<()>>>, Vec<tokio::sync::mpsc::Sender<()>>, JoinHandle<TribResult<()>>, MpscSender<()>)> {
    let mut backs = Vec::new();
    let mut handles = Vec::new();
    let mut back_shutdowns = Vec::new();

    // Setup Backs
    for i in 0..s {
        let back = format!("{}:{}", DEFAULT_ADDR, (DEFAULT_PORT + i));
        backs.push(back.clone());

        let storage = Box::new(MemStorage::new());
        let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        let (shut_tx, shut_rx) = tokio::sync::mpsc::channel(1);
        let cfg = BackConfig {
            addr: back.clone(),
            storage: storage,
            ready: Some(tx.clone()),
            shutdown: Some(shut_rx),
        };

        let handle = spawn_back(cfg);
        handles.push(handle);
        back_shutdowns.push(shut_tx.clone());
        let ready = rx.recv_timeout(Duration::from_secs(5))?;
        if !ready {
            return Err(Box::new(TribblerError::Unknown(
                "back failed to start".to_string(),
            )));
        }
    }

    // Setup Keeper
    let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let (shut_tx, shut_rx) = tokio::sync::mpsc::channel(1);
    let cfg_keeper = KeeperConfig {
        backs: backs.clone(),
        addrs: vec![DEFAULT_KEEPER.to_string()],
        this: 0 as usize,
        id: 0 as u128,
        ready: Some(tx.clone()),
        shutdown: Some(shut_rx),
    };

    let keeper_handle = tokio::spawn(lab2::serve_keeper(cfg_keeper));
    let ready = rx.recv_timeout(Duration::from_secs(5))?;
    if !ready {
        return Err(Box::new(TribblerError::Unknown(
            "back failed to start".to_string(),
        )));
    }

    Ok((backs, handles, back_shutdowns, keeper_handle, shut_tx.clone()))
}

fn spawn_back(cfg: BackConfig) -> tokio::task::JoinHandle<TribResult<()>> {
    tokio::spawn(lab1::serve_back(cfg))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_keeper_shutdown() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(1).await?;

    let _ = shutdown_keeper.send(()).await;
    let r = keeper_handle.await.unwrap();
    assert!(r.is_ok());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_teardown() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;

    for s in shutdown_backs {
        let _ = s.send(()).await?;
    }

    for b in backs {
        let r = b.await.unwrap();
        assert!(r.is_ok());
    }

    let _ = shutdown_keeper.send(()).await;
    let r = keeper_handle.await.unwrap();
    assert!(r.is_ok());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_signup() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Invalid Name
    let r = tribserver.sign_up("").await;
    assert!(r.is_err());

    // Add Bob
    let expected_users = vec!["bob".to_string()];
    let r = tribserver.sign_up("bob").await;
    assert!(r.is_ok());
    let users = tribserver.list_users().await?;
    assert_eq!(expected_users, users);

    // Add Bob again
    let r = tribserver.sign_up("bob").await;
    assert!(r.is_err());

    // Add Alice
    let expected_users = vec!["alice".to_string(), "bob".to_string()];
    let r = tribserver.sign_up("alice").await;
    assert!(r.is_ok());
    let users = tribserver.list_users().await?;
    assert_eq!(expected_users, users);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_list_users() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    let names = vec!["z", "y", "x", "w", "v", "u", "t", "s", "r", "q", "p", "o", "n", "m", "l", "k", "j", "i", "h", "g", "f", "e", "d", "c", "b", "a"];
    let names_string:Vec<String> = names.iter().map(|ch| ch.to_string()).collect();

    for n in names {
        tribserver.sign_up(&n).await?;
    }

    let mut expected = names_string[0..20].to_vec();
    expected.sort();

    let users = tribserver.list_users().await?;
    assert_eq!(users, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_post() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Post too long
    let too_long = (0..(MAX_TRIB_LEN+1)).map(|_| "X").collect::<String>();
    let r = tribserver.post("bob", &too_long, 0).await;
    assert!(r.is_err());

    let valid_post = (0..(MAX_TRIB_LEN)).map(|_| "X").collect::<String>();

    // User doesn't exist
    let r = tribserver.post("bob", &valid_post, 0).await;
    assert!(r.is_err());

    let _ = tribserver.sign_up("bob").await?;

    let r = tribserver.post("bob", &valid_post, 0).await;
    assert!(r.is_ok());

    // garbage collection
    for i in 0..(MAX_TRIB_FETCH+50) {
        let post = i.to_string();
        let _ = tribserver.post("bob", &post, 0).await?;
    }
    tribserver.tribs("bob").await?; // garbage collection

    let bin_storage_2 = lab2::new_bin_client(back_addrs.clone()).await?;
    let bin = bin_storage_2.bin("bob").await?;
    let List(serialized_tribs) = bin.list_get("tribs").await?;
    assert_eq!(serialized_tribs.len(), MAX_TRIB_FETCH);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_tribs() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    let _ = tribserver.sign_up("bob").await?;

    // fetch 2 tribs
    let post = "a";
    let _ = tribserver.post("bob", &post, 0).await?;
    let _ = tribserver.post("bob", &post, 0).await?;
    let tribs = tribserver.tribs("bob").await?;
    assert_eq!(tribs.len(), 2);

    // fetch 100 tribs
    for i in 0..MAX_TRIB_FETCH {
        let post = "b";
        let _ = tribserver.post("bob", &post, 0).await?;
    }
    let tribs = tribserver.tribs("bob").await?;
    assert_eq!(tribs.len(), 100);
    // shouldn't be one of the initial 2 posts
    for t in tribs {
        assert_eq!(t.message, "b");
    }

    // garbage collection
    let bin_storage_2 = lab2::new_bin_client(back_addrs.clone()).await?;
    let bin = bin_storage_2.bin("bob").await?;
    let List(serialized_tribs) = bin.list_get("tribs").await?;
    assert_eq!(serialized_tribs.len(), MAX_TRIB_FETCH);
    
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_follow() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Bob doesn't exist
    let r = tribserver.follow("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("bob").await?;

    // Alice doesn't exist
    let r = tribserver.follow("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("alice").await?;

    // Bob can't follow Bob
    let r = tribserver.follow("bob", "bob").await;
    assert!(r.is_err());

    // Bob follows Alice
    let r = tribserver.follow("bob", "alice").await;
    assert!(r.is_ok());

    // Bob can't follow Alice again
    let r = tribserver.follow("bob", "alice").await;
    assert!(r.is_err());

    for i in 0..1999 {
        let name = format!("alice{}", i);
        let _ = tribserver.sign_up(&name).await?;
        let _ = tribserver.follow("bob", &name).await?;
    }

    // Too many follows
    let _ = tribserver.sign_up("malory").await?;
    let r = tribserver.follow("bob", "malory").await;
    assert!(r.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_unfollow() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Bob doesn't exist
    let r = tribserver.unfollow("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("bob").await?;

    // Alice doesn't exist
    let r = tribserver.unfollow("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("alice").await?;

    // Bob can't unfollow Bob
    let r = tribserver.unfollow("bob", "bob").await;
    assert!(r.is_err());

    // Bob can't unfollow Alice yet
    let r = tribserver.unfollow("bob", "alice").await;
    assert!(r.is_err());

    // Bob can unfollow Alice after following
    let _ = tribserver.follow("bob", "alice").await?;
    let r = tribserver.unfollow("bob", "alice").await;
    assert!(r.is_ok());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_is_following() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Bob doesn't exist
    let r = tribserver.is_following("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("bob").await?;

    // Alice doesn't exist
    let r = tribserver.is_following("bob", "alice").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("alice").await?;

    // Bob can't follow Bob
    let r = tribserver.is_following("bob", "bob").await;
    assert!(r.is_err());

    // Bob isn't following Alice
    let r = tribserver.is_following("bob", "alice").await?;
    assert!(!r);

    // Bob follows Alice
    let _ = tribserver.follow("bob", "alice").await?;
    let r = tribserver.is_following("bob", "alice").await?;
    assert!(r);

    // Bob no longer follows Alice
    let _ = tribserver.unfollow("bob", "alice").await?;
    let r = tribserver.is_following("bob", "alice").await?;
    assert!(!r);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_following() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    // Bob doesn't exist
    let r = tribserver.following("bob").await;
    assert!(r.is_err());
    let _ = tribserver.sign_up("bob").await?;

    // Bob following no one
    let r = tribserver.following("bob").await?;
    assert_eq!(r.len(), 0);

    // Bob following Alice
    let _ = tribserver.sign_up("alice").await?;
    let r = tribserver.follow("bob", "alice").await?;
    let r = tribserver.following("bob").await?;
    let expected = vec!["alice".to_string()];
    assert_eq!(r, expected);

    // Bob no longer following Alice
    let r = tribserver.unfollow("bob", "alice").await?;
    let r = tribserver.following("bob").await?;
    assert_eq!(r.len(), 0);

    // Bob following 2000 users
    for i in 0..2000 {
        let name = format!("alice{}", i);
        let _ = tribserver.sign_up(&name).await?;
        let _ = tribserver.follow("bob", &name).await?;
    }

    let r = tribserver.following("bob").await?;
    assert_eq!(r.len(), 2000);

    // Bob unfollow 2000 users
    for i in 0..2000 {
        let name = format!("alice{}", i);
        let _ = tribserver.unfollow("bob", &name).await?;
    }

    let r = tribserver.following("bob").await?;
    assert_eq!(r.len(), 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(unused_variables)]
async fn test_home() -> TribResult<()> {
    let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
    let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
    let tribserver = lab2::new_front(bin_storage).await?;

    let mut names = Vec::new();

    // Bob doesn't exist
    let r = tribserver.following("bob").await;
    assert!(r.is_err());

    let _ = tribserver.sign_up("bob").await?;
    names.push("bob".to_string());

    // Should have no tribs
    let home = tribserver.home("bob").await?;
    assert_eq!(home.len(), 0);

    // Create 4 other users that bob follows
    for i in 0..4 {
        let name = format!("alice{}", i);
        let _ = tribserver.sign_up(&name).await?;
        let _ = tribserver.follow("bob", &name).await?;
        names.push(name);
    }

    for i in 0..names.len() {
        let name = &names[i];
        for c in 0..(i+1) {
            let _ = tribserver.post(name, "post", 0).await?;
        }
    }

    // Should pull in 15
    let home = tribserver.home("bob").await?;
    assert_eq!(home.len(), 15);

    for i in 0..names.len() {
        let name = &names[i];
        for c in 0..20 {
            let _ = tribserver.post(name, "post", 0).await?;
        }
    }

    // Should pull in 100
    let home = tribserver.home("bob").await?;
    assert_eq!(home.len(), 100);

    Ok(())
}

// #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
// #[allow(unused_variables)]
// async fn test_concurrent_follow() -> TribResult<()> {
//     let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await?;
//     let bin_storage = lab2::new_bin_client(back_addrs.clone()).await?;
//     let tribserver = lab2::new_front(bin_storage).await?;
//     let _ = tribserver.sign_up("bob").await?;
//     let _ = tribserver.sign_up("alice").await?;

//     let mut counter = 0;
//     let mut handles: Vec<JoinHandle<()>> = Vec::new();
//     while (counter < 2) {
//         let temp = tokio::spawn(async move{
//             let (back_addrs, backs, shutdown_backs, keeper_handle, shutdown_keeper) = setup_n(3).await.unwrap();
//             let bin_storage = lab2::new_bin_client(back_addrs.clone()).await.unwrap();
//             let tribserver = lab2::new_front(bin_storage).await.unwrap();
//             let r = tribserver.follow("bob", "alice").await.unwrap();
//         });

//         counter += 1;
//         handles.push(temp);
//     }
//     let mut result_counter = 0;

//     for item in handles {
//         let temp = item.await;
//         dbg!(&temp);
//         if temp.is_ok() {
//             result_counter += 1;
//         }
//     }

//     assert_eq!(result_counter, 1);

//     return Ok(());
// }
