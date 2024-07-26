use sha1::{Digest, Sha1};
use std::sync::LazyLock;

pub(crate) struct Script {
    pub src: &'static str,
    pub hash: String,
}

impl Script {
    pub fn new(src: &'static str) -> Self {
        let mut hasher = Sha1::new();

        hasher.update(src.as_bytes());

        let result = hasher.finalize();
        Self {
            src,
            hash: format!("{:x}", result),
        }
    }
}

pub(crate) static DELETE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
redis.call('HSET', KEYS[1], 'lockUntil', 0)
redis.call('HDEL', KEYS[1], 'lockOwner')
redis.call('EXPIRE', KEYS[1], ARGV[1])"#,
    )
});

pub(crate) static GET_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local v = redis.call('HGET', KEYS[1], 'value')
local lu = redis.call('HGET', KEYS[1], 'lockUntil')
if lu ~= false and tonumber(lu) < tonumber(ARGV[1]) or lu == false and v == false then
    redis.call('HSET', KEYS[1], 'lockUntil', ARGV[2])
    redis.call('HSET', KEYS[1], 'lockOwner', ARGV[3])
    return { v, 'LOCKED' }
end
return {v, lu}"#,
    )
});

pub(crate) static SET_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local o = redis.call('HGET', KEYS[1], 'lockOwner')
if o ~= ARGV[2] then
		return
end
redis.call('HSET', KEYS[1], 'value', ARGV[1])
redis.call('HDEL', KEYS[1], 'lockUntil')
redis.call('HDEL', KEYS[1], 'lockOwner')
redis.call('EXPIRE', KEYS[1], ARGV[3])"#,
    )
});

pub(crate) static UNLOCK_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local lo = redis.call('HGET', KEYS[1], 'lockOwner')
if lo == ARGV[1] then
	redis.call('HSET', KEYS[1], 'lockUntil', 0)
	redis.call('HDEL', KEYS[1], 'lockOwner')
	redis.call('EXPIRE', KEYS[1], ARGV[2])
end"#,
    )
});

// pub(crate) static GET_BATCH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
//     Script::new(
//         r#"
// local rets = {}
// for i, key in ipairs(KEYS)
// do
// 	local v = redis.call('HGET', key, 'value')
// 	local lu = redis.call('HGET', key, 'lockUntil')
// 	if lu ~= false and tonumber(lu) < tonumber(ARGV[1]) or lu == false and v == false then
// 		redis.call('HSET', key, 'lockUntil', ARGV[2])
// 		redis.call('HSET', key, 'lockOwner', ARGV[3])
// 		table.insert(rets, { v, 'LOCKED' })
// 	else
// 		table.insert(rets, {v, lu})
// 	end
// end
// return rets"#,
//     )
// });

// pub(crate) static SET_BATCH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
//     Script::new(
//         r#"
// local n = #KEYS
// for i, key in ipairs(KEYS)
// do
// 	local o = redis.call('HGET', key, 'lockOwner')
// 	if o ~= ARGV[1] then
// 			return
// 	end
// 	redis.call('HSET', key, 'value', ARGV[i+1])
// 	redis.call('HDEL', key, 'lockUntil')
// 	redis.call('HDEL', key, 'lockOwner')
// 	redis.call('EXPIRE', key, ARGV[i+1+n])
// end"#,
//     )
// });

// pub(crate) static LOCK_BATCH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
//     Script::new(
//         r#"
// for i, key in ipairs(KEYS) do
// 	redis.call('HSET', key, 'lockUntil', 0)
// 	redis.call('HDEL', key, 'lockOwner')
// 	redis.call('EXPIRE', key, ARGV[1])
// end"#,
//     )
// });
