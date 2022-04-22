use async_trait::async_trait;
use serde_json;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::string::String;
use std::sync::Arc;
use std::time::SystemTime;
use tribbler::{
    self,
    err::{TribResult, TribblerError},
    storage::{BinStorage, KeyValue, Pattern},
    trib::{
        is_valid_username, Server, Trib, MAX_FOLLOWING, MAX_TRIB_FETCH, MAX_TRIB_LEN, MIN_LIST_USER,
    },
};

pub struct FrontendServer {
    pub bin_storage: Box<dyn BinStorage>,
}

#[async_trait]
impl Server for FrontendServer {
    async fn sign_up(&self, user: &str) -> TribResult<()> {
        // println!("sign_up input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }

        // use the general bin to check if the user has signed up
        let general_bin = self.bin_storage.bin("").await?;
        let signup_string = "signup_".to_owned() + user;
        let signed = general_bin.get(&signup_string).await?;
        match signed {
            None => {
                // The user hasn't signed up.
                general_bin
                    .set(&KeyValue {
                        key: signup_string,
                        value: "T".to_string(),
                    })
                    .await?;
                // Two sign_up operations may succeed (allowed in SPEC).
            }
            Some(_) => {
                // The user has already signed up.
                return Err(Box::new(TribblerError::UsernameTaken(user.to_string())));
            }
        }
        return Ok(());
    }

    async fn list_users(&self) -> TribResult<Vec<String>> {
        // The cache is good enough if we remember to store unique elements in it.
        let general_bin = self.bin_storage.bin("").await?;
        let mut user_cache = general_bin.list_get("cache").await?;
        if user_cache.0.len() >= MIN_LIST_USER {
            // println!("use cache!");
            return Ok(user_cache.0);
        }

        // clean the cache
        for user in user_cache.0 {
            general_bin
                .list_remove(&KeyValue {
                    key: "cache".to_string(),
                    value: user,
                })
                .await?;
        }

        // The cache is not good enough => get all keys with the "signup_" prefix
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let user_list = general_bin
            .keys(&Pattern {
                prefix: "signup_".to_string(),
                suffix: "".to_string(),
            })
            .await?;

        let mut user_vec = Vec::<String>::new();
        for user in user_list.0 {
            let user = &user.to_string()[7..];
            user_vec.push(user.to_string()); // strip the "sign_up" prefix
        }

        // sort + dedup to remove duplicates
        user_vec.sort();
        user_vec.dedup();

        // get fewer than 20 users
        let user_num = user_vec.len();
        if user_num > MIN_LIST_USER {
            user_vec = user_vec[..MIN_LIST_USER].to_vec();
        }
        user_vec.sort(); // sort in alphabetical order

        for user in user_vec {
            general_bin
                .list_append(&KeyValue {
                    key: "cache".to_string(),
                    value: user,
                })
                .await?;
        }
        user_cache = general_bin.list_get("cache").await?;
        // println!("list_users output: {:?}", user_cache.0);
        return Ok(user_cache.0);
    }

    async fn post(&self, who: &str, post: &str, clock: u64) -> TribResult<()> {
        // println!("post input: {}", who);
        // println!("post input: {}", post);
        // println!("post input: {}", clock);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if post.len() > MAX_TRIB_LEN {
            // The post is too long.
            return Err(Box::new(TribblerError::TribTooLong));
        }

        // use the general bin to check if the user has signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let signup_string = "signup_".to_owned() + who;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }

        // use the user bin to store his trib
        let who_bin = self.bin_storage.bin(who).await?;
        let storage_clock = who_bin.clock(clock).await?; // get the clock from the storage

        // create the trib
        let trib = Trib {
            user: who.to_string(),
            message: post.to_string(),
            time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            clock: storage_clock,
        };

        // store as the user's posted trib
        let trib_string = serde_json::to_string(&trib)?;
        who_bin
            .list_append(&KeyValue {
                key: "tribs".to_string(),
                value: trib_string,
            })
            .await?;
        return Ok(());
    }

    async fn tribs(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        // println!("tribs input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }

        // use the general bin to check if the user has signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let signup_string = "signup_".to_owned() + user;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(user.to_string())));
        }

        // get the tribs
        let mut trib_vec = Vec::<Arc<Trib>>::new();
        let user_bin = self.bin_storage.bin(user).await?;
        let tribs = user_bin.list_get("tribs").await?;
        for trib in tribs.0 {
            let json_trib = serde_json::from_str(&trib)?;
            trib_vec.push(json_trib);
        }
        trib_vec.sort_by(|a, b| sort_trib(a, b)); // sort the tribbles based on the priority

        // garbage collect older tribs
        let trib_num = trib_vec.len();
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
        // println!("tribs output: {:?}", trib_vec);
        return Ok(trib_vec);
    }

    async fn follow(&self, who: &str, whom: &str) -> TribResult<()> {
        // println!("follow input: {}", who);
        // println!("follow input: {}", whom);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        // use the general bin to check if who and whom have signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let mut signup_string = "signup_".to_owned() + who;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }
        signup_string = "signup_".to_owned() + whom;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
        }

        // The follower cannot follow himself.
        if who == whom {
            return Err(Box::new(TribblerError::Unknown(
                "The follower cannot follow himself.".to_string(),
            )));
        }

        // append the log entry
        let who_bin = self.bin_storage.bin(who).await?;
        let storage_clock = who_bin.clock(0).await?;
        let log_entry = storage_clock.to_string() + "::follow::" + whom;
        who_bin
            .list_append(&KeyValue {
                key: "log".to_string(),
                value: log_entry,
            })
            .await?;

        // check the log entry
        let mut followees = HashSet::new();
        let log = who_bin.list_get("log").await?;
        for log_entry in log.0 {
            let res: Vec<String> = log_entry.split("::").map(|s| s.to_string()).collect();
            let parsed_clock = (&res[0]).to_string(); // unique identifier
            let parsed_follow_string = (&res[1]).to_string(); // follow or unfollow
            let parsed_followee = (&res[2]).to_string(); // followee

            if parsed_follow_string == "unfollow" {
                if followees.contains(&parsed_followee) {
                    followees.remove(&parsed_followee);
                }
            } else {
                if parsed_followee == whom {
                    if parsed_clock.to_string() == storage_clock.to_string() {
                        // this operation
                        if !followees.contains(&parsed_followee) && followees.len() < MAX_FOLLOWING
                        {
                            return Ok(()); // successfully follow whom
                        } else if followees.contains(&parsed_followee) {
                            return Err(Box::new(TribblerError::AlreadyFollowing(
                                who.to_string(),
                                whom.to_string(),
                            )));
                        } else {
                            return Err(Box::new(TribblerError::FollowingTooMany));
                        }
                    } else {
                        // other operations
                        if !followees.contains(&parsed_followee) && followees.len() < MAX_FOLLOWING
                        {
                            followees.insert(parsed_followee);
                        }
                    }
                } else {
                    if !followees.contains(&parsed_followee) && followees.len() < MAX_FOLLOWING {
                        followees.insert(parsed_followee);
                    }
                }
            }
        }
        return Ok(());
    }

    async fn unfollow(&self, who: &str, whom: &str) -> TribResult<()> {
        // println!("unfollow input: {}", who);
        // println!("unfollow input: {}", whom);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        // use the general bin to check if who and whom have signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let mut signup_string = "signup_".to_owned() + who;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }
        signup_string = "signup_".to_owned() + whom;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
        }

        // The follower cannot unfollow himself.
        if who == whom {
            return Err(Box::new(TribblerError::Unknown(
                "The follower cannot follow himself.".to_string(),
            )));
        }

        // append the log entry
        let who_bin = self.bin_storage.bin(who).await?;
        let storage_clock = who_bin.clock(0).await?;
        let log_entry = storage_clock.to_string() + "::unfollow::" + whom;
        who_bin
            .list_append(&KeyValue {
                key: "log".to_string(),
                value: log_entry,
            })
            .await?;

        // check the log entry
        let mut followees = HashSet::new();
        let log = who_bin.list_get("log").await?;
        for log_entry in log.0 {
            let res: Vec<String> = log_entry.split("::").map(|s| s.to_string()).collect();
            let parsed_clock = (&res[0]).to_string(); // unique identifier
            let parsed_follow_string = (&res[1]).to_string(); // follow or unfollow
            let parsed_followee = (&res[2]).to_string(); // followee

            if parsed_follow_string == "follow" {
                if !followees.contains(&parsed_followee) && followees.len() < MAX_FOLLOWING {
                    followees.insert(parsed_followee);
                }
            } else {
                // unfollow
                if parsed_followee == whom {
                    if parsed_clock == storage_clock.to_string() {
                        // this operation
                        if followees.contains(&parsed_followee) {
                            return Ok(());
                        }
                        return Err(Box::new(TribblerError::NotFollowing(
                            who.to_string(),
                            whom.to_string(),
                        )));
                    } else {
                        // other operations
                        if followees.contains(&parsed_followee) {
                            followees.remove(&parsed_followee);
                        }
                    }
                } else {
                    if followees.contains(&parsed_followee) {
                        followees.remove(&parsed_followee);
                    }
                }
            }
        }
        return Ok(());
    }

    async fn is_following(&self, who: &str, whom: &str) -> TribResult<bool> {
        // println!("is_follow input: {}", who);
        // println!("is_follow input: {}", whom);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }
        if !is_valid_username(whom) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(whom.to_string())));
        }

        // use the general bin to check if who and whom have signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let mut signup_string = "signup_".to_owned() + who;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }
        signup_string = "signup_".to_owned() + whom;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
        }

        // The follower cannot follow/unfollow himself.
        if who == whom {
            return Err(Box::new(TribblerError::Unknown(
                "The follower cannot follow himself.".to_string(),
            )));
        }

        // check who's followees
        let followee_vec = self.following(who).await?;
        return Ok(followee_vec.contains(&whom.to_string()));
    }

    async fn following(&self, who: &str) -> TribResult<Vec<String>> {
        // println!("following input: {}", who);
        if !is_valid_username(who) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(who.to_string())));
        }

        // use the general bin to check if who has signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let signup_string = "signup_".to_owned() + who;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }

        // check the log entry
        let mut followees = HashSet::new();
        let who_bin = self.bin_storage.bin(who).await?;
        let log = who_bin.list_get("log").await?;
        for log_entry in log.0 {
            let res: Vec<String> = log_entry.split("::").map(|s| s.to_string()).collect();
            let parsed_follow_string = (&res[1]).to_string(); // follow or unfollow
            let parsed_followee = (&res[2]).to_string(); // followee

            if parsed_follow_string == "follow" {
                if !followees.contains(&parsed_followee) && followees.len() < MAX_FOLLOWING {
                    followees.insert(parsed_followee);
                }
            } else {
                if followees.contains(&parsed_followee) {
                    followees.remove(&parsed_followee);
                }
            }
        }
        let mut followee_vec = Vec::<String>::new();
        for followee in followees {
            followee_vec.push(followee.to_string());
        }
        followee_vec.sort();
        return Ok(followee_vec);
    }

    async fn home(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        // println!("home input: {}", user);
        if !is_valid_username(user) {
            // invalid user name
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }

        // use the general bin to check if the user has signed up
        let general_bin = self.bin_storage.bin("").await?; // get the general bin
        let signup_string = "signup_".to_owned() + user;
        let signed = general_bin.get(&signup_string).await?;
        if signed == None {
            return Err(Box::new(TribblerError::UserDoesNotExist(user.to_string())));
        }

        // get the tribs of the user
        let mut user_home = Vec::<Arc<Trib>>::new();
        let mut user_tribs = self.tribs(user).await?;
        user_home.append(&mut user_tribs);

        // get the tribs of the followees
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
        // println!("home output: {:?}", user_home);
        return Ok(user_home);
    }
}

// follow the priority to sort the tribs
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
