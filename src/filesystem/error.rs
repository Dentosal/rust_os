pub type FsResult<T> = Result<T, FsError>;

#[derive(Debug)]
#[must_use]
pub enum FsError {
    /// Trying to create a node which already exists
    NodeExists,
    /// Node is requested but does not exist
    NodeNotFound,
    /// Node could not be created
    NodeCreation,
    /// Invalid control function
    ControlFunction,
}
