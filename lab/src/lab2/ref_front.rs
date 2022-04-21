#![allow(dead_code)]
use std::{
    cmp::{min, Ordering},
    collections::{HashMap, HashSet},
    sync::{
        atomic::{self, AtomicU64},
        Arc, RwLock,
    },
    time::SystemTime,
};

use async_trait::async_trait;

use tribbler::{
    err::{TribResult, TribblerError},
    trib::{
        is_valid_username, Server, Trib, MAX_FOLLOWING, MAX_TRIB_FETCH, MAX_TRIB_LEN, MIN_LIST_USER,
    },
};

/// The [User] type holds the data on tribs the user has posted along with
/// related follower information.
#[derive(Debug)]
struct User {
    following: HashSet<String>,
    followers: HashSet<String>,
    seq_tribs: Vec<SeqTrib>,
    tribs: Vec<Arc<Trib>>,
}

/// A [Trib] type with an additional sequence number
#[derive(Debug, Clone)]
struct SeqTrib {
    seq: u64,
    trib: Arc<Trib>,
}

impl Ord for SeqTrib {
    fn cmp(&self, other: &Self) -> Ordering {
        self.seq.cmp(&other.seq)
    }
}

impl Eq for SeqTrib {}

impl PartialOrd for SeqTrib {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.seq.partial_cmp(&other.seq)
    }
}

impl PartialEq for SeqTrib {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq
    }
}

impl User {
    /// creates a new user reference
    fn new() -> User {
        User {
            following: HashSet::new(),
            followers: HashSet::new(),
            seq_tribs: vec![],
            tribs: vec![],
        }
    }

    /// Checks whether this user is following `whom`
    fn is_following(&self, whom: &str) -> bool {
        self.following.contains(whom)
    }

    /// updates [User] to follow `whom`
    fn follow(&mut self, whom: &str) {
        self.following.insert(whom.to_string());
    }

    /// updates [User] to unfollow `whom`
    fn unfollow(&mut self, whom: &str) {
        self.following.remove(whom);
    }

    /// updates [User] to add to the follower list
    fn add_follower(&mut self, who: &str) {
        self.followers.insert(who.to_string());
    }

    /// updates [User] to remove from the follower list
    fn remove_follower(&mut self, who: &str) {
        self.followers.remove(who);
    }

    /// lists the [User]s that this user follows
    fn list_following(&self) -> Vec<String> {
        self.following.iter().map(String::clone).collect()
    }

    /// instructs this [User] to post a new [Trib] with the given parameters
    /// returns a reference to the posted [Trib]
    ///
    /// Note: `time` refers to Unix time. In other words, time since epoch in seconds.
    fn post(&mut self, who: &str, msg: &str, seq: u64, time: u64) -> Arc<Trib> {
        // make the new trib
        let trib = Arc::new(Trib {
            user: who.to_string(),
            message: msg.to_string(),
            time,
            clock: seq,
        });
        // append sequential number
        let seq_trib = SeqTrib {
            seq,
            trib: trib.clone(),
        };

        // add to my own tribs
        self.tribs.push(trib.clone());
        self.seq_tribs.push(seq_trib);
        trib
    }

    /// Gets the list of [Trib]s posted by this [User]
    fn list_tribs(&self) -> &[Arc<Trib>] {
        let ntrib = self.tribs.len();
        let start = match ntrib.cmp(&MAX_TRIB_FETCH) {
            Ordering::Greater => ntrib - MAX_TRIB_FETCH,
            _ => 0,
        };
        &self.tribs[start..]
    }
}

pub struct FrontServer {
    users: Arc<RwLock<HashMap<String, User>>>,
    homes: Arc<RwLock<HashMap<String, Vec<Arc<Trib>>>>>,
    seq: AtomicU64,
}

impl FrontServer {
    /// Creates a [RefServer] with no data
    pub fn new() -> FrontServer {
        FrontServer {
            users: Arc::new(RwLock::new(HashMap::new())),
            homes: Arc::new(RwLock::new(HashMap::new())),
            seq: AtomicU64::new(0),
        }
    }

    /// rebuilds the users' homepage based on the current set of [SeqTrib]s and
    /// other users' tribs
    fn rebuild_home(&self, who: &User, users: &HashMap<String, User>) -> Vec<Arc<Trib>> {
        let mut home: Vec<SeqTrib> = vec![];
        home.append(&mut who.seq_tribs.clone());
        for user in who.following.iter() {
            match users.get(user) {
                Some(v) => {
                    home.append(&mut v.seq_tribs.clone());
                }
                None => continue,
            };
        }
        home.sort();
        home.iter()
            .map(|x| x.trib.clone())
            .collect::<Vec<Arc<Trib>>>()
    }
}

impl Default for FrontServer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Server for FrontServer {
    async fn sign_up(&self, user: &str) -> TribResult<()> {
        if !is_valid_username(user) {
            return Err(Box::new(TribblerError::InvalidUsername(user.to_string())));
        }
        let mut users = self.users.write().unwrap(); // get exclusive write access
        match users.contains_key(user) {
            // repetitive users
            true => Err(Box::new(TribblerError::UsernameTaken(user.to_string()))),
            false => {
                users.insert(user.to_string(), User::new());
                let mut homes = self.homes.write().unwrap();
                homes.insert(user.to_string(), vec![]); // add the user's home
                Ok(())
            }
        }
    }

    async fn list_users(&self) -> TribResult<Vec<String>> {
        let users = self.users.read().unwrap();
        let mut k: Vec<&String> = users.keys().collect();
        k.sort(); // sorted in alphabetical order
        let sorted = k[..min(MIN_LIST_USER, k.len())].to_vec(); // list at most 20 users
        let res: Vec<String> = sorted
            .iter() // convert the vector to an iterator
            .map(|x| x.to_string()) // convert each &str to String
            .collect::<Vec<String>>(); // convert the iterator a vector
        Ok(res)
    }

    // Tribs are not modified yet!!!!
    async fn post(&self, who: &str, post: &str, clock: u64) -> TribResult<()> {
        if post.len() > MAX_TRIB_LEN {
            // The post is too long.
            return Err(Box::new(TribblerError::TribTooLong));
        }
        let mut users = self.users.write().unwrap();
        match users.get_mut(who) {
            // get a mutable reference of the value
            Some(user) => {
                if self.seq.load(atomic::Ordering::SeqCst) == u64::MAX {
                    return Err(Box::new(TribblerError::MaxedSeq));
                }
                let _ = self.seq.fetch_update(
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                    |v| {
                        if v < clock {
                            Some(clock)
                        } else {
                            None
                        }
                    },
                );

                let trib = user.post(
                    who,
                    post,
                    self.seq.fetch_add(1, atomic::Ordering::SeqCst), 
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)?
                        .as_secs(), // machine time
                );
                // add it to the timeline of my followers
                let mut homes = self.homes.write().unwrap(); // get homes of all followers
                for follower in user.followers.iter() {
                    homes
                        .entry(follower.to_string()) // get the home of this follower
                        .and_modify(|e| e.push(trib.clone())); // add the trib to its home
                }
                // add it to my own timeline
                homes
                    .entry(who.to_string())
                    .and_modify(|e| e.push(trib.clone()));
                Ok(())
            }
            None => Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
        }
    }

    // list the most recent 100 tribbles
    async fn tribs(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        let users = self.users.read().unwrap();
        match users.get(user) {
            Some(user) => {
                let user_tribs = user.list_tribs();
                let n = min(user_tribs.len(), MAX_TRIB_FETCH);
                let mut start = 0;
                if n > MAX_TRIB_FETCH {
                    // ex. n = 120 => [20:]
                    start = n - MAX_TRIB_FETCH;
                }
                Ok(user.list_tribs()[start..].to_vec())
            }
            None => Err(Box::new(TribblerError::UserDoesNotExist(user.to_string()))),
        }
    }

    async fn follow(&self, who: &str, whom: &str) -> TribResult<()> {
        if who == whom {
            return Err(Box::new(TribblerError::WhoWhom(who.to_string())));
        }
        let mut users = self.users.write().unwrap();
        if !users.contains_key(whom) {
            // The followee doesn't exist.
            return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string())));
        }
        match users.get_mut(who) {
            Some(u) => {
                if u.is_following(whom) {
                    return Err(Box::new(TribblerError::AlreadyFollowing(
                        who.to_string(),
                        whom.to_string(),
                    )));
                }
                // cannot follow too many people
                let followee_num = u.following.len();
                if followee_num >= MAX_FOLLOWING {
                    return Err(Box::new(TribblerError::FollowingTooMany));
                }
                u.follow(whom);
            }
            // The follower doesn't exist.
            None => return Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
        };
        // add a follower to the followee
        let _ = users
            .entry(whom.to_string())
            .and_modify(|e| e.add_follower(who));
        // rebuild home
        match users.get(who) {
            Some(user) => {
                // add the posts of the new followees
                let mut homes = self.homes.write().unwrap();
                homes.insert(who.to_string(), self.rebuild_home(user, &users));
                Ok(())
            }
            None => Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
        }
    }

    async fn unfollow(&self, who: &str, whom: &str) -> TribResult<()> {
        if who == whom {
            return Err(Box::new(TribblerError::WhoWhom(who.to_string())));
        }
        let mut users = self.users.write().unwrap();
        if !users.contains_key(whom) {
            return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
        }
        match users.get_mut(who) {
            Some(u) => {
                if !u.is_following(whom) {
                    return Err(Box::new(TribblerError::NotFollowing(
                        who.to_string(),
                        whom.to_string(),
                    )));
                }
                u.unfollow(whom);
            }
            None => return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string()))),
        };
        let _ = users
            .entry(whom.to_string())
            .and_modify(|e| e.remove_follower(who));
        // rebuild home
        match users.get(who) {
            Some(user) => {
                let mut homes = self.homes.write().unwrap();
                homes.insert(who.to_string(), self.rebuild_home(user, &users));
                Ok(())
            }
            None => Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
        }
    }

    async fn is_following(&self, who: &str, whom: &str) -> TribResult<bool> {
        if who == whom {
            return Err(Box::new(TribblerError::WhoWhom(who.to_string())));
        }
        let users = self.users.read().unwrap();
        if !users.contains_key(whom) {
            // The followee doesn't exist.
            return Err(Box::new(TribblerError::UserDoesNotExist(whom.to_string())));
        }
        match users.get(who) {
            Some(user) => Ok(user.is_following(whom)),
            None => Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
            // The follower doesn't exist.
        }
    }

    async fn following(&self, who: &str) -> TribResult<Vec<String>> {
        let users = self.users.read().unwrap();
        match users.get(who) {
            Some(user) => Ok(user.list_following()),
            None => Err(Box::new(TribblerError::UserDoesNotExist(who.to_string()))),
        }
    }

    async fn home(&self, user: &str) -> TribResult<Vec<Arc<Trib>>> {
        let homes = self.homes.read().unwrap();
        match homes.get(user) {
            Some(home) => {
                // show at most 100 tribs
                let ntrib = home.len();
                let start = match ntrib.cmp(&MAX_TRIB_FETCH) {
                    Ordering::Greater => ntrib - MAX_TRIB_FETCH,
                    _ => 0,
                };
                Ok(home[start..].to_vec())
            }
            None => Err(Box::new(TribblerError::UserDoesNotExist(user.to_string()))),
        }
    }
}
