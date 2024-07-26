#[derive(Debug)]
pub enum Error {
    RedisError(rustis::Error),
    EncodeError(rmp_serde::encode::Error),
    DecodeError(rmp_serde::decode::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn new_redis_error(err: rustis::Error) -> Error {
    Error::RedisError(err)
}

pub(crate) fn new_encode_error(err: rmp_serde::encode::Error) -> Error {
    Error::EncodeError(err)
}

pub(crate) fn new_decode_error(err: rmp_serde::decode::Error) -> Error {
    Error::DecodeError(err)
}
