use async_trait::async_trait;
use serde_json;
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::SystemTime;
use tribbler::{
    self,
    err::{TribResult, TribblerError},
    storage::{BinStorage, KeyValue},
    trib::{
        is_valid_username, Server, Trib, MAX_FOLLOWING, MAX_TRIB_FETCH, MAX_TRIB_LEN, MIN_LIST_USER,
    },
};

pub struct FrontendServer {
    pub bin_storage: Box<dyn BinStorage>,
}

// We haven't split the storage yet!!!!!
#[async_trait]
impl Server for FrontendServer {
    async fn sign_up(&self, user: &str) -> TribResult<()> {
        println!("sign_up input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }
        let user_bin = self.bin_storage.bin(user).await?; // get the storage of the user
        let following = user_bin.get(user).await?; // ex. "Ben::Ben"
        match following {
            // ex. "Ben::Ben" => "F" because a user is never its own follower
            None => {
                let _ = user_bin
                    .set(&KeyValue {
                        key: user.to_string(),
                        value: "F".to_string(),
                    })
                    .await?;

                // ex. "::users" => ["Ben", "Alice", ...]
                let general_bin = self.bin_storage.bin("").await?; // get the general bin
                let user_list = general_bin.list_get("users").await?; // get the list of all users

                // need more users for list_users()
                if user_list.0.len() < MIN_LIST_USER {
                    let _ = general_bin
                        .list_append(&KeyValue {
                            key: "users".to_string(),
                            value: user.to_string(), // We want to show the unescaped users to clients.
                        })
                        .await?;
                }
                return Ok(());
            }
            // existing user
            Some(_) => {
                return Err(Box::new(TribblerError::UsernameTaken(user.to_string())));
            }
        }
    }

    async fn list_users(&self) -> TribResult<Vec<String>> {
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let mut user_list = general_bin.list_get("users").await?; // get the list of users
        user_list.0.sort(); // sort in alphabetical order
        println!("list_users output: {:?}", user_list.0);
        return Ok(user_list.0);
    }

    async fn post(&self, who: &str, post: &str, clock: u64) -> TribResult<()> {
        println!("post input: {}", who);
        println!("post input: {}", post);
        println!("post input: {}", clock);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }

        // The post is too long.
        if post.len() > MAX_TRIB_LEN {
            return Err(Box::new(TribblerError::TribTooLong));
        }

        let who_bin = self.bin_storage.bin(who).await?;
        let following = who_bin.get(who).await?;
        match following {
            None => {
                // The user has never signed up.
                return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
            }
            Some(_) => {
                // get the clock from the storage
                let storage_clock = who_bin.clock(clock).await?;

                // create the trib
                let trib = Trib {
                    user: who.to_string(),
                    message: post.to_string(),
                    time: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)?
                        .as_secs(),
                    clock: storage_clock,
                };
                let trib_string = serde_json::to_string(&trib)?;

                // store as the user's posted trib
                let _ = who_bin
                    .list_append(&KeyValue {
                        key: "tribs".to_string(),
                        value: trib_string,
                    })
                    .await?;
                return Ok(());
            }
        }
    }

    async fn tribs(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        println!("tribs input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }

        let user_bin = self.bin_storage.bin(user).await?;
        let following = user_bin.get(user).await?; // check if the user is signed up
        match following {
            None => {
                // The user has never signed up.
                return Err(Box::new(TribblerError::UserDoesNotExist(user.to_string())));
            }
            Some(_) => {
                let mut trib_vec = Vec::<Arc<Trib>>::new();
                let tribs = user_bin.list_get("tribs").await?;
                for trib in tribs.0 {
                    let json_trib = serde_json::from_str(&trib)?;
                    trib_vec.push(json_trib);
                }

                // sort the tribbles based on the priority
                trib_vec.sort_by(|a, b| sort_trib(a, b));
                let trib_num = trib_vec.len();

                // garbage collect older tribs
                if trib_num > MAX_TRIB_FETCH {
                    let old_num = trib_num - MAX_TRIB_FETCH;

                    // The tribs with less clock values are older.
                    for i in 0..old_num {
                        let old_trib = &trib_vec[i];
                        let old_trib_string = serde_json::to_string(&old_trib)?;
                        user_bin
                            .list_remove(&KeyValue {
                                key: "tribs".to_string(),
                                value: old_trib_string,
                            })
                            .await?;
                    }
                    trib_vec = trib_vec[old_num..].to_vec();
                }
                println!("tribs output: {:?}", trib_vec);
                return Ok(trib_vec);
            }
        }
    }

    async fn follow(&self, who: &str, whom: &str) -> TribResult<()> {
        println!("follow input: {}", who);
        println!("follow input: {}", whom);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }

        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        let who_bin = self.bin_storage.bin(who).await?;
        let following = who_bin.get(who).await?; // check if the user is signed up
        match following {
            None => {
                // The follower doesn't exist.
                return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
            }
            Some(_) => {
                let whom_bin = self.bin_storage.bin(whom).await?;
                let following = whom_bin.get(whom).await?;
                match following {
                    None => {
                        // The followee doesn't exist.
                        return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
                    }
                    Some(_) => {
                        // cannot follow too many people
                        let followees = who_bin.list_get("followees").await?;
                        if followees.0.len() >= MAX_FOLLOWING {
                            return Err(Box::new(TribblerError::FollowingTooMany));
                        }

                        let storage_clock = who_bin.clock(0).await?;
                        let log_entry = storage_clock.to_string() + "::follow::" + whom;

                        // append the log entry
                        who_bin
                            .list_append(&KeyValue {
                                key: "log".to_string(),
                                value: log_entry,
                            })
                            .await?;

                        // check the log entry
                        let mut follow_state = 0;
                        let log = who_bin.list_get("log").await?;
                        for log_entry in log.0 {
                            let mut string_iterator = log_entry.split("::");
                            let parsed_clock = string_iterator.next();
                            let parsed_follow_string = string_iterator.next();
                            let parsed_followee = string_iterator.next();
                            match parsed_followee {
                                None => {
                                    return Err(Box::new(TribblerError::Unknown(
                                        "unexisting followee".to_string(),
                                    )));
                                }
                                Some(followee_name) => {
                                    if followee_name == whom {
                                        match parsed_follow_string {
                                            Some("follow") => {
                                                match parsed_clock {
                                                    None => {
                                                        return Err(Box::new(
                                                            TribblerError::Unknown(
                                                                "unexisting identifier".to_string(),
                                                            ),
                                                        ));
                                                    }
                                                    Some(log_clock) => {
                                                        // same unique identifier
                                                        if storage_clock.to_string() == log_clock {
                                                            if follow_state == 1 {
                                                                return Err(Box::new(
                                                                    TribblerError::AlreadyFollowing(
                                                                        who.to_string(),
                                                                        whom.to_string(),
                                                                    ),
                                                                ));
                                                            } else {
                                                                break;
                                                            }
                                                        } else {
                                                            follow_state = 1;
                                                        }
                                                    }
                                                }
                                            }
                                            Some("unfollow") => {
                                                follow_state = 0;
                                            }
                                            _ => {
                                                // None or Some(_)
                                                return Err(Box::new(TribblerError::Unknown(
                                                    "unknown follow string".to_string(),
                                                )));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // add the followee to the followee list
                        who_bin
                            .list_append(&KeyValue {
                                key: "followees".to_string(),
                                value: whom.to_string(),
                            })
                            .await?;

                        // set value in key string storage, ex. "Alice::Ben" => True
                        let _ = who_bin
                            .set(&KeyValue {
                                key: whom.to_string(),
                                value: "T".to_string(),
                            })
                            .await?;
                    }
                }
            }
        }
        return Ok(());
    }

    async fn unfollow(&self, who: &str, whom: &str) -> TribResult<()> {
        println!("unfollow input: {}", who);
        println!("unfollow input: {}", whom);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        let who_bin = self.bin_storage.bin(who).await?;
        let following = who_bin.get(who).await?; // check if the user is signed up
        match following {
            None => {
                // The unfollower doesn't exist.
                return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
            }
            Some(_) => {
                let whom_bin = self.bin_storage.bin(whom).await?;
                let following = whom_bin.get(whom).await?;
                match following {
                    None => {
                        // The unfollowee doesn't exist.
                        return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
                    }
                    Some(_) => {
                        let storage_clock = who_bin.clock(0).await?;
                        let log_entry = storage_clock.to_string() + "::unfollow::" + whom;

                        // append the log entry
                        who_bin
                            .list_append(&KeyValue {
                                key: "log".to_string(),
                                value: log_entry,
                            })
                            .await?;

                        // check the log entry
                        let mut follow_state = 0;
                        let log = who_bin.list_get("log").await?;
                        for log_entry in log.0 {
                            let mut string_iterator = log_entry.split("::");
                            let parsed_clock = string_iterator.next();
                            let parsed_follow_string = string_iterator.next();
                            let parsed_followee = string_iterator.next();
                            match parsed_followee {
                                None => {
                                    return Err(Box::new(TribblerError::Unknown(
                                        "unexisting followee".to_string(),
                                    )));
                                }
                                Some(followee_name) => {
                                    if followee_name == whom {
                                        match parsed_follow_string {
                                            Some("follow") => {
                                                follow_state = 1;
                                            }
                                            Some("unfollow") => {
                                                match parsed_clock {
                                                    None => {
                                                        return Err(Box::new(
                                                            TribblerError::Unknown(
                                                                "unexisting identifier".to_string(),
                                                            ),
                                                        ));
                                                    }
                                                    Some(log_clock) => {
                                                        // same unique identifier
                                                        if storage_clock.to_string() == log_clock {
                                                            if follow_state == 0 {
                                                                return Err(Box::new(
                                                                    TribblerError::NotFollowing(
                                                                        who.to_string(),
                                                                        whom.to_string(),
                                                                    ),
                                                                ));
                                                            } else {
                                                                break;
                                                            }
                                                        } else {
                                                            follow_state = 0;
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {
                                                // None or Some(_)
                                                return Err(Box::new(TribblerError::Unknown(
                                                    "unknown follow string".to_string(),
                                                )));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // remove the followee from the followee list
                        who_bin
                            .list_remove(&KeyValue {
                                key: "followees".to_string(),
                                value: whom.to_string(),
                            })
                            .await?;

                        // set value in key string storage, ex. "Alice::Ben" => False
                        let _ = who_bin
                            .set(&KeyValue {
                                key: whom.to_string(),
                                value: "F".to_string(),
                            })
                            .await?;
                    }
                }
            }
        }
        return Ok(());
    }

    async fn is_following(&self, who: &str, whom: &str) -> TribResult<bool> {
        println!("is_follow input: {}", who);
        println!("is_follow input: {}", who);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        let who_bin = self.bin_storage.bin(who).await?;
        let following = who_bin.get(who).await?; // check if the user is signed up
        match following {
            None => {
                // The user doesn't exist.
                return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
            }
            Some(_) => {
                let whom_bin = self.bin_storage.bin(whom).await?;
                let following = whom_bin.get(whom).await?;
                match following {
                    None => {
                        // The user doesn't exist.
                        return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
                    }
                    Some(_) => {
                        let result = who_bin.get(whom).await?;
                        match result {
                            None => {
                                // The follower doesn't know the followee at all, so the return value of get() is none.
                                return Ok(false);
                            }
                            Some(true_false) => {
                                println!("follow input: {}", true_false);
                                if true_false == "T" {
                                    return Ok(true);
                                } else if true_false == "F" {
                                    return Ok(false);
                                } else {
                                    return Err(Box::new(TribblerError::Unknown(
                                        "should only be T or F".to_string(),
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn following(&self, who: &str) -> TribResult<Vec<String>> {
        println!("following input: {}", who);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }

        let who_bin = self.bin_storage.bin(who).await?;
        let following = who_bin.get(who).await?; // check if the user is signed up
        match following {
            None => {
                // The user doesn't exist.
                return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
            }
            Some(_) => {
                let followees = who_bin.list_get("followees").await?;
                println!("following output: {:?}", followees.0);
                return Ok(followees.0);
            }
        }
    }

    async fn home(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        println!("home input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }

        let user_bin = self.bin_storage.bin(user).await?;
        let following = user_bin.get(user).await?; // check if the user is signed up
        match following {
            None => {
                // The user doesn't exist.
                return Err(Box::new(TribblerError::UserDoesNotExist(user.to_string())));
            }
            Some(_) => {
                let mut user_home = Vec::<Arc<Trib>>::new();

                // get the tribs of the user
                let mut user_tribs = self.tribs(user).await?;
                user_home.append(&mut user_tribs);

                let followees = self.following(user).await?;
                for followee in followees {
                    let mut followee_tribs = self.tribs(&followee).await?;
                    user_home.append(&mut followee_tribs);
                }

                // sort the tribbles based on the priority
                user_home.sort_by(|a, b| sort_trib(a, b));

                let trib_num = user_home.len();
                if trib_num > MAX_TRIB_FETCH {
                    let old_num = trib_num - MAX_TRIB_FETCH;
                    user_home = user_home[old_num..].to_vec();
                }
                println!("home output: {:?}", user_home);
                return Ok(user_home);
            }
        }
    }
}

fn sort_trib(a: &Arc<Trib>, b: &Arc<Trib>) -> Ordering {
    if a.clock < b.clock {
        return Ordering::Less;
    } else if a.clock > b.clock {
        return Ordering::Greater;
    } else {
        if a.time < b.time {
            return Ordering::Less;
        } else if a.time > b.time {
            return Ordering::Greater;
        } else {
            if a.user < b.user {
                return Ordering::Less;
            } else if a.user > b.user {
                return Ordering::Greater;
            } else {
                if a.message < b.message {
                    return Ordering::Less;
                } else if a.message > b.message {
                    return Ordering::Greater;
                } else {
                    return Ordering::Equal;
                }
            }
        }
    }
}
