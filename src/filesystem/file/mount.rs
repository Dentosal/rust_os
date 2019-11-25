use crate::multitasking::ProcessId;

/// # Mount point
/// This node and its contents are managed by a driver
/// software. On branch nodes, the driver can provide
/// child nodes that are used in addition to the ones
/// described here.
/// ## Caching
/// In the future, this filesystem tree should be able
/// to cache filesystem trees from drivers that indicate
/// that the paths are allowed to be cached.
/// ## Nesting mount points
/// Nested mounts are allowed.
/// The innermost mount point will receive all operations,
/// and the relayed path is relative to the mount point.
/// ## Unmounting
/// Unlike Linux, where unmounting requires that all inner
/// mounts are unmounted first, this implementation simply
/// fabricates paths until the inner mount point.
#[derive(Debug, Clone)]
pub struct MountTarget {
    /// Identifier of this mount point.
    /// Used by the managing process to differentiate
    /// multiple mount points.
    mount_id: u64,
    /// Process managing the mount point
    process_id: ProcessId,
    /// Leafness is a static property of a mount,
    /// the controlling process cannot change this
    is_leaf: bool,
}
