/// Common error/result types for the metadata gateway loop.

pub type MetaError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type MetaResult<T> = Result<T, MetaError>;
