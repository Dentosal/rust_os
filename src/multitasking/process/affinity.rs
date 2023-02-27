#[derive(Debug, Clone)]
pub enum CpuAffinity {
    /// Must run on the BSP
    BspOnly,
    /// Any CPU core is ok
    Neutral,
}
