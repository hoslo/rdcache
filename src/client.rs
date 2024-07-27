use crate::{
    error::{new_decode_error, new_encode_error, new_redis_error},
    script::Script,
    Error, Result,
};
use chrono::Local;
use rustis::{
    commands::{CallBuilder, GenericCommands, ScriptingCommands},
    resp::{CommandArgs, SingleArg, SingleArgCollection, Value},
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, future::Future, time::Duration};
use uuid::Uuid;

use crate::script::{DELETE_SCRIPT, GET_SCRIPT, SET_SCRIPT, UNLOCK_SCRIPT};

#[derive(Debug)]
pub struct Options {
    // Delay is the delay delete time for keys that are tag deleted. default is 10s
    pub delay: Duration,
    // EmptyExpire is the expire time for empty result. default is 60s
    pub empty_expire: Duration,
    // LockExpire is the expire time for the lock which is allocated when updating cache. default is 3s
    // should be set to the max of the underling data calculating time.
    pub lock_expire: Duration,
    // LockSleep is the sleep interval time if try lock failed. default is 100ms
    pub lock_sleep: Duration,
    // RandomExpireAdjustment is the random adjustment for the expire time. default 0.1
    // if the expire time is set to 600s, and this value is set to 0.1, then the actual expire time will be 540s - 600s
    // solve the problem of cache avalanche.
    pub random_expire_adjustment: f64,
    // CacheReadDisabled is the flag to disable read cache. default is false
    // when redis is down, set this flat to downgrade.
    pub disable_cache_read: bool,
    // CacheDeleteDisabled is the flag to disable delete cache. default is false
    // when redis is down, set this flat to downgrade.
    pub disable_cache_delete: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delay: Duration::from_secs(10),
            empty_expire: Duration::from_secs(60),
            lock_expire: Duration::from_secs(3),
            lock_sleep: Duration::from_millis(100),
            random_expire_adjustment: 0.1,
            disable_cache_read: false,
            disable_cache_delete: false,
        }
    }
}

pub struct Client {
    rdb: rustis::client::Client,
    pub options: Options,
}

impl Client {
    pub fn new(rdb: rustis::client::Client, options: Options) -> Self {
        Self { rdb, options }
    }
    pub fn rdb(&self) -> &rustis::client::Client {
        &self.rdb
    }

    pub async fn fetch<F, Fut, V>(
        &self,
        key: impl Into<String>,
        expire: Duration,
        f: F,
    ) -> Result<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>>>,
        V: DeserializeOwned + Serialize + Debug,
    {
        let ex = expire
            - self.options.delay
            - Duration::from_secs(
                (self.options.random_expire_adjustment * expire.as_secs() as f64) as u64,
            );
        if self.options.disable_cache_read {
            f().await
        } else {
            self.strong_fetch(&key.into(), ex, f).await
        }
    }

    pub async fn tag_as_deleted(&self, key: impl Into<String>) -> Result<()> {
        if self.options.disable_cache_delete {
            return Ok(());
        }
        self.call_lua(
            &DELETE_SCRIPT,
            CommandArgs::default().arg(key.into()).build(),
            CommandArgs::default()
                .arg(self.options.delay.as_secs())
                .build(),
        )
        .await?;
        Ok(())
    }

    async fn strong_fetch<F, Fut, V>(&self, key: &str, expire: Duration, f: F) -> Result<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>>>,
        V: DeserializeOwned + Serialize + Debug,
    {
        let owner = Uuid::new_v4().simple().to_string();
        let now = Local::now().timestamp() as u64;
        let (mut value, mut lock_until): (Value, Value) = self
            .call_lua(
                &GET_SCRIPT,
                CommandArgs::default().arg(key).build(),
                CommandArgs::default()
                    .arg(now)
                    .arg(now + self.options.lock_expire.as_secs())
                    .arg(&owner)
                    .build(),
            )
            .await?;
        while lock_until != Value::Nil && lock_until.to_string() != "LOCKED" {
            tokio::time::sleep(self.options.lock_sleep).await;
            (value, lock_until) = self
                .call_lua(
                    &GET_SCRIPT,
                    CommandArgs::default().arg(key).build(),
                    CommandArgs::default()
                        .arg(now)
                        .arg(now + self.options.lock_expire.as_secs())
                        .arg(&owner)
                        .build(),
                )
                .await?;
        }
        if lock_until.to_string() != "LOCKED" {
            let Value::BulkString(s) = value else {
                return Err(Error::RedisError(rustis::Error::Aborted));
            };
            return rmp_serde::from_slice(&s).map_err(new_decode_error);
        }
        self.fetch_new(key, expire, &owner, f).await
    }

    async fn fetch_new<F, Fut, V>(
        &self,
        key: &str,
        expire: Duration,
        owner: &str,
        f: F,
    ) -> Result<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>>>,
        V: DeserializeOwned + Serialize + Debug,
    {
        let result = f().await;
        let mut expire = expire;

        match result {
            Ok(result) => {
                if result.is_none() {
                    expire = self.options.empty_expire;
                    if self.options.empty_expire.as_secs() == 0 {
                        _ = self.rdb.del(key).await.map_err(new_redis_error);
                    }
                }

                let result_bytes = rmp_serde::to_vec(&result).map_err(new_encode_error)?;
                self.call_lua(
                    &SET_SCRIPT,
                    CommandArgs::default().arg(key).build(),
                    CommandArgs::default()
                        .arg(result_bytes)
                        .arg(owner)
                        .arg(expire.as_secs())
                        .build(),
                )
                .await?;
                Ok(result)
            }
            Err(e) => {
                _ = self.unlock_for_update(key, owner).await;
                Err(e)
            }
        }
    }

    async fn unlock_for_update(&self, key: &str, owner: &str) -> Result<()> {
        let _: Vec<Value> = self
            .call_lua(
                &UNLOCK_SCRIPT,
                CommandArgs::default().arg(key).build(),
                CommandArgs::default()
                    .arg(owner)
                    .arg(self.options.lock_expire.as_secs())
                    .build(),
            )
            .await?;
        Ok(())
    }

    async fn call_lua<K, C, V>(&self, script: &Script, keys: C, args: C) -> Result<V>
    where
        K: SingleArg,
        C: SingleArgCollection<K> + Clone,
        V: DeserializeOwned,
    {
        let command = self.rdb.evalsha::<String>(
            CallBuilder::sha1(&script.hash)
                .keys(keys.clone())
                .args(args.clone()),
        );
        let r = self.rdb.send(command.command, None).await;
        match r {
            Ok(v) => {
                let resp: String = v.to_string();
                if resp.contains("kind: NoScript") {
                    let command = self.rdb.script_load::<&str, String>(script.src);
                    match self.rdb.send(command.command, None).await {
                        Ok(_) => {
                            let command = self.rdb.evalsha::<String>(
                                CallBuilder::sha1(&script.hash).keys(keys).args(args),
                            );

                            let r = self.rdb.send(command.command, None).await;

                            match r {
                                Ok(v) => v.to().map_err(new_redis_error),
                                Err(e) => Err(Error::RedisError(e)),
                            }
                        }
                        Err(_) => {
                            let command = self.rdb.evalsha::<String>(
                                CallBuilder::sha1(&script.hash).keys(keys).args(args),
                            );
                            let r = self.rdb.send(command.command, None).await;
                            match r {
                                Ok(v) => v.to().map_err(new_redis_error),
                                Err(e) => Err(Error::RedisError(e)),
                            }
                        }
                    }
                } else {
                    v.to().map_err(new_redis_error)
                }
            }
            Err(e) => Err(Error::RedisError(e)),
        }
    }
}
