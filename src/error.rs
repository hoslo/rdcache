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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_redis_error() {
        let error = new_redis_error(rustis::Error::Aborted);
        assert!(matches!(error, Error::RedisError(_)));
    }

    #[test]
    fn test_new_encode_error() {
        let err = new_encode_error(rmp_serde::encode::Error::UnknownLength);
        assert!(matches!(err, Error::EncodeError(_)));
    }

    #[test]
    fn test_new_decode_error() {
        let error = new_decode_error(rmp_serde::decode::Error::OutOfRange);
        assert!(matches!(error, Error::DecodeError(_)));
    }
}
