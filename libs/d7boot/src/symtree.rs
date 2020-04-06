#[cfg(not(test))]
use core::u16;
#[cfg(test)]
use std::{fmt, u16};

use super::error;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Node {
    pub left: u16,
    pub right: u16,
}
#[cfg(test)]
impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_leaf() {
            write!(f, "Leaf({})", self.right)
        } else if self.is_empty() {
            write!(f, "Empty()")
        } else if self.is_missing(false) {
            write!(f, "Branch(?, {})", self.right)
        } else if self.is_missing(true) {
            write!(f, "Branch({}, ?)", self.left)
        } else {
            write!(f, "Branch({}, {})", self.left, self.right)
        }
    }
}
impl Node {
    /// Special value marking that branch is misssing
    const MISSING: u16 = u16::MAX;
    /// If left is LEAF, the right is value
    const LEAF: u16 = u16::MAX - 1;

    pub fn is_leaf(&self) -> bool {
        self.left == Self::LEAF
    }

    /// Check is_leaf first to see if there is a value
    pub fn value(&self) -> u8 {
        self.right as u8
    }

    pub fn direction(&self, direction: bool) -> u16 {
        if direction { self.right } else { self.left }
    }

    pub fn with_direction(self, direction: bool, index: u16) -> Node {
        if direction {
            Self {
                right: index,
                ..self
            }
        } else {
            Self {
                left: index,
                ..self
            }
        }
    }

    pub fn is_missing(&self, direction: bool) -> bool {
        self.direction(direction) == Self::MISSING
    }

    pub fn is_empty(&self) -> bool {
        self.is_missing(false) && self.is_missing(true)
    }

    pub const fn new_empty() -> Self {
        Self {
            left: Self::MISSING,
            right: Self::MISSING,
        }
    }

    pub const fn new_leaf(value: u8) -> Self {
        Self {
            left: Self::LEAF,
            right: value as u16,
        }
    }
}

#[repr(C)]
pub struct SymTree {
    pub nodes: [Node; 0x200 - 1],
    pub next_free: u16,
}
impl SymTree {
    /// Reserved in Plan.md
    pub const ADDR: usize = 0x4000;

    /// Unsafe: Must be called only once
    pub unsafe fn getmut() -> &'static mut Self {
        &mut *(Self::ADDR as *mut Self)
    }

    pub fn init(&mut self) {
        unsafe {
            // Init root
            self.nodes[0] = Node::new_empty();
            // And next available just after root
            self.next_free = 1;
        }
    }

    fn node(&self, i: u16) -> Node {
        unsafe { *self.nodes.get_unchecked(i as usize) }
    }

    fn set_node(&mut self, i: u16, n: Node) {
        unsafe {
            *self.nodes.get_unchecked_mut(i as usize) = n;
        }
    }

    fn add_node(&mut self, n: Node) -> u16 {
        let new_index = self.next_free;
        self.set_node(new_index, n);
        self.next_free += 1;
        new_index
    }

    /// Must be called only once per value
    /// Depth limit must be over 0
    pub fn set(&mut self, value: u8, depth: u8, mut path: u32) {
        let mut cursor: u16 = 0; // Zero is root
        for _ in 0..depth {
            let direction = path & 1 != 0;
            path = path >> 1;

            let mut node = self.node(cursor);
            if node.is_leaf() {
                error('&');
            } else if node.is_missing(direction) {
                let new_index = self.add_node(Node::new_empty());
                self.set_node(cursor, node);
                self.set_node(cursor, node.with_direction(direction, new_index));
                cursor = new_index;
            } else {
                cursor = node.direction(direction);
            }
        }
        self.set_node(cursor, Node::new_leaf(value));
    }

    /// Depth limit is 0 for no nodes, and 1 for root, etc.
    /// Bool true if found
    #[inline]
    pub fn get(&self, depth_limit: u8, mut path: u32) -> (bool, u8) {
        let mut cursor: u16 = 0; // Zero is root
        for i in 0..(depth_limit + 1) {
            let direction = path & 1 != 0;
            path = path >> 1;

            let node = self.node(cursor);
            if node.is_leaf() {
                return (true, node.right as u8);
            } else if node.is_missing(direction) {
                error('?');
            } else {
                cursor = node.direction(direction);
            }
        }
        (false, 0)
    }

    /// Private mock constructor for unit testing
    /// Filled with weird values to make easier to notice uninit use
    pub fn mock_new() -> Self {
        Self {
            nodes: [Node::new_empty(); 0x200 - 1],
            next_free: 1,
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Node, SymTree};
    use std::mem::size_of;

    #[test]
    fn size_check() {
        assert!(size_of::<Node>() == 4);
        assert_eq!(size_of::<SymTree>(), 4 * (0x200 - 1) + 2);
    }

    #[test]
    fn test_symtree_depths() {
        let mut st = SymTree::mock_new();
        st.init();

        st.set(1, 2, 0b00);
        st.set(2, 3, 0b010);
        st.set(3, 3, 0b110);
        st.set(4, 1, 0b1);

        println!("Nodes: {:?}", st.nodes[..(st.next_free as usize)].to_vec());
        println!("NextF: {:?}", st.next_free);

        assert_eq!(st.get(0, 0b00), (false, 0));
        assert_eq!(st.get(0, 0b010), (false, 0));
        assert_eq!(st.get(0, 0b110), (false, 0));
        assert_eq!(st.get(0, 0b1), (false, 0));

        assert_eq!(st.get(1, 0b00), (false, 0));
        assert_eq!(st.get(1, 0b010), (false, 0));
        assert_eq!(st.get(1, 0b110), (false, 0));
        assert_eq!(st.get(1, 0b1), (true, 4));

        assert_eq!(st.get(2, 0b00), (true, 1));
        assert_eq!(st.get(2, 0b010), (false, 0));
        assert_eq!(st.get(2, 0b110), (false, 0));
        assert_eq!(st.get(2, 0b1), (true, 4));

        assert_eq!(st.get(3, 0b00), (true, 1));
        assert_eq!(st.get(3, 0b010), (true, 2));
        assert_eq!(st.get(3, 0b110), (true, 3));
        assert_eq!(st.get(3, 0b1), (true, 4));

        assert_eq!(st.get(4, 0b00), (true, 1));
        assert_eq!(st.get(4, 0b010), (true, 2));
        assert_eq!(st.get(4, 0b110), (true, 3));
        assert_eq!(st.get(4, 0b1), (true, 4));
    }

    #[test]
    fn test_symtree_2() {
        let mut st = SymTree::mock_new();
        st.init();

        st.set(1, 1, 0b1);
        st.set(2, 2, 0b10);
        st.set(3, 3, 0b100);
        st.set(4, 3, 0b000);

        println!("Nodes: {:?}", st.nodes[..(st.next_free as usize)].to_vec());
        println!("NextF: {:?}", st.next_free);

        assert_eq!(st.get(1, 0b1), (true, 1));
        assert_eq!(st.get(2, 0b10), (true, 2));
        assert_eq!(st.get(3, 0b100), (true, 3));
        assert_eq!(st.get(3, 0b000), (true, 4));
    }
}
